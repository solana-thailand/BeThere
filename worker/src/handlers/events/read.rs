use axum::Extension;
use axum::extract::{Path, State};
use serde_json::json;

use crate::error::ApiOk;
use crate::state::AppState;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

/// GET /api/events/{id}
/// Get full configuration for a single event.
///
/// Access rules:
/// - **SuperAdmin**: can view any event
/// - **Organizer/Staff**: can only view events they are assigned to
#[worker::send]
pub async fn get_event(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(event_id = %id, staff_email = %claims.email, "get event requested");

    let config = crate::event_store::resolve_event_or_fallback(
        state.events_kv.as_ref(),
        Some(&id),
        &state.config,
        state.d1.as_deref(),
    )
    .await
    .map_err(|e| {
        tracing::error!(event_id = %id, error = %e, "failed to get event");
        AppError::Internal(format!("failed to read event: {e}"))
    })?;

    // Access check: non-super_admin must be assigned to this event
    let is_super_admin = state
        .config
        .super_admin_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(&claims.email));

    if !is_super_admin && !crate::event_store::has_event_access(&config, &claims.email) {
        tracing::warn!(
            staff_email = %claims.email,
            event_id = %config.id,
            event_name = %config.name,
            "get event denied: no access",
        );
        return Err(AppError::Forbidden(format!("you do not have access to event '{id}'")).into());
    }

    Ok(ApiOk::new(json!({
        "event": config,
    })))
}
