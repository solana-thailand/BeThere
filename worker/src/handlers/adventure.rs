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

    // Determine required level IDs (for now, use configured required_level or empty)
    let required_levels: Vec<String> = match &config {
        Some(_c) => {
            // Will be populated by level data — for now empty means all must be done
            vec![]
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
