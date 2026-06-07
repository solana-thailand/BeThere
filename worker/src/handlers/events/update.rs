use axum::Extension;
use axum::extract::{Path, State};
use axum::response::Json;
use serde_json::json;

use crate::error::ApiOk;
use crate::state::AppState;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::UpdateEventRequest;

#[worker::send]
pub async fn update_event(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Json(body): Json<UpdateEventRequest>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(event_id = %id, staff_email = %claims.email, "update event requested");

    let kv = state.events_kv.as_ref();

    // Role check: fetch existing event — KV first, D1 fallback
    let existing_event = if let Some(kv_ref) = kv {
        crate::event_store::get_event(kv_ref, &id)
            .await
            .map_err(|e| {
                tracing::error!(event_id = %id, error = %e, "failed to fetch event for role check");
                AppError::Internal(format!("failed to read event: {e}"))
            })?
    } else {
        None
    };

    let existing_event = match existing_event {
        Some(e) => e,
        None => {
            tracing::info!(event_id = %id, "KV miss, trying D1 for event");
            if let Some(ref d1) = state.d1 {
                crate::db::events::get_event(d1, &id)
                    .await
                    .map_err(|e| {
                        tracing::error!(event_id = %id, error = %e, "D1 get event failed");
                        AppError::Internal(format!("failed to read event from D1: {e}"))
                    })?
                    .map(|row| row.to_event_config())
                    .ok_or_else(|| AppError::NotFound(format!("event '{id}' not found")))?
            } else {
                return Err(AppError::NotFound(format!("event '{id}' not found")).into());
            }
        }
    };

    let role = crate::auth::resolve_user_role(&claims.email, &state, Some(&existing_event)).await;
    if role < crate::auth::UserRole::Organizer {
        return Err(AppError::Forbidden(
            "only super admins or organizers can modify events".into(),
        )
        .into());
    }

    // SEC-ESCROW-RESET: Verify on-chain escrow is actually closed before allowing
    // reset to None. Prevents confusing UI state when KV says "none" but on-chain
    // escrow still holds funds.
    if let Some(ref new_status) = body.escrow_status
        && matches!(
            new_status,
            event_checkin_domain::models::event::EscrowStatus::None
        )
        && matches!(
            existing_event.escrow_status,
            event_checkin_domain::models::event::EscrowStatus::Closed
                | event_checkin_domain::models::event::EscrowStatus::Cancelled
        )
        && !existing_event.escrow_address.is_empty()
        && !existing_event.organizer_wallet.is_empty()
    {
        let on_chain_id = if existing_event.on_chain_event_id != 0 {
            existing_event.on_chain_event_id
        } else {
            crate::handlers::deposit::derive_on_chain_event_id(&existing_event.id)
        };

        let rpc_url = state.config.solana.full_rpc_url();

        match crate::solana_escrow::check_escrow_pda_available(
            &rpc_url,
            &existing_event.organizer_wallet,
            on_chain_id,
        )
        .await
        {
            Ok(_) => {
                tracing::info!(
                    event_id = %id,
                    "escrow PDA confirmed closed on-chain — reset to None allowed"
                );
            }
            Err(e) => {
                tracing::warn!(
                    event_id = %id,
                    error = %e,
                    "escrow PDA still exists on-chain — rejecting reset to None"
                );
                return Err(AppError::Validation(
                    "cannot reset escrow: on-chain escrow account still exists. Close it on-chain first.".to_string()
                ).into());
            }
        }
    }

    // Apply partial update to existing config (works regardless of KV vs D1 source)
    let mut config = existing_event.clone();
    crate::event_store::apply_update(&mut config, &body).map_err(AppError::Validation)?;
    config.updated_by = claims.email.clone();
    config.updated_at = chrono::Utc::now().to_rfc3339();

    // Write to KV (if available, non-fatal)
    if let Some(kv_ref) = kv
        && let Err(e) = crate::event_store::save_event_config(kv_ref, &config).await
    {
        tracing::warn!(event_id = %id, error = %e, "KV write failed for event update");
    }

    // D1 dual-write
    crate::event_store::sync_event_to_d1(state.d1.as_deref(), &config).await;

    // Escrow reverse index dual-write (when escrow address set or changed)
    if !config.escrow_address.is_empty() {
        let _ = crate::event_store::save_escrow_index(
            state.d1.as_deref(),
            kv,
            &config.escrow_address,
            &config.id,
        )
        .await;
    }

    // Update KV index entry (if KV available)
    if let Some(kv_ref) = kv
        && let Ok(mut index) = crate::event_store::get_event_index(kv_ref).await
    {
        if let Some(entry) = index.events.iter_mut().find(|e| e.id == id) {
            *entry = config.to_meta();
        }
        if let Err(e) = crate::event_store::save_event_index(kv_ref, &index).await {
            tracing::warn!(event_id = %id, error = %e, "KV index update failed");
        }
    }

    tracing::info!(
        event_id = %config.id,
        status = %config.status.as_str(),
        staff_email = %claims.email,
        "event updated",
    );

    // Audit log
    if let Some(kv_ref) = kv {
        let _ = crate::audit_store::append_event_audit(
            kv_ref,
            &config.id,
            crate::audit_store::create_entry(
                &claims.email,
                crate::audit_store::AuditAction::EventUpdated,
                &config.id,
                &format!("event '{}' updated", config.name),
            ),
            state.d1.as_deref(),
        )
        .await;

        // Audit log: escrow re-initialized (reset from Closed/Cancelled → None)
        if body.escrow_status == Some(event_checkin_domain::models::event::EscrowStatus::None)
            && matches!(
                existing_event.escrow_status,
                event_checkin_domain::models::event::EscrowStatus::Closed
                    | event_checkin_domain::models::event::EscrowStatus::Cancelled
            )
        {
            let _ = crate::audit_store::append_event_audit(
                kv_ref,
                &config.id,
                crate::audit_store::create_entry(
                    &claims.email,
                    crate::audit_store::AuditAction::EscrowReinitialized,
                    &config.id,
                    &format!(
                        "escrow reset from {} to none — ready for re-initialization",
                        existing_event.escrow_status
                    ),
                ),
                state.d1.as_deref(),
            )
            .await;
        }

        // Sync to Events tab in contacts sheet (non-fatal)
        super::audit::sync_event_to_tab(&state, &config, 0, Some(kv_ref)).await;
    } else if let Some(ref db) = state.d1 {
        super::audit::audit_d1_only(
            db,
            &config.id,
            &claims.email,
            crate::audit_store::AuditAction::EventUpdated,
            &config.id,
            &format!("event '{}' updated", config.name),
            None,
        )
        .await;

        // Escrow re-initialized audit
        if body.escrow_status == Some(event_checkin_domain::models::event::EscrowStatus::None)
            && matches!(
                existing_event.escrow_status,
                event_checkin_domain::models::event::EscrowStatus::Closed
                    | event_checkin_domain::models::event::EscrowStatus::Cancelled
            )
        {
            super::audit::audit_d1_only(
                db,
                &config.id,
                &claims.email,
                crate::audit_store::AuditAction::EscrowReinitialized,
                &config.id,
                &format!(
                    "escrow reset from {} to none — ready for re-initialization",
                    existing_event.escrow_status
                ),
                None,
            )
            .await;
        }

        // Sync to Events tab in contacts sheet (non-fatal)
        super::audit::sync_event_to_tab(&state, &config, 0, None).await;
    }

    Ok(ApiOk::new(json!({
        "id": config.id,
        "name": config.name,
        "slug": config.slug,
        "status": config.status.as_str(),
        "updated_at": config.updated_at,
    })))
}
