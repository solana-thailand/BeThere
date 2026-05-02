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
    Extension,
    extract::{Path, Query, State},
    response::Json,
};

use serde_json::json;

use event_checkin_domain::models::adventure::{AdventureConfig, AdventureSaveRequest};
use event_checkin_domain::models::auth::Claims;

use crate::adventure;
use crate::event_store;
use crate::state::AppState;

use super::quiz::EventIdQuery;

/// GET /api/adventure/{token}/status
/// Get adventure status for a claim token.
#[worker::send]
pub async fn get_adventure_status(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Json<serde_json::Value> {
    let event = match event_store::resolve_event_or_fallback(
        state.events_kv.as_ref(),
        query.event_id.as_deref(),
        &state.config,
    )
    .await
    {
        Ok(e) => e,
        Err(e) => return Json(json!({ "success": false, "error": e })),
    };

    let kv = match state.events_kv.as_ref().or(state.quiz_kv.as_ref()) {
        Some(kv) => kv,
        None => {
            return Json(json!({
                "success": false,
                "error": "KV storage not available",
            }));
        }
    };

    let status = match adventure::get_adventure_status(kv, &event.id, &token).await {
        Ok(s) => s,
        Err(e) => return Json(json!({ "success": false, "error": e })),
    };

    let progress = match adventure::get_adventure_progress(kv, &event.id, &token).await {
        Ok(p) => p,
        Err(e) => return Json(json!({ "success": false, "error": e })),
    };

    Json(json!({
        "success": true,
        "data": {
            "status": status,
            "progress": progress,
        }
    }))
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
) -> Json<serde_json::Value> {
    // Validate token matches body
    if body.claim_token != token {
        return Json(json!({ "success": false, "error": "token mismatch" }));
    }

    let event = match event_store::resolve_event_or_fallback(
        state.events_kv.as_ref(),
        query.event_id.as_deref(),
        &state.config,
    )
    .await
    {
        Ok(e) => e,
        Err(e) => return Json(json!({ "success": false, "error": e })),
    };

    // Verify claim token belongs to a checked-in attendee
    match crate::sheets::get_attendee_by_claim_token(
        &token,
        &state,
        &event.sheet_id,
        &event.sheet_name,
    )
    .await
    {
        Ok(Some(_)) => {} // valid checked-in attendee
        Ok(None) => {
            tracing::warn!("adventure save: invalid claim token {token}");
            return Json(json!({
                "success": false,
                "error": "invalid claim token — you must be checked in first",
            }));
        }
        Err(ref e) => {
            tracing::error!("adventure save: failed to look up claim token {token}: {e}");
            return Json(json!({
                "success": false,
                "error": "failed to verify claim token",
            }));
        }
    }

    let kv = match state.events_kv.as_ref().or(state.quiz_kv.as_ref()) {
        Some(kv) => kv,
        None => {
            return Json(json!({
                "success": false,
                "error": "KV storage not available",
            }));
        }
    };

    // Get config to determine required levels
    let config = match adventure::get_adventure_config(kv, &event.id).await {
        Ok(c) => c,
        Err(e) => return Json(json!({ "success": false, "error": e })),
    };

    // Determine required level IDs from config.
    // If `required_level` is set (e.g., 1), the attendee must complete level_01 through that level.
    // If not set, they must complete all levels.
    let required_levels: Vec<String> = match &config {
        Some(c) if c.enabled => {
            match c.required_level {
                Some(n) => (1..=n).map(|i| format!("level_{i:02}")).collect(),
                None => vec![], // empty = all levels required (handled in save_level_completion)
            }
        }
        _ => vec![],
    };

    let progress = match adventure::save_level_completion(
        kv,
        &event.id,
        &token,
        &body.level_id,
        body.score,
        &required_levels,
    )
    .await
    {
        Ok(p) => p,
        Err(e) => return Json(json!({ "success": false, "error": e })),
    };

    Json(json!({
        "success": true,
        "data": {
            "progress": progress,
        }
    }))
}

/// GET /api/admin/adventure
/// Get adventure configuration (admin only).
#[worker::send]
pub async fn get_admin_adventure(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Json<serde_json::Value> {
    tracing::info!("admin adventure config read by {}", _claims.email);

    let event = match event_store::resolve_event_or_fallback(
        state.events_kv.as_ref(),
        query.event_id.as_deref(),
        &state.config,
    )
    .await
    {
        Ok(e) => e,
        Err(e) => return Json(json!({ "success": false, "error": e })),
    };

    let kv = match state.events_kv.as_ref().or(state.quiz_kv.as_ref()) {
        Some(kv) => kv,
        None => {
            return Json(json!({
                "success": false,
                "error": "KV storage not available",
            }));
        }
    };

    let config = match adventure::get_adventure_config(kv, &event.id).await {
        Ok(c) => c,
        Err(e) => return Json(json!({ "success": false, "error": e })),
    };

    Json(json!({
        "success": true,
        "data": {
            "event_id": event.id,
            "config": config,
        }
    }))
}

/// PUT /api/admin/adventure
/// Create or update adventure configuration (admin only).
#[worker::send]
pub async fn put_admin_adventure(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
    Json(body): Json<AdventureConfig>,
) -> Json<serde_json::Value> {
    tracing::info!(
        "admin adventure config update by {} (enabled={})",
        _claims.email,
        body.enabled
    );

    let event = match event_store::resolve_event_or_fallback(
        state.events_kv.as_ref(),
        query.event_id.as_deref(),
        &state.config,
    )
    .await
    {
        Ok(e) => e,
        Err(e) => return Json(json!({ "success": false, "error": e })),
    };

    let kv = match state.events_kv.as_ref().or(state.quiz_kv.as_ref()) {
        Some(kv) => kv,
        None => {
            return Json(json!({
                "success": false,
                "error": "KV storage not available",
            }));
        }
    };

    if let Err(e) = adventure::save_adventure_config(kv, &event.id, &body).await {
        return Json(json!({ "success": false, "error": e }));
    }

    Json(json!({
        "success": true,
        "data": {
            "event_id": event.id,
            "enabled": body.enabled,
            "required_level": body.required_level,
        }
    }))
}
