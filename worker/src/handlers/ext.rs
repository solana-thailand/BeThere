//! Shared handler utilities: extractors, event resolution, and access guards.

use serde::Deserialize;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::EventConfig;

use crate::event_store;
use crate::state::AppState;

/// Optional event_id query parameter for event-scoped requests.
/// Shared across all handlers that accept optional event scoping.
#[derive(Debug, Clone, Deserialize)]
pub struct EventIdQuery {
    pub event_id: Option<String>,
}

/// Query params for attendee list with cursor-based pagination.
#[derive(Debug, Clone, Deserialize)]
pub struct AttendeesQuery {
    pub event_id: Option<String>,
    /// Cursor: row_index of the last attendee in the previous page.
    /// None means start from the beginning.
    #[serde(default)]
    pub cursor: Option<usize>,
    /// Page size limit. Default: 200, max: 200.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Resolve an event from query params and verify access.
///
/// Returns `AppError::Unauthorized` if the user has no access to the event,
/// `AppError::Internal` if event resolution fails.
pub async fn resolve_event_with_access(
    state: &AppState,
    claims: &Claims,
    event_id: Option<&str>,
) -> Result<EventConfig, AppError> {
    let event = event_store::resolve_event_or_fallback(
        state.events_kv.as_ref(),
        event_id,
        &state.config,
        state.d1.as_deref(),
    )
    .await
    .map_err(AppError::from)?;

    if let Err(e) = crate::auth::check_event_access(&claims.email, state, &event).await {
        tracing::warn!(
            staff_fingerprint = %state.log_fingerprint(&claims.email),
            event_id = %event.id,
            event_name = %event.name,
            "access denied: actor has no access to event",
        );
        return Err(AppError::Forbidden(e));
    }

    Ok(event)
}

/// Resolve event without auth check (for public endpoints like claim).
pub async fn resolve_event(
    state: &AppState,
    event_id: Option<&str>,
) -> Result<EventConfig, AppError> {
    event_store::resolve_event_or_fallback(
        state.events_kv.as_ref(),
        event_id,
        &state.config,
        state.d1.as_deref(),
    )
    .await
    .map_err(AppError::from)
}

/// Helper to get the KV store from state (events_kv or quiz_kv fallback).
pub fn resolve_kv(state: &AppState) -> Option<&worker::KvStore> {
    state.events_kv.as_ref().or(state.quiz_kv.as_ref())
}

/// Attendee lookup for unauthenticated routes. D1 first; a miss falls back to
/// reading the whole Google Sheet only while the caller's per-IP budget lasts
/// (plan 028 W7), otherwise `AppError::RateLimited`. The Sheet stays reachable
/// because rows added to it directly reach D1 only after a manual sync.
pub async fn get_attendee_for_public(
    state: &AppState,
    headers: &axum::http::HeaderMap,
    attendee_id: &str,
    event: &EventConfig,
    kv: Option<&worker::KvStore>,
) -> Result<Option<event_checkin_domain::models::attendee::Attendee>, AppError> {
    if let Some(attendee) = crate::sheets::get_attendee_by_id_from_d1(attendee_id, state).await {
        return Ok(Some(attendee));
    }
    if !crate::middleware::rate_limit::allow_sheets_fallback(state, headers).await {
        tracing::warn!(attendee_id = %attendee_id, "public lookup: Sheets fallback rate-limited");
        return Err(AppError::RateLimited(
            "too many unknown attendee lookups".into(),
        ));
    }
    crate::sheets::get_attendee_by_id_from_sheets(
        attendee_id,
        state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
    )
    .await
    .map_err(AppError::Internal)
}
