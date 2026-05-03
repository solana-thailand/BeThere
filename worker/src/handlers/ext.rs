//! Shared handler utilities: extractors, event resolution, and access guards.

use axum::response::Json;
use serde::Deserialize;
use serde_json::json;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::event::EventConfig;

use crate::event_store;
use crate::state::AppState;

/// Optional event_id query parameter for event-scoped requests.
/// Shared across all handlers that accept optional event scoping.
#[derive(Debug, Clone, Deserialize)]
pub struct EventIdQuery {
    pub event_id: Option<String>,
}

/// Resolve an event from query params and verify access.
///
/// This replaces the repeated pattern in handlers:
/// ```ignore
/// let event = event_store::resolve_event_or_fallback(...).await?;
/// crate::auth::check_event_access(&claims.email, &state, &event).await?;
/// ```
pub async fn resolve_event_with_access(
    state: &AppState,
    claims: &Claims,
    event_id: Option<&str>,
) -> Result<EventConfig, Json<serde_json::Value>> {
    let event =
        event_store::resolve_event_or_fallback(state.events_kv.as_ref(), event_id, &state.config)
            .await
            .map_err(|e| Json(json!({ "success": false, "error": e })))?;

    if let Err(e) = crate::auth::check_event_access(&claims.email, state, &event).await {
        tracing::warn!(
            "access denied: {} has no access to event '{}' ({})",
            claims.email,
            event.name,
            event.id,
        );
        return Err(Json(json!({ "success": false, "error": e })));
    }

    Ok(event)
}

/// Resolve event without auth check (for public endpoints like claim).
pub async fn resolve_event(
    state: &AppState,
    event_id: Option<&str>,
) -> Result<EventConfig, Json<serde_json::Value>> {
    event_store::resolve_event_or_fallback(state.events_kv.as_ref(), event_id, &state.config)
        .await
        .map_err(|e| Json(json!({ "success": false, "error": e })))
}

/// Helper to get the KV store from state (events_kv or quiz_kv fallback).
pub fn resolve_kv(state: &AppState) -> Option<&worker::KvStore> {
    state.events_kv.as_ref().or(state.quiz_kv.as_ref())
}
