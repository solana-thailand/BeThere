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
        staff_fingerprint = %state.log_fingerprint(&claims.email),
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
    let crate::event_store::SavedEvent { config, d1_sync } =
        crate::event_store::create_event(kv, d1, &body, &claims.email)
            .await
            // Invalid input is a 400 with its message, only a storage
            // failure a 500. A slug collision cannot fail: it is deduplicated.
            .map_err(|e| e.logged("create event"))?;

    tracing::info!(
        event_id = %config.id,
        event_name = %config.name,
        staff_fingerprint = %state.log_fingerprint(&claims.email),
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
        "warnings": d1_sync.warnings(),
    })))
}
