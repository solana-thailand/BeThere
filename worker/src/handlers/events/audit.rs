use crate::error::ApiOk;
use crate::state::AppState;
use axum::Extension;
use axum::extract::{Path, State};
use axum::response::Json;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::RegistrationFormConfig;

/// GET /api/events/{id}/audit — Get audit trail for a specific event.
/// Returns the last 100 audit entries, newest first.
/// Admins and organizers only (Staff cannot view audit logs).
#[worker::send]
pub async fn get_event_audit(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
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

    if role == crate::auth::UserRole::Staff {
        return Err(
            AppError::Forbidden("only admins and organizers can view audit logs".into()).into(),
        );
    }

    // Read audit entries from KV (if available) + D1
    let entries = if let Some(kv_ref) = kv {
        crate::audit_store::get_event_audit(kv_ref, &id, 100, state.d1.as_deref())
            .await
            .map_err(|e| {
                tracing::error!(event_id = %id, error = %e, "failed to get audit log");
                AppError::Internal(e.to_string())
            })?
    } else if let Some(ref d1) = state.d1 {
        // D1-only: read audit from D1 directly, convert to AuditEntry
        let rows = crate::db::audit::get_audit_entries(d1, &id, 100)
            .await
            .map_err(|e| {
                tracing::error!(event_id = %id, error = %e, "failed to get audit log from D1");
                AppError::Internal(e.to_string())
            })?;
        rows.into_iter()
            .filter_map(|r| {
                let action: Option<crate::audit_store::AuditAction> =
                    serde_json::from_str(&format!("\"{}\"", r.action)).ok();
                Some(crate::audit_store::AuditEntry {
                    timestamp: r.timestamp,
                    actor: r.actor,
                    action: action?,
                    target: r.target,
                    description: r.description,
                    metadata: r.metadata.and_then(|s| serde_json::from_str(&s).ok()),
                })
            })
            .collect()
    } else {
        vec![]
    };

    tracing::info!(event_id = %id, count = entries.len(), "audit log retrieved");

    Ok(ApiOk::new(serde_json::json!({
        "event_id": id,
        "entries": entries,
    })))
}

/// GET /api/audit/global — Get system-wide audit trail.
/// Returns the last 200 global audit entries, newest first.
/// SuperAdmin only.
#[worker::send]
pub async fn get_global_audit(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    // Role check: SuperAdmin only for global audit
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(
            AppError::Forbidden("only super admins can view global audit logs".into()).into(),
        );
    }

    let kv = state.events_kv.as_ref();
    let d1 = state.d1.as_deref();

    let entries = if let Some(kv_ref) = kv {
        crate::audit_store::get_global_audit(kv_ref, 200, d1)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed to get global audit log");
                AppError::Internal(e.to_string())
            })?
    } else if let Some(db) = d1 {
        // D1-only path: read global audit entries directly
        crate::db::audit::get_global_audit_entries(db, 200)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed to get global audit log from D1");
                AppError::Internal(e.to_string())
            })?
            .into_iter()
            .filter_map(|r| {
                let action: Option<crate::audit_store::AuditAction> =
                    serde_json::from_str(&format!("\"{}\"", r.action)).ok();
                Some(crate::audit_store::AuditEntry {
                    timestamp: r.timestamp,
                    actor: r.actor,
                    action: action?,
                    target: r.target,
                    description: r.description,
                    metadata: r.metadata.and_then(|s| serde_json::from_str(&s).ok()),
                })
            })
            .collect()
    } else {
        vec![]
    };

    tracing::info!(count = entries.len(), "global audit log retrieved");

    Ok(ApiOk::new(serde_json::json!({
        "entries": entries,
    })))
}

/// GET /api/events/{id}/form-config
///
/// Returns the registration form configuration for an event.
/// If no custom config is stored in KV, returns the default config.
#[worker::send]
pub async fn get_form_config(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(event_id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let config = if let Some(kv) = &state.events_kv {
        crate::event_store::get_form_config(kv, &event_id)
            .await
            .map_err(AppError::Internal)?
    } else {
        None
    };

    let form_config = config.unwrap_or_else(RegistrationFormConfig::default_config);
    let value = serde_json::to_value(&form_config)
        .map_err(|e| AppError::Internal(format!("failed to serialize form config: {e}")))?;

    Ok(ApiOk::new(value))
}

/// PUT /api/events/{id}/form-config
///
/// Saves the registration form configuration for an event.
#[worker::send]
pub async fn put_form_config(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(event_id): Path<String>,
    Json(body): Json<RegistrationFormConfig>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("KV namespace not available".into()))?;

    // Validate that all profile_field keys map to known developer_profiles columns
    let valid_profile_keys = [
        "experience_level",
        "tech_stack",
        "interests",
        "github_handle",
        "discord_handle",
        "twitter_handle",
        "primary_role",
        "learning_goals",
        "company_org",
        "location_city",
    ];
    for field in &body.fields {
        if field.profile_field && !valid_profile_keys.contains(&field.key.as_str()) {
            return Err(AppError::Validation(format!(
                "unknown profile field key: '{}' — valid keys: {}",
                field.key,
                valid_profile_keys.join(", ")
            ))
            .into());
        }
    }

    crate::event_store::save_form_config(kv, &event_id, &body)
        .await
        .map_err(AppError::Internal)?;

    tracing::info!(event_id = %event_id, "form config updated");

    let value = serde_json::to_value(&body)
        .map_err(|e| AppError::Internal(format!("failed to serialize form config: {e}")))?;

    Ok(ApiOk::new(value))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Non-fatal sync of an event to the Events tab in the contacts Google Sheet.
///
/// Called after event create/update. Errors are logged but never block the
/// main operation.
pub(super) async fn sync_event_to_tab(
    state: &AppState,
    config: &event_checkin_domain::models::event::EventConfig,
    total_attendees: usize,
    kv: Option<&worker::KvStore>,
) {
    let resolved = if let Some(kv_ref) = kv {
        crate::org_store::resolve_contacts_sheet(kv_ref, config, &state.config.sheets).await
    } else {
        event_checkin_domain::models::org::ResolvedContactsSheet {
            sheet_id: state.config.sheets.contacts_sheet_id.clone(),
            contacts_sheet_name: state.config.sheets.contacts_sheet_name.clone(),
            events_sheet_name: state.config.sheets.events_sheet_name.clone(),
        }
    };

    if resolved.sheet_id.is_empty() {
        return;
    }

    if let Err(e) = crate::sheets::events_tab::upsert_event_tab(
        config,
        total_attendees,
        state,
        &resolved.sheet_id,
        &resolved.events_sheet_name,
        kv,
    )
    .await
    {
        tracing::warn!(
            event_id = %config.id,
            error = %e,
            "failed to sync event to Events tab (non-fatal)"
        );
    }
}

/// Write an audit entry to D1 directly (for when KV is unavailable).
/// Non-fatal — errors are logged but not propagated.
pub(super) async fn audit_d1_only(
    db: &worker::D1Database,
    event_id: &str,
    email: &str,
    action: crate::audit_store::AuditAction,
    target: &str,
    description: &str,
    metadata: Option<serde_json::Value>,
) {
    let action_str = serde_json::to_string(&action)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string();
    let metadata_str = metadata.as_ref().map(|v| v.to_string());
    if let Err(e) = crate::db::append_audit(
        db,
        event_id,
        email,
        &action_str,
        target,
        description,
        metadata_str.as_deref(),
    )
    .await
    {
        tracing::warn!(event_id, error = %e, "D1-only audit write failed");
    }
}
