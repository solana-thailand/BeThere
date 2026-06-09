//! Adventure API handlers for the Rust Adventures feature.
//!
//! Public endpoints:
//!   GET  /api/adventure/{token}/status     — current adventure progress
//!   POST /api/adventure/{token}/save       — save level completion
//!
//! Admin endpoint (protected):
//!   GET  /api/admin/adventure              — get adventure config
//!   PUT  /api/admin/adventure              — create/update adventure config

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use serde_json::json;

use crate::error::ApiOk;

use event_checkin_domain::models::adventure::{AdventureConfig, AdventureSaveRequest};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use super::ext::{EventIdQuery, resolve_event};
use crate::adventure;
use crate::error::WorkerError;
use crate::state::AppState;

/// GET /api/adventure/{token}/status
/// Get adventure status for a claim token.
#[worker::send]
pub async fn get_adventure_status(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let event = resolve_event(&state, query.event_id.as_deref()).await?;
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    let status = adventure::get_adventure_status(db, &event.id, &token)
        .await
        .map_err(AppError::Internal)?;

    let progress = adventure::get_adventure_progress(db, &event.id, &token)
        .await
        .map_err(AppError::Internal)?;

    Ok(ApiOk::new(json!({
        "status": status,
        "progress": progress,
    })))
}

/// POST /api/adventure/{token}/save
/// Save level completion progress.
///
/// Security: validates that the claim token belongs to a checked-in attendee
/// before saving progress. Mirrors the quiz handler's validation pattern.
#[worker::send]
pub async fn save_adventure_progress(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<EventIdQuery>,
    Json(body): Json<AdventureSaveRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    // Validate token matches body
    if body.claim_token != token {
        return Err(AppError::Validation("token mismatch".to_string()).into());
    }

    let event = resolve_event(&state, query.event_id.as_deref()).await?;

    // Verify claim token belongs to a checked-in attendee
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
        Ok(Some(_)) => {} // valid checked-in attendee
        Ok(None) => {
            tracing::warn!("adventure save: invalid claim token {token}");
            return Err(AppError::NotFound(
                "invalid claim token — you must be checked in first".to_string(),
            )
            .into());
        }
        Err(ref e) => {
            tracing::error!("adventure save: failed to look up claim token {token}: {e}");
            return Err(AppError::Internal("failed to verify claim token".to_string()).into());
        }
    }

    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    // Get config to determine required levels
    let config = adventure::get_adventure_config(db, &event.id)
        .await
        .map_err(AppError::Internal)?;

    // Determine required level prefixes from config.
    // required_level is 0-based index: n=2 means levels 01..03 must be completed.
    // If None, all 10 levels must be completed.
    // If adventure is not enabled, pass empty vec to skip passed-check entirely.
    const TOTAL_LEVELS: usize = 10;
    let required_levels: Vec<String> = match &config {
        Some(c) if c.enabled => match c.required_level {
            Some(n) => (1..=(n + 1)).map(|i| format!("{i:02}")).collect(),
            None => (1..=TOTAL_LEVELS).map(|i| format!("{i:02}")).collect(),
        },
        _ => vec![],
    };

    let progress = adventure::save_level_completion(
        db,
        &event.id,
        &token,
        &body.level_id,
        body.score,
        &required_levels,
    )
    .await
    .map_err(AppError::Internal)?;

    Ok(ApiOk::new(json!({
        "progress": progress,
    })))
}

/// GET /api/admin/adventure
/// Get adventure configuration (admin only).
#[worker::send]
pub async fn get_admin_adventure(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!("admin adventure config read by {}", _claims.email);

    let event = resolve_event(&state, query.event_id.as_deref()).await?;
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    let config = adventure::get_adventure_config(db, &event.id)
        .await
        .map_err(AppError::Internal)?;

    Ok(ApiOk::new(json!({
        "event_id": event.id,
        "config": config,
    })))
}

/// PUT /api/admin/adventure
/// Create or update adventure configuration (admin only).
#[worker::send]
pub async fn put_admin_adventure(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
    Json(body): Json<AdventureConfig>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!(
        "admin adventure config update by {} (enabled={})",
        _claims.email,
        body.enabled
    );

    let event = resolve_event(&state, query.event_id.as_deref()).await?;
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    adventure::save_adventure_config(db, &event.id, &body)
        .await
        .map_err(AppError::Internal)?;

    Ok(ApiOk::new(json!({
        "event_id": event.id,
        "enabled": body.enabled,
        "required_level": body.required_level,
    })))
}
