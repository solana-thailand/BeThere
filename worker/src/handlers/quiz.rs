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
    extract::{Path, Query, State},
    response::Json,
};

use serde_json::json;

use event_checkin_domain::models::api::{QuizConfig, QuizStatus, QuizSubmitRequest};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use super::ext::{EventIdQuery, resolve_event};
use crate::error::WorkerError;
use crate::quiz;
use crate::state::AppState;

/// GET /api/quiz
/// Fetch quiz questions for the frontend.
///
/// Returns questions with options only (no correct answers).
/// If no quiz is configured, returns an empty response with `configured: false`.
#[worker::send]
pub async fn get_quiz(
    State(state): State<AppState>,
    Query(query): Query<EventIdQuery>,
) -> Result<Json<serde_json::Value>, WorkerError> {
    // Resolve event (uses events_kv if available, falls back to global config)
    let event = resolve_event(&state, query.event_id.as_deref()).await?;

    // Determine KV namespace for quiz data
    let kv = match state.events_kv.as_ref().or(state.quiz_kv.as_ref()) {
        Some(kv) => kv,
        None => {
            return Ok(Json(json!({
                "success": true,
                "data": {
                    "configured": false,
                    "questions": [],
                    "passing_score_percent": 0,
                    "max_attempts": 0,
                    "time_limit_seconds": null,
                },
            })));
        }
    };

    let eid = event.id.as_str();

    let config = quiz::get_quiz_config(kv, eid)
        .await
        .map_err(|e| AppError::Internal(format!("failed to read quiz: {e}")))?;

    match config {
        Some(config) => {
            let public = quiz::to_public_questions(&config);
            Ok(Json(json!({
                "success": true,
                "data": {
                    "configured": true,
                    "questions": public.questions,
                    "passing_score_percent": public.passing_score_percent,
                    "max_attempts": public.max_attempts,
                    "time_limit_seconds": public.time_limit_seconds,
                },
            })))
        }
        None => Ok(Json(json!({
            "success": true,
            "data": {
                "configured": false,
                "questions": [],
                "passing_score_percent": 0,
                "max_attempts": 0,
                "time_limit_seconds": null,
            },
        }))),
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
    Query(query): Query<EventIdQuery>,
    Json(body): Json<QuizSubmitRequest>,
) -> Result<Json<serde_json::Value>, WorkerError> {
    tracing::info!(
        "quiz submit for token: {token} ({} answers)",
        body.answers.len()
    );

    // Resolve event (uses events_kv if available, falls back to global config)
    let event = resolve_event(&state, query.event_id.as_deref()).await?;

    // Determine KV namespace for quiz data
    let kv = state
        .events_kv
        .as_ref()
        .or(state.quiz_kv.as_ref())
        .ok_or_else(|| AppError::Internal("quiz is not configured for this event".to_string()))?;

    let eid = event.id.as_str();

    // Verify claim token exists (attendee must be checked in)
    let sheets_kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());
    match crate::sheets::get_attendee_by_claim_token(
        &token,
        &state,
        &event.sheet_id,
        &event.sheet_name,
        sheets_kv,
    )
    .await
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            tracing::warn!("quiz submit: invalid claim token {token}");
            return Err(AppError::NotFound(
                "invalid claim token — you must be checked in first".to_string(),
            )
            .into());
        }
        Err(ref e) => {
            tracing::error!("quiz submit: failed to look up claim token {token}: {e}");
            return Err(AppError::Internal(format!("failed to verify claim: {e}")).into());
        }
    }

    // Load quiz config
    let config = match quiz::get_quiz_config(kv, eid).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Err(AppError::NotFound("no quiz configured for this event".to_string()).into());
        }
        Err(e) => {
            tracing::error!("quiz submit: failed to read config: {e}");
            return Err(AppError::Internal(format!("failed to read quiz: {e}")).into());
        }
    };

    // Validate answers — each question_id must exist in config
    for answer in &body.answers {
        if !config.questions.iter().any(|q| q.id == answer.question_id) {
            tracing::warn!(
                "quiz submit: unknown question_id '{}' in answers",
                answer.question_id
            );
            return Err(AppError::Validation(format!(
                "unknown question id: {}",
                answer.question_id
            ))
            .into());
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
    let result = quiz::submit_quiz(kv, eid, &config, &token, &body.answers)
        .await
        .map_err(|e| {
            tracing::error!("quiz submit failed for token {token}: {e}");
            AppError::Internal(e.to_string())
        })?;

    tracing::info!(
        "quiz scored: token={token} attempt={} score={}% passed={}",
        result.attempt_number,
        result.score_percent,
        result.passed,
    );

    Ok(Json(json!({
        "success": true,
        "data": result,
    })))
}

/// GET /api/quiz/{token}/status
/// Get the quiz progress for an attendee.
///
/// Returns attempts used, best score, and whether passed.
#[worker::send]
pub async fn get_quiz_status(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<Json<serde_json::Value>, WorkerError> {
    tracing::info!("quiz status for token: {token}");

    // Resolve event (uses events_kv if available, falls back to global config)
    let event = resolve_event(&state, query.event_id.as_deref()).await?;

    // Determine KV namespace for quiz data
    let kv = match state.events_kv.as_ref().or(state.quiz_kv.as_ref()) {
        Some(kv) => kv,
        None => {
            return Ok(Json(json!({
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
            })));
        }
    };

    let eid = event.id.as_str();

    let config = match quiz::get_quiz_config(kv, eid).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Ok(Json(json!({
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
            })));
        }
        Err(e) => {
            tracing::error!("quiz status: failed to read config: {e}");
            return Err(AppError::Internal(format!("failed to read quiz: {e}")).into());
        }
    };

    let status = quiz::get_quiz_status(kv, eid, &token).await.map_err(|e| {
        tracing::error!("quiz status failed for token {token}: {e}");
        AppError::Internal(e.to_string())
    })?;

    let progress = quiz::get_quiz_progress(kv, eid, &token)
        .await
        .unwrap_or(None);

    let (attempts, best_score, passed) = match &progress {
        Some(p) => (p.attempts, p.best_score_percent, p.passed),
        None => (0u8, 0u8, false),
    };

    Ok(Json(json!({
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
    })))
}

/// GET /api/admin/quiz
/// Fetch full quiz config including correct answers (staff/admin only).
///
/// Returns the complete QuizConfig so the admin UI can load and edit it.
/// Unlike the public GET /api/quiz, this includes `correct_index` fields.
#[worker::send]
pub async fn get_admin_quiz(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<Json<serde_json::Value>, WorkerError> {
    tracing::info!("admin quiz read by {}", _claims.email);

    // Resolve event (uses events_kv if available, falls back to global config)
    let event = resolve_event(&state, query.event_id.as_deref()).await?;

    // Determine KV namespace for quiz data
    let kv = match state.events_kv.as_ref().or(state.quiz_kv.as_ref()) {
        Some(kv) => kv,
        None => {
            return Ok(Json(json!({
                "success": true,
                "data": {
                    "configured": false,
                    "questions": [],
                    "passing_score_percent": 0,
                    "max_attempts": 0,
                    "time_limit_seconds": null,
                },
            })));
        }
    };

    let eid = event.id.as_str();

    let config = quiz::get_quiz_config(kv, eid).await.map_err(|e| {
        tracing::error!("failed to read quiz config: {e}");
        AppError::Internal(format!("failed to read quiz: {e}"))
    })?;

    match config {
        Some(config) => Ok(Json(json!({
            "success": true,
            "data": {
                "configured": true,
                "questions": config.questions,
                "passing_score_percent": config.passing_score_percent,
                "max_attempts": config.max_attempts,
                "time_limit_seconds": config.time_limit_seconds,
            },
        }))),
        None => Ok(Json(json!({
            "success": true,
            "data": {
                "configured": false,
                "questions": [],
                "passing_score_percent": 0,
                "max_attempts": 0,
                "time_limit_seconds": null,
            },
        }))),
    }
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
    Query(query): Query<EventIdQuery>,
    Json(body): Json<QuizConfig>,
) -> Result<Json<serde_json::Value>, WorkerError> {
    tracing::info!(
        "admin quiz update by {} ({} questions)",
        _claims.email,
        body.questions.len()
    );

    // Resolve event (uses events_kv if available, falls back to global config)
    let event = resolve_event(&state, query.event_id.as_deref()).await?;

    // Determine KV namespace for quiz data
    let kv = state
        .events_kv
        .as_ref()
        .or(state.quiz_kv.as_ref())
        .ok_or_else(|| {
            AppError::Internal(
                "quiz KV namespace not configured — add QUIZ binding in wrangler.toml".to_string(),
            )
        })?;

    let eid = event.id.as_str();

    // Validate: at least 1 question
    if body.questions.is_empty() {
        return Err(AppError::Validation("quiz must have at least 1 question".to_string()).into());
    }

    // Validate: each question has at least 2 options
    for q in &body.questions {
        if q.options.len() < 2 {
            return Err(AppError::Validation(format!(
                "question '{}' must have at least 2 options",
                q.id
            ))
            .into());
        }
        if (q.correct_index as usize) >= q.options.len() {
            return Err(AppError::Validation(format!(
                "question '{}' correct_index {} out of range (0-{})",
                q.id,
                q.correct_index,
                q.options.len() - 1
            ))
            .into());
        }
    }

    // Validate: passing score 1-100
    if body.passing_score_percent == 0 || body.passing_score_percent > 100 {
        return Err(AppError::Validation(
            "passing_score_percent must be between 1 and 100".to_string(),
        )
        .into());
    }

    // Validate: max attempts >= 1
    if body.max_attempts == 0 {
        return Err(AppError::Validation("max_attempts must be at least 1".to_string()).into());
    }

    // Validate: unique question IDs
    let mut seen_ids = std::collections::HashSet::new();
    for q in &body.questions {
        if !seen_ids.insert(&q.id) {
            return Err(AppError::Validation(format!("duplicate question id: '{}'", q.id)).into());
        }
    }

    quiz::save_quiz_config(kv, eid, &body).await.map_err(|e| {
        tracing::error!("failed to save quiz: {e}");
        AppError::Internal(format!("failed to save quiz: {e}"))
    })?;

    tracing::info!(
        "quiz saved: {} questions, {}% passing, {} max attempts",
        body.questions.len(),
        body.passing_score_percent,
        body.max_attempts,
    );

    Ok(Json(json!({
        "success": true,
        "data": {
            "questions_count": body.questions.len(),
            "passing_score_percent": body.passing_score_percent,
            "max_attempts": body.max_attempts,
        },
    })))
}
