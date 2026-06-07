use axum::Extension;
use axum::extract::State;
use serde_json::json;

use crate::error::ApiOk;
use crate::state::AppState;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

/// GET /api/events
/// List events visible to the current user.
///
/// - **SuperAdmin**: sees all events
/// - **Organizer/Staff**: sees only events they are assigned to
///   (matched by `organizer_emails` or `staff_emails` in event config,
///   or by Google Sheet staff role)
///
/// Returns events sorted by creation date (newest first).
#[worker::send]
pub async fn list_events(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(staff_email = %claims.email, "list events requested");

    let kv = state.events_kv.as_ref();

    let all_events = if let Some(kv_ref) = kv {
        let events = crate::event_store::list_events(kv_ref).await.map_err(|e| {
            tracing::error!(error = %e, "failed to list events");
            AppError::Internal(format!("failed to list events: {e}"))
        })?;
        if !events.is_empty() {
            events
        } else if let Some(ref d1) = state.d1 {
            tracing::info!("KV empty, falling back to D1 for event list");
            crate::db::events::list_events_as_meta(d1)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "D1 list events failed");
                    AppError::Internal(format!("failed to list events from D1: {e}"))
                })?
        } else {
            events
        }
    } else if let Some(ref d1) = state.d1 {
        tracing::info!("no KV, reading event list from D1");
        crate::db::events::list_events_as_meta(d1)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "D1 list events failed");
                AppError::Internal(format!("failed to list events from D1: {e}"))
            })?
    } else {
        return Err(AppError::Internal(
            "events KV namespace not configured — add EVENTS binding in wrangler.toml".into(),
        )
        .into());
    };

    // SuperAdmin sees everything
    if state
        .config
        .super_admin_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(&claims.email))
    {
        return Ok(ApiOk::new(json!({
            "events": all_events,
        })));
    }

    // Organizer/Staff: only see events they are assigned to.
    // EventMeta only has organizer_emails, not staff_emails.
    // We must load full configs to check both lists.
    let mut visible = Vec::new();
    for meta in &all_events {
        // Quick check: organizer_emails is in meta (no need to load full config)
        let in_organizer_list = meta
            .organizer_emails
            .iter()
            .any(|e| e.eq_ignore_ascii_case(&claims.email));

        if in_organizer_list {
            visible.push(meta.clone());
            continue;
        }

        // Slower check: load full config to check staff_emails
        // KV first, then D1 fallback
        if let Some(kv_ref) = kv
            && let Ok(Some(config)) = crate::event_store::get_event_config(kv_ref, &meta.id).await
            && crate::event_store::has_event_access(&config, &claims.email)
        {
            visible.push(meta.clone());
        } else if let Some(ref d1) = state.d1 {
            // D1 fallback for staff check when event is not in KV
            if let Ok(Some(row)) = crate::db::events::get_event(d1, &meta.id).await {
                let config = row.to_event_config();
                if crate::event_store::has_event_access(&config, &claims.email) {
                    visible.push(meta.clone());
                }
            }
        }
    }

    Ok(ApiOk::new(json!({
        "events": visible,
    })))
}
