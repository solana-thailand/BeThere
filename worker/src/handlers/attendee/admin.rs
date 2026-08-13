//! Admin maintenance endpoints — cache flush + claim-token repair.

use axum::{
    Extension,
    extract::{Query, State},
};
use serde_json::json;

use crate::error::ApiOk;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::handlers::ext::{EventIdQuery, resolve_event_with_access, resolve_kv};
use crate::sheets;
use crate::state::AppState;

/// POST /api/admin/flush-cache
/// Flush server-side column mapping cache for an event.
/// Use after changing sheet structure or headers.
///
/// Phase 2d: attendee list cache removed — only column map cache remains.
#[worker::send]
pub async fn flush_cache(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!("flushing column map cache (requested by: {})", claims.email);

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;
    let kv = resolve_kv(&state);

    sheets::invalidate_column_map_cache(kv, &event.sheet_id, &event.sheet_name).await;

    Ok(ApiOk::new(json!({
        "flushed": true,
        "event_id": event.id,
        "sheet_id": event.sheet_id,
    })))
}

// ---------------------------------------------------------------------------
// POST /api/admin/repair-claim-tokens — backfill empty D1 claim_tokens
// ---------------------------------------------------------------------------

/// Repair D1 attendees with empty or NULL claim_tokens.
///
/// Generates a UUID v7 for each affected row. Idempotent — rows with valid
/// tokens are untouched. SuperAdmin only.
#[worker::send]
pub async fn repair_claim_tokens(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(staff_email = %claims.email, "claim token repair requested");

    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(AppError::Forbidden("only super admins can repair claim tokens".into()).into());
    }

    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    let repaired = crate::db::attendees::repair_empty_claim_tokens(db)
        .await
        .map_err(AppError::Internal)?;

    tracing::info!(repaired, "claim token repair complete");

    Ok(ApiOk::new(json!({
        "repaired": repaired,
    })))
}

/// POST /api/admin/test-alert
/// Super-admin only. Sends a test message to the configured Slack webhook so
/// alerting can be verified end-to-end without waiting for a real 5xx.
#[worker::send]
pub async fn test_alert(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(AppError::Forbidden("super admin only".into()).into());
    }

    let webhook = state.config.slack_webhook_url.clone();
    if webhook.is_empty() {
        return Ok(ApiOk::new(json!({
            "sent": false,
            "reason": "SLACK_WEBHOOK_URL is not set",
        })));
    }

    let text = format!("✅ BeThere test alert — requested by {}", claims.email);
    match crate::middleware::alert::post_slack(&webhook, &text).await {
        Ok(()) => Ok(ApiOk::new(json!({ "sent": true }))),
        Err(e) => Ok(ApiOk::new(json!({ "sent": false, "error": e }))),
    }
}
