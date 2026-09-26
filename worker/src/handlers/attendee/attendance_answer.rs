//! `PUT /api/attendee/{id}/attendance-answer` — record what a registrant said
//! when asked whether they can still come (migration 0052).

use axum::{
    Extension,
    extract::{Path, Query, State},
};
use serde_json::json;

use crate::error::ApiOk;
use event_checkin_domain::models::attendee::AttendanceAnswer;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::handlers::ext::{EventIdQuery, resolve_event_with_access};
use crate::state::AppState;

/// Request body. `answer: null` clears a recorded answer.
#[derive(serde::Deserialize)]
pub(crate) struct AttendanceAnswerBody {
    answer: Option<AttendanceAnswer>,
}

/// PUT /api/attendee/{id}/attendance-answer?event_id=
///
/// Staff-only (authed router). D1 is the only store: the answer is roster
/// information for this round of asking, so it is not mirrored to the Sheet.
#[worker::send]
pub async fn set_attendance_answer(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Query(query): Query<EventIdQuery>,
    axum::Json(body): axum::Json<AttendanceAnswerBody>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;
    let Some(db) = state.d1.as_deref() else {
        return Err(AppError::Internal("D1 not configured".to_string()).into());
    };

    let written =
        crate::db::attendance_answers::set(db, &event.id, &id, body.answer, &claims.email)
            .await
            .map_err(AppError::Internal)?;
    // A clear with nothing recorded is also `false`; only a set proves absence.
    if !written && body.answer.is_some() {
        return Err(AppError::NotFound(format!("attendee {id} not found on this event")).into());
    }

    let value = body.answer.map(AttendanceAnswer::as_str);
    if let Some(kv) = &state.events_kv {
        let _ = crate::audit_store::append_event_audit(
            kv,
            &event.id,
            crate::audit_store::create_entry_with_meta(
                &claims.email,
                crate::audit_store::AuditAction::AttendanceAnswerRecorded,
                &id,
                &format!("attendance answer → {}", value.unwrap_or("cleared")),
                json!({ "answer": value }),
            ),
            Some(db),
        )
        .await;
    }

    tracing::info!(
        attendee_id = %id,
        event_id = %event.id,
        answer = value.unwrap_or("cleared"),
        staff_fingerprint = %state.log_fingerprint(&claims.email),
        "attendance answer recorded"
    );

    Ok(ApiOk::new(json!({
        "attendee_id": id,
        "event_id": event.id,
        "attendance_answer": value,
    })))
}
