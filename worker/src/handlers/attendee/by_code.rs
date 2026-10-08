//! `GET /api/attendees/by-code/{code}` — the scanner's "Enter code" path
//! (`.issues/178`).
//!
//! Staff-only and event-scoped: it sits in the `protected` router (it takes
//! `Extension<Claims>`, which 500s in the public router), and it authorizes the
//! event with `resolve_event_with_access` exactly like `GET /api/attendee/{id}`.
//! It returns **only** the attendee id. The scanner then runs its existing
//! lookup and check-in flow with that id, so this endpoint adds no new data
//! and no new action: a display code never reaches claim or deposit handlers.

use axum::{
    Extension,
    extract::{Path, Query, State},
};
use serde_json::json;

use crate::error::ApiOk;
use event_checkin_domain::models::attendee::DisplayCode;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::handlers::ext::{EventIdQuery, resolve_event_with_access};
use crate::state::AppState;

/// GET /api/attendees/by-code/{code}?event_id=
#[worker::send]
pub async fn get_attendee_by_display_code(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(code): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    // Authorize the event before looking at the code, so a caller without
    // access learns nothing, not even whether the code is well-formed.
    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;

    let code = DisplayCode::parse(&code).map_err(|e| AppError::Validation(e.to_string()))?;

    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("ticket codes need D1".to_string()))?;

    let attendee_id = crate::db::attendees::find_attendee_id_by_display_code(db, &event.id, &code)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "display code lookup failed");
            AppError::Internal("ticket code lookup failed".to_string())
        })?
        .ok_or_else(|| AppError::NotFound("no ticket with that code in this event".to_string()))?;

    tracing::info!(
        staff_fingerprint = %state.log_fingerprint(&claims.email),
        event_id = %event.id,
        "attendee resolved by display code",
    );

    Ok(ApiOk::new(json!({ "attendee_id": attendee_id })))
}
