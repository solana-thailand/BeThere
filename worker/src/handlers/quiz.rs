//! Quiz API handlers for the activity-gated claim flow (Issue 002).
//!
//! Public endpoints:
//!   GET  /api/quiz                    — quiz questions (no correct answers)
//!   POST /api/quiz/{token}/submit     — submit answers, get scored
//!   GET  /api/quiz/{token}/status     — current quiz progress
//!
//! Admin endpoint (protected):
//!   PUT  /api/admin/quiz              — create or update quiz questions

use axum::{
    Extension,
    extract::{Path, State},
    response::Json,
};

use serde_json::json;

use event_checkin_domain::models::api::{QuizConfig, QuizStatus, QuizSubmitRequest};
use event_checkin_domain::models::auth::Claims;

use crate::quiz;
use crate::state::AppState;

/// GET /api/quiz
/// Fetch quiz questions for the frontend.
///
/// Returns questions with options only (no correct answers).
/// If no quiz is configured, returns an empty response with `configured: false`.
#[worker::send]
pub async fn get_quiz(State(state): State<AppState>) -> Json<serde_json::Value> {
    let kv = match state.quiz_kv {
        Some(ref kv) => kv,
        None => {
            return Json(json!({
                "success": true,
                "data": {
                    "configured": false,
                    "questions": [],
                    "passing_score_percent": 0,
                    "max_attempts": 0,
                    "time_limit_seconds": null,
                },
            }));
        }
    };

    match quiz::get_quiz_config(kv).await {
        Ok(Some(config)) => {
            let public = quiz::to_public_questions(&config);
            Json(json!({
                "success": true,
                "data": {
                    "configured": true,
                    "questions": public.questions,
                    "passing_score_percent": public.passing_score_percent,
                    "max_attempts": public.max_attempts,
                    "time_limit_seconds": public.time_limit_seconds,
                },
            }))
        }
        Ok(None) => Json(json!({
            "success": true,
            "data": {
                "configured": false,
                "questions": [],
                "passing_score_percent": 0,
                "max_attempts": 0,
                "time_limit_seconds": null,
            },
        })),
        Err(e) => {
            tracing::error!("failed to read quiz config: {e}");
            Json(json!({
                "success": false,
                "error": format!("failed to read quiz: {e}"),
            }))
        }
    }
}

/// POST /api/quiz/{token}/submit
/// Submit quiz answers for scoring.
///
/// The attendee must be checked in (claim token exists in sheets).
/// Answers are compared by **text** (not index) so frontend shuffling
/// doesn't break grading.
#[worker::send]
pub async fn submit_quiz(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(body): Json<QuizSubmitRequest>,
) -> Json<serde_json::Value> {
    tracing::info!(
        "quiz submit for token: {token} ({} answers)",
        body.answers.len()
    );

    let kv = match state.quiz_kv {
        Some(ref kv) => kv,
        None => {
            return Json(json!({
                "success": false,
                "error": "quiz is not configured for this event",
            }));
        }
    };

    // Verify claim token exists (attendee must be checked in)
    match crate::sheets::get_attendee_by_claim_token(&token, &state).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            tracing::warn!("quiz submit: invalid claim token {token}");
            return Json(json!({
                "success": false,
                "error": "invalid claim token — you must be checked in first",
            }));
        }
        Err(ref e) => {
            tracing::error!("quiz submit: failed to look up claim token {token}: {e}");
            return Json(json!({
                "success": false,
                "error": format!("failed to verify claim: {e}"),
            }));
        }
    }

    // Load quiz config
    let config = match quiz::get_quiz_config(kv).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Json(json!({
                "success": false,
                "error": "no quiz configured for this event",
            }));
        }
        Err(e) => {
            tracing::error!("quiz submit: failed to read config: {e}");
            return Json(json!({
                "success": false,
                "error": format!("failed to read quiz: {e}"),
            }));
        }
    };

    // Validate answers — each question_id must exist in config
    for answer in &body.answers {
        if !config.questions.iter().any(|q| q.id == answer.question_id) {
            tracing::warn!(
                "quiz submit: unknown question_id '{}' in answers",
                answer.question_id
            );
            return Json(json!({
                "success": false,
                "error": format!("unknown question id: {}", answer.question_id),
            }));
        }

        // Validate selected_text matches a valid option
        let question = config
            .questions
            .iter()
            .find(|q| q.id == answer.question_id)
            .unwrap();
        let selected = answer.selected_text.trim();
        if !selected.is_empty()
            && !question
                .options
                .iter()
                .any(|opt| opt.trim().eq_ignore_ascii_case(selected))
        {
            tracing::warn!(
                "quiz submit: selected_text '{}' not in options for question '{}'",
                answer.selected_text,
                answer.question_id
            );
            // Don't reveal options — just mark as wrong answer (don't reject)
        }
    }

    // Score and persist
    match quiz::submit_quiz(kv, &config, &token, &body.answers).await {
        Ok(result) => {
            tracing::info!(
                "quiz scored: token={token} attempt={} score={}% passed={}",
                result.attempt_number,
                result.score_percent,
                result.passed,
            );
            Json(json!({
                "success": true,
                "data": result,
            }))
        }
        Err(e) => {
            tracing::error!("quiz submit failed for token {token}: {e}");
            Json(json!({
                "success": false,
                "error": format!("{e}"),
            }))
        }
    }
}

/// GET /api/quiz/{token}/status
/// Get the quiz progress for an attendee.
///
/// Returns attempts used, best score, and whether passed.
#[worker::send]
pub async fn get_quiz_status(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Json<serde_json::Value> {
    tracing::info!("quiz status for token: {token}");

    let kv = match state.quiz_kv {
        Some(ref kv) => kv,
        None => {
            return Json(json!({
                "success": true,
                "data": {
                    "configured": false,
                    "quiz_status": "not_required",
                    "attempts": 0,
                    "max_attempts": 0,
                    "best_score_percent": 0,
                    "passed": false,
                    "passing_threshold_percent": 0,
                },
            }));
        }
    };

    let config = match quiz::get_quiz_config(kv).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Json(json!({
                "success": true,
                "data": {
                    "configured": false,
                    "quiz_status": "not_required",
                    "attempts": 0,
                    "max_attempts": 0,
                    "best_score_percent": 0,
                    "passed": false,
                    "passing_threshold_percent": 0,
                },
            }));
        }
        Err(e) => {
            tracing::error!("quiz status: failed to read config: {e}");
            return Json(json!({
                "success": false,
                "error": format!("failed to read quiz: {e}"),
            }));
        }
    };

    let status = match quiz::get_quiz_status(kv, &token).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("quiz status failed for token {token}: {e}");
            return Json(json!({
                "success": false,
                "error": format!("{e}"),
            }));
        }
    };

    let progress = quiz::get_quiz_progress(kv, &token).await.unwrap_or(None);

    let (attempts, best_score, passed) = match &progress {
        Some(p) => (p.attempts, p.best_score_percent, p.passed),
        None => (0u8, 0u8, false),
    };

    Json(json!({
        "success": true,
        "data": {
            "configured": true,
            "quiz_status": match status {
                QuizStatus::NotRequired => "not_required",
                QuizStatus::NotStarted => "not_started",
                QuizStatus::InProgress => "in_progress",
                QuizStatus::Passed => "passed",
            },
            "attempts": attempts,
            "max_attempts": config.max_attempts,
            "best_score_percent": best_score,
            "passed": passed,
            "passing_threshold_percent": config.passing_score_percent,
        },
    }))
}

/// PUT /api/admin/quiz
/// Create or update quiz questions (staff/admin only).
///
/// Accepts the full QuizConfig and stores it in KV.
/// Organizers call this before the event to set up the quiz.
#[worker::send]
pub async fn put_quiz(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(body): Json<QuizConfig>,
) -> Json<serde_json::Value> {
    tracing::info!(
        "admin quiz update by {} ({} questions)",
        _claims.email,
        body.questions.len()
    );

    let kv = match state.quiz_kv {
        Some(ref kv) => kv,
        None => {
            return Json(json!({
                "success": false,
                "error": "quiz KV namespace not configured — add QUIZ binding in wrangler.toml",
            }));
        }
    };

    // Validate: at least 1 question
    if body.questions.is_empty() {
        return Json(json!({
            "success": false,
            "error": "quiz must have at least 1 question",
        }));
    }

    // Validate: each question has at least 2 options
    for q in &body.questions {
        if q.options.len() < 2 {
            return Json(json!({
                "success": false,
                "error": format!("question '{}' must have at least 2 options", q.id),
            }));
        }
        if (q.correct_index as usize) >= q.options.len() {
            return Json(json!({
                "success": false,
                "error": format!(
                    "question '{}' correct_index {} out of range (0-{})",
                    q.id, q.correct_index, q.options.len() - 1
                ),
            }));
        }
    }

    // Validate: passing score 1-100
    if body.passing_score_percent == 0 || body.passing_score_percent > 100 {
        return Json(json!({
            "success": false,
            "error": "passing_score_percent must be between 1 and 100",
        }));
    }

    // Validate: max attempts >= 1
    if body.max_attempts == 0 {
        return Json(json!({
            "success": false,
            "error": "max_attempts must be at least 1",
        }));
    }

    // Validate: unique question IDs
    let mut seen_ids = std::collections::HashSet::new();
    for q in &body.questions {
        if !seen_ids.insert(&q.id) {
            return Json(json!({
                "success": false,
                "error": format!("duplicate question id: '{}'", q.id),
            }));
        }
    }

    match quiz::save_quiz_config(kv, &body).await {
        Ok(()) => {
            tracing::info!(
                "quiz saved: {} questions, {}% passing, {} max attempts",
                body.questions.len(),
                body.passing_score_percent,
                body.max_attempts,
            );
            Json(json!({
                "success": true,
                "data": {
                    "questions_count": body.questions.len(),
                    "passing_score_percent": body.passing_score_percent,
                    "max_attempts": body.max_attempts,
                },
            }))
        }
        Err(e) => {
            tracing::error!("failed to save quiz: {e}");
            Json(json!({
                "success": false,
                "error": format!("failed to save quiz: {e}"),
            }))
        }
    }
}
