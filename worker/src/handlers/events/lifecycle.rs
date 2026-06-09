use axum::Extension;
use axum::extract::{Path, Query, State};
use serde_json::json;

use crate::error::ApiOk;
use crate::state::AppState;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

/// DELETE /api/events/{id} — archive (soft-delete) event.
///
/// Only works on non-archived events. Requires SuperAdmin or Organizer role.
#[worker::send]
pub async fn archive_event(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(event_id = %id, staff_email = %claims.email, "archive event requested");

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
            "only super admins or organizers can archive events".into(),
        )
        .into());
    }

    // Apply archive status change
    let mut config = existing_event.clone();

    // SEC-004: Block archive if escrow is active on-chain
    if config.escrow_status.is_active() {
        return Err(AppError::Validation(format!(
            "cannot archive event with active on-chain escrow (status: {}) — close escrow first",
            config.escrow_status
        ))
        .into());
    }

    config.status = event_checkin_domain::models::event::EventStatus::Archived;
    config.updated_by = claims.email.clone();
    config.updated_at = chrono::Utc::now().to_rfc3339();

    // Write to KV (if available, non-fatal)
    if let Some(kv_ref) = kv {
        crate::event_store::archive_event(kv_ref, &id)
            .await
            .map_err(|e| {
                tracing::error!(event_id = %id, error = %e, "failed to archive event in KV");
                AppError::Internal(e.to_string())
            })?;
    }

    // D1 dual-write
    crate::event_store::sync_event_to_d1(state.d1.as_deref(), &config).await;

    tracing::info!(event_id = %id, staff_email = %claims.email, "event archived");

    // Audit log
    if let Some(kv_ref) = kv {
        let _ = crate::audit_store::append_event_audit(
            kv_ref,
            &id,
            crate::audit_store::create_entry(
                &claims.email,
                crate::audit_store::AuditAction::EventArchived,
                &id,
                "event archived",
            ),
            state.d1.as_deref(),
        )
        .await;
    } else if let Some(ref db) = state.d1 {
        super::audit::audit_d1_only(
            db,
            &id,
            &claims.email,
            crate::audit_store::AuditAction::EventArchived,
            &id,
            "event archived",
            None,
        )
        .await;
    }

    Ok(ApiOk::new(json!({
        "id": id,
        "status": "archived",
    })))
}

/// POST /api/events/{id}/restore
/// Restore an archived event back to Draft status.
///
/// Only works on events currently in Archived status.
/// Requires SuperAdmin or Organizer role for this event.
#[worker::send]
pub async fn restore_event(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(event_id = %id, staff_email = %claims.email, "restore event requested");

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
            "only super admins or organizers can restore events".into(),
        )
        .into());
    }

    if existing_event.status != event_checkin_domain::models::event::EventStatus::Archived {
        return Err(AppError::Validation(format!(
            "event '{id}' is not archived (current status: {}) — only archived events can be restored",
            existing_event.status.as_str()
        ))
        .into());
    }

    // Write to KV (if available, non-fatal)
    if let Some(kv_ref) = kv {
        crate::event_store::restore_event(kv_ref, &id)
            .await
            .map_err(|e| {
                tracing::error!(event_id = %id, error = %e, "failed to restore event in KV");
                AppError::Internal(e.to_string())
            })?;
    }

    // D1 dual-write
    let mut config = existing_event.clone();
    config.status = event_checkin_domain::models::event::EventStatus::Draft;
    config.updated_by = claims.email.clone();
    config.updated_at = chrono::Utc::now().to_rfc3339();
    crate::event_store::sync_event_to_d1(state.d1.as_deref(), &config).await;

    tracing::info!(event_id = %id, staff_email = %claims.email, "event restored from archive");

    // Audit log
    if let Some(kv_ref) = kv {
        let _ = crate::audit_store::append_event_audit(
            kv_ref,
            &id,
            crate::audit_store::create_entry(
                &claims.email,
                crate::audit_store::AuditAction::EventRestored,
                &id,
                "event restored from archive",
            ),
            state.d1.as_deref(),
        )
        .await;
    } else if let Some(ref db) = state.d1 {
        super::audit::audit_d1_only(
            db,
            &id,
            &claims.email,
            crate::audit_store::AuditAction::EventRestored,
            &id,
            "event restored from archive",
            None,
        )
        .await;
    }

    Ok(ApiOk::new(json!({
        "id": id,
        "status": "draft",
    })))
}

/// DELETE /api/events/{id}/delete — permanently delete an archived event.
///
/// Only works on Archived events. This is irreversible and frees the slug.
#[worker::send]
pub async fn hard_delete_event(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let force = params.get("force").map(|v| v == "true").unwrap_or(false);
    tracing::info!(event_id = %id, staff_email = %claims.email, force, "hard delete event requested");

    let kv = state.events_kv.as_ref();

    // Role check: SuperAdmin only for permanent deletion
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(
            AppError::Forbidden("only super admins can permanently delete events".into()).into(),
        );
    }

    // Load event config BEFORE deletion — KV first, D1 fallback
    let pre_delete_config = if let Some(kv_ref) = kv {
        crate::event_store::get_event_config(kv_ref, &id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let pre_delete_config = match pre_delete_config {
        Some(c) => Some(c),
        None => {
            if let Some(ref d1) = state.d1 {
                crate::db::events::get_event(d1, &id)
                    .await
                    .ok()
                    .flatten()
                    .map(|row| row.to_event_config())
            } else {
                None
            }
        }
    };

    // KV delete (if available)
    if let Some(kv_ref) = kv {
        crate::event_store::hard_delete_event(kv_ref, &id, force)
            .await
            .map_err(|e| {
                tracing::error!(event_id = %id, error = %e, "failed to hard-delete event from KV");
                AppError::Validation(e.to_string())
            })?;
    }

    // D1 dual-delete (non-fatal)
    crate::event_store::sync_delete_event_from_d1(state.d1.as_deref(), &id).await;

    // Validate event exists somewhere (KV or D1) — prevent deleting non-existent events
    if pre_delete_config.is_none() && kv.is_none() {
        return Err(AppError::NotFound(format!("event '{id}' not found")).into());
    }

    // Clean up Events tab in contacts sheet (non-fatal)
    let resolved = match &pre_delete_config {
        Some(config) => {
            if let Some(db) = state.d1.as_deref() {
                crate::org_store::resolve_contacts_sheet(db, config, &state.config.sheets).await
            } else {
                event_checkin_domain::models::org::ResolvedContactsSheet {
                    sheet_id: state.config.sheets.contacts_sheet_id.clone(),
                    contacts_sheet_name: state.config.sheets.contacts_sheet_name.clone(),
                    events_sheet_name: state.config.sheets.events_sheet_name.clone(),
                }
            }
        }
        None => event_checkin_domain::models::org::ResolvedContactsSheet {
            sheet_id: state.config.sheets.contacts_sheet_id.clone(),
            contacts_sheet_name: state.config.sheets.contacts_sheet_name.clone(),
            events_sheet_name: state.config.sheets.events_sheet_name.clone(),
        },
    };

    if !resolved.sheet_id.is_empty() {
        let res = crate::sheets::events_tab::delete_event_tab(
            &id,
            &state,
            &resolved.sheet_id,
            &resolved.events_sheet_name,
            kv,
        )
        .await;
        if let Err(e) = res {
            tracing::warn!(
                event_id = %id,
                error = %e,
                "failed to clean up Events tab after hard delete (non-fatal)"
            );
        }
    }

    tracing::info!(event_id = %id, staff_email = %claims.email, force, "event permanently deleted");

    // Audit log — use global log since event KV entry is gone
    let action = if force {
        crate::audit_store::AuditAction::ForceDeleteUsed
    } else {
        crate::audit_store::AuditAction::EventHardDeleted
    };
    if let Some(kv_ref) = kv {
        let _ = crate::audit_store::append_global_audit(
            kv_ref,
            crate::audit_store::create_entry_with_meta(
                &claims.email,
                action,
                &id,
                "event permanently deleted",
                serde_json::json!({"force": force}),
            ),
            state.d1.as_deref(),
        )
        .await;
    } else if let Some(ref db) = state.d1 {
        super::audit::audit_d1_only(
            db,
            "__global__",
            &claims.email,
            action,
            &id,
            "event permanently deleted",
            Some(serde_json::json!({"force": force})),
        )
        .await;
    }

    Ok(ApiOk::new(json!({
        "id": id,
        "status": "deleted",
    })))
}
