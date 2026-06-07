use axum::Extension;
use axum::extract::State;
use axum::response::Json;
use serde_json::json;

use crate::error::ApiOk;
use crate::state::AppState;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::CreateEventRequest;

#[worker::send]
pub async fn create_event(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateEventRequest>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(
        staff_email = %claims.email,
        event_name = %body.name,
        "create event requested",
    );

    // Role check: SuperAdmin or Organizer required
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role < crate::auth::UserRole::Organizer {
        return Err(AppError::Forbidden(
            "only super admins or organizers can create events".into(),
        )
        .into());
    }

    let kv = state.events_kv.as_ref();
    let d1 = state.d1.as_deref();

    // Require at least D1 or KV to create an event
    if kv.is_none() && d1.is_none() {
        return Err(AppError::Internal(
            "no storage configured — need D1 database or EVENTS KV binding".into(),
        )
        .into());
    }

    // Build the event config, optionally writing to KV index
    let config = crate::event_store::create_event(kv, d1, &body, &claims.email)
        .await
        .map_err(|e| {
            let err_msg = e.to_string();
            // Duplicate slug is a validation error (409), not internal (500)
            if err_msg.contains("already exists") {
                tracing::warn!(error = %err_msg, "create event rejected: duplicate slug");
                AppError::Validation(err_msg)
            } else {
                tracing::error!(error = %err_msg, "failed to create event");
                AppError::Internal(err_msg)
            }
        })?;

    // D1 is primary — always write (sync_event_to_d1 is non-fatal)
    crate::event_store::sync_event_to_d1(d1, &config).await;

    tracing::info!(
        event_id = %config.id,
        event_name = %config.name,
        staff_email = %claims.email,
        "event created",
    );

    // Audit log — write to D1 (always) + KV (if available)
    if let Some(kv_ref) = kv {
        let _ = crate::audit_store::append_event_audit(
            kv_ref,
            &config.id,
            crate::audit_store::create_entry(
                &claims.email,
                crate::audit_store::AuditAction::EventCreated,
                &config.id,
                &format!("event '{}' created", config.name),
            ),
            d1,
        )
        .await;
    } else if let Some(db) = d1 {
        super::audit::audit_d1_only(
            db,
            &config.id,
            &claims.email,
            crate::audit_store::AuditAction::EventCreated,
            &config.id,
            &format!("event '{}' created", config.name),
            None,
        )
        .await;
    }

    // Sync to Events tab in contacts sheet (non-fatal)
    super::audit::sync_event_to_tab(&state, &config, 0, kv).await;

    Ok(ApiOk::new(json!({
        "id": config.id,
        "name": config.name,
        "slug": config.slug,
        "status": config.status.as_str(),
        "updated_at": config.updated_at,
    })))
}
