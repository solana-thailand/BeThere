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

use event_checkin_domain::models::api::{
    QuizConfig, QuizQuestion, QuizStatus, QuizSubmitRequest, QuizSubmitResponse,
};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use super::ext::{EventIdQuery, resolve_event};
use crate::error::{ApiOk, WorkerError};
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
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    // Resolve event (uses events_kv if available, falls back to global config)
    let event = resolve_event(&state, query.event_id.as_deref()).await?;

    let eid = event.id.as_str();
    let d1 = state.d1.as_deref();
    let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());

    let config = quiz::get_quiz_config(d1, kv, eid)
        .await
        .map_err(|e| AppError::Internal(format!("failed to read quiz: {e}")))?;

    match config {
        Some(config) => {
            let public = quiz::to_public_questions(&config);
            Ok(ApiOk::new(json!({
                "configured": true,
                "questions": public.questions,
                "passing_score_percent": public.passing_score_percent,
                "max_attempts": public.max_attempts,
                "time_limit_seconds": public.time_limit_seconds,
            })))
        }
        None => Ok(ApiOk::new(json!({
            "configured": false,
            "questions": [],
            "passing_score_percent": 0,
            "max_attempts": 0,
            "time_limit_seconds": null,
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
) -> Result<ApiOk<QuizSubmitResponse>, WorkerError> {
    tracing::info!(
        claim_token = %token,
        answer_count = body.answers.len(),
        "quiz submit requested"
    );

    // Resolve event (uses events_kv if available, falls back to global config)
    let event = resolve_event(&state, query.event_id.as_deref()).await?;

    let eid = event.id.as_str();
    let d1 = state.d1.as_deref();
    let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());

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
            tracing::warn!(claim_token = %token, "quiz submit: invalid claim token");
            return Err(AppError::NotFound(
                "invalid claim token — you must be checked in first".to_string(),
            )
            .into());
        }
        Err(ref e) => {
            tracing::error!(claim_token = %token, error = ?e, "quiz submit: failed to look up claim token");
            return Err(AppError::Internal(format!("failed to verify claim: {e}")).into());
        }
    }

    // Load quiz config
    let config = match quiz::get_quiz_config(d1, kv, eid).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Err(AppError::NotFound("no quiz configured for this event".to_string()).into());
        }
        Err(e) => {
            tracing::error!(error = ?e, "quiz submit: failed to read quiz config");
            return Err(AppError::Internal(format!("failed to read quiz: {e}")).into());
        }
    };

    // Validate answers — each question_id must exist in config
    for answer in &body.answers {
        if !config.questions.iter().any(|q| q.id == answer.question_id) {
            tracing::warn!(
                question_id = %answer.question_id,
                "quiz submit: unknown question_id in answers"
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
                selected_text = %answer.selected_text,
                question_id = %answer.question_id,
                "quiz submit: selected_text not in options for question"
            );
            // Don't reveal options — just mark as wrong answer (don't reject)
        }
    }

    // Score and persist
    let result = quiz::submit_quiz(d1, kv, eid, &config, &token, &body.answers)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            // "No attempts remaining" is a normal user condition, not a server
            // fault — surface it as a clean 4xx with a friendly message instead
            // of a scary HTTP 500 "internal error".
            if msg.contains("no attempts remaining") {
                AppError::Validation(
                    "You've used all your quiz attempts for this event. Ask an organizer to reset them.".to_string(),
                )
            } else {
                tracing::error!(claim_token = %token, error = ?e, "quiz submit failed");
                AppError::Internal(msg)
            }
        })?;

    tracing::info!(
        claim_token = %token,
        attempt = result.attempt_number,
        score_percent = result.score_percent,
        passed = result.passed,
        "quiz scored"
    );

    Ok(ApiOk::new(result))
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
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!(claim_token = %token, "quiz status requested");

    // Resolve event (uses events_kv if available, falls back to global config)
    let event = resolve_event(&state, query.event_id.as_deref()).await?;

    let eid = event.id.as_str();
    let d1 = state.d1.as_deref();
    let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());

    let config = match quiz::get_quiz_config(d1, kv, eid).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Ok(ApiOk::new(json!({
                "configured": false,
                "quiz_status": "not_required",
                "attempts": 0,
                "max_attempts": 0,
                "best_score_percent": 0,
                "passed": false,
                "passing_threshold_percent": 0,
            })));
        }
        Err(e) => {
            tracing::error!(error = ?e, "quiz status: failed to read config");
            return Err(AppError::Internal(format!("failed to read quiz: {e}")).into());
        }
    };

    let status = quiz::get_quiz_status(d1, kv, eid, &token)
        .await
        .map_err(|e| {
            tracing::error!(claim_token = %token, error = ?e, "quiz status failed");
            AppError::Internal(e.to_string())
        })?;

    let progress = quiz::get_quiz_progress(d1, kv, eid, &token)
        .await
        .unwrap_or(None);

    let (attempts, best_score, passed) = match &progress {
        Some(p) => (p.attempts, p.best_score_percent, p.passed),
        None => (0u8, 0u8, false),
    };

    Ok(ApiOk::new(json!({
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
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!(staff_email = %_claims.email, "admin quiz read");

    // Resolve event (uses events_kv if available, falls back to global config)
    let event = resolve_event(&state, query.event_id.as_deref()).await?;

    let eid = event.id.as_str();
    let d1 = state.d1.as_deref();
    let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());

    let config = quiz::get_quiz_config(d1, kv, eid).await.map_err(|e| {
        tracing::error!(error = ?e, "failed to read quiz config");
        AppError::Internal(format!("failed to read quiz: {e}"))
    })?;

    match config {
        Some(config) => Ok(ApiOk::new(json!({
            "configured": true,
            "questions": config.questions,
            "passing_score_percent": config.passing_score_percent,
            "max_attempts": config.max_attempts,
            "time_limit_seconds": config.time_limit_seconds,
        }))),
        None => Ok(ApiOk::new(json!({
            "configured": false,
            "questions": [],
            "passing_score_percent": 0,
            "max_attempts": 0,
            "time_limit_seconds": null,
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
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!(
        staff_email = %_claims.email,
        question_count = body.questions.len(),
        "admin quiz update"
    );

    // Resolve event (uses events_kv if available, falls back to global config)
    let event = resolve_event(&state, query.event_id.as_deref()).await?;

    let eid = event.id.as_str();
    let d1 = state.d1.as_deref();
    let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());

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

    quiz::save_quiz_config(d1, kv, eid, &body)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "failed to save quiz");
            AppError::Internal(format!("failed to save quiz: {e}"))
        })?;

    tracing::info!(
        question_count = body.questions.len(),
        passing_score_percent = body.passing_score_percent,
        max_attempts = body.max_attempts,
        "quiz saved"
    );

    Ok(ApiOk::new(json!({
        "questions_count": body.questions.len(),
        "passing_score_percent": body.passing_score_percent,
        "max_attempts": body.max_attempts,
    })))
}

/// POST /api/admin/quiz/questions
/// Add a single question to the quiz.
#[worker::send]
pub async fn add_quiz_question(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
    Json(body): Json<QuizQuestion>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!(
        staff_email = %_claims.email,
        question_id = %body.id,
        "admin quiz add question"
    );

    let event = resolve_event(&state, query.event_id.as_deref()).await?;
    let d1 = state.d1.as_deref();
    let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());

    // Validate: at least 2 options
    if body.options.len() < 2 {
        return Err(
            AppError::Validation("question must have at least 2 options".to_string()).into(),
        );
    }
    if (body.correct_index as usize) >= body.options.len() {
        return Err(AppError::Validation(format!(
            "correct_index {} out of range",
            body.correct_index
        ))
        .into());
    }
    if body.text.trim().is_empty() {
        return Err(AppError::Validation("question text must not be empty".to_string()).into());
    }

    let question = quiz::add_question(d1, kv, event.id.as_str(), body)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(question_id = %question.id, "question added");
    Ok(ApiOk::new(json!({
        "question": question,
    })))
}

/// PUT /api/admin/quiz/questions/{id}
/// Update a single question by ID.
#[worker::send]
pub async fn update_quiz_question(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(question_id): Path<String>,
    Query(query): Query<EventIdQuery>,
    Json(body): Json<QuizQuestion>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!(
        staff_email = %_claims.email,
        question_id = %question_id,
        "admin quiz update question"
    );

    let event = resolve_event(&state, query.event_id.as_deref()).await?;
    let d1 = state.d1.as_deref();
    let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());

    // Validate
    if body.options.len() < 2 {
        return Err(
            AppError::Validation("question must have at least 2 options".to_string()).into(),
        );
    }
    if (body.correct_index as usize) >= body.options.len() {
        return Err(AppError::Validation(format!(
            "correct_index {} out of range",
            body.correct_index
        ))
        .into());
    }

    let question = quiz::update_question(d1, kv, event.id.as_str(), &question_id, body)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(question_id = %question.id, "question updated");
    Ok(ApiOk::new(json!({
        "question": question,
    })))
}

/// DELETE /api/admin/quiz/questions/{id}
/// Delete a single question by ID.
#[worker::send]
pub async fn delete_quiz_question(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(question_id): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!(
        staff_email = %_claims.email,
        question_id = %question_id,
        "admin quiz delete question"
    );

    let event = resolve_event(&state, query.event_id.as_deref()).await?;
    let d1 = state.d1.as_deref();
    let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());

    quiz::delete_question(d1, kv, event.id.as_str(), &question_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(question_id = %question_id, "question deleted");
    Ok(ApiOk::new(json!({
        "deleted": question_id,
    })))
}

/// PATCH /api/admin/quiz/questions/{id}/toggle
/// Toggle the enabled state of a single question.
#[worker::send]
pub async fn toggle_quiz_question(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(question_id): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!(
        staff_email = %_claims.email,
        question_id = %question_id,
        "admin quiz toggle question"
    );

    let event = resolve_event(&state, query.event_id.as_deref()).await?;
    let d1 = state.d1.as_deref();
    let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());

    let question = quiz::toggle_question(d1, kv, event.id.as_str(), &question_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(question_id = %question.id, enabled = question.enabled, "question toggled");
    Ok(ApiOk::new(json!({
        "question": question,
    })))
}
