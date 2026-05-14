//! Event management API handlers (Issue 004 — Multi-event / Organizer support).
//!
//! Protected endpoints (require admin auth):
//!   GET    /api/events          — list all events
//!   POST   /api/events          — create a new event
//!   POST   /api/events/seed     — seed first event from env vars (super admin only)
//!   POST   /api/events/migrate  — migrate quiz data from QUIZ to EVENTS namespace (super admin only)
//!   GET    /api/events/{id}     — get event details
//!   PUT    /api/events/{id}     — update event config
//!   DELETE /api/events/{id}     — archive (soft-delete) event

use axum::{
    Extension,
    extract::{Path, State},
    response::Json,
};

use serde_json::json;

use crate::error::ApiOk;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::{CreateEventRequest, UpdateEventRequest};

use crate::state::AppState;

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

    let kv = state.events_kv.as_ref().ok_or_else(|| {
        AppError::Internal(
            "events KV namespace not configured — add EVENTS binding in wrangler.toml".into(),
        )
    })?;

    let all_events = crate::event_store::list_events(kv).await.map_err(|e| {
        tracing::error!(error = %e, "failed to list events");
        AppError::Internal(format!("failed to list events: {e}"))
    })?;

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
        if let Ok(Some(config)) = crate::event_store::get_event_config(kv, &meta.id).await
            && crate::event_store::has_event_access(&config, &claims.email)
        {
            visible.push(meta.clone());
        }
    }

    Ok(ApiOk::new(json!({
        "events": visible,
    })))
}

/// POST /api/events/seed
/// Seed the first event from global env vars.
///
/// Idempotent — if an active event already exists, returns it.
/// Requires SuperAdmin role.
#[worker::send]
pub async fn seed_event(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(staff_email = %claims.email, "seed event requested");

    // Role check: SuperAdmin only
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(AppError::Forbidden("only super admins can seed events".into()).into());
    }

    let kv = state.events_kv.as_ref().ok_or_else(|| {
        AppError::Internal(
            "events KV namespace not configured — add EVENTS binding in wrangler.toml".into(),
        )
    })?;

    let config = crate::event_store::seed_from_config(kv, &state.config, &state)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to seed event");
            AppError::Internal(e.to_string())
        })?;

    tracing::info!(
        event_id = %config.id,
        event_name = %config.name,
        staff_email = %claims.email,
        "event seeded",
    );

    Ok(ApiOk::new(json!({
        "id": config.id,
        "name": config.name,
        "slug": config.slug,
        "status": config.status.as_str(),
    })))
}

/// POST /api/events/migrate
/// Migrate quiz data from legacy QUIZ KV namespace to event-scoped EVENTS KV namespace.
///
/// Reads the "questions" key from QUIZ namespace and copies it to
/// "event:default:quiz:questions" in EVENTS namespace (if not already migrated).
/// SuperAdmin only.
#[worker::send]
pub async fn migrate_quiz(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(staff_email = %claims.email, "quiz migration requested");

    // Role check: SuperAdmin only
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(AppError::Forbidden("only super admins can migrate quiz data".into()).into());
    }

    let events_kv = state.events_kv.as_ref().ok_or_else(|| {
        AppError::Internal(
            "events KV namespace not configured — add EVENTS binding in wrangler.toml".into(),
        )
    })?;

    let quiz_kv = state.quiz_kv.as_ref().ok_or_else(|| {
        AppError::Internal(
            "quiz KV namespace not configured — add QUIZ binding in wrangler.toml".into(),
        )
    })?;

    let result = crate::event_store::migrate_quiz_to_event(events_kv, quiz_kv, "default")
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to migrate quiz data");
            AppError::Internal(e.to_string())
        })?;

    tracing::info!(
        event_id = %result.event_id,
        migrated = %result.migrated,
        staff_email = %claims.email,
        "quiz migration completed",
    );

    Ok(ApiOk::new(json!({
        "migrated": result.migrated,
        "event_id": result.event_id,
        "message": result.message,
    })))
}

/// POST /api/events
/// Create a new event.
///
/// Validates required fields, generates a slug-based ID,
/// and stores both the full config and index entry.
/// Requires SuperAdmin or Organizer role.
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

    let kv = state.events_kv.as_ref().ok_or_else(|| {
        AppError::Internal(
            "events KV namespace not configured — add EVENTS binding in wrangler.toml".into(),
        )
    })?;

    let config = crate::event_store::create_event(kv, &body)
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

    tracing::info!(
        event_id = %config.id,
        event_name = %config.name,
        staff_email = %claims.email,
        "event created",
    );

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &config.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::EventCreated,
            &config.id,
            &format!("event '{}' created", config.name),
        ),
    )
    .await;

    Ok(ApiOk::new(json!({
        "id": config.id,
        "name": config.name,
        "slug": config.slug,
        "status": config.status.as_str(),
        "updated_at": config.updated_at,
    })))
}

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

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("events KV namespace not configured".into()))?;

    let config = crate::event_store::get_event(kv, &id)
        .await
        .map_err(|e| {
            tracing::error!(event_id = %id, error = %e, "failed to get event");
            AppError::Internal(format!("failed to read event: {e}"))
        })?
        .ok_or_else(|| AppError::NotFound(format!("event '{id}' not found")))?;

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

/// PUT /api/events/{id}
/// Update an existing event's configuration.
///
/// Only provided (non-None) fields are updated.
/// Supports partial updates — omit fields you don't want to change.
/// Requires SuperAdmin or Organizer role for this event.
#[worker::send]
pub async fn update_event(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Json(body): Json<UpdateEventRequest>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(event_id = %id, staff_email = %claims.email, "update event requested");

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("events KV namespace not configured".into()))?;

    // Role check: fetch existing event to resolve per-event role
    let existing_event = crate::event_store::get_event(kv, &id)
        .await
        .map_err(|e| {
            tracing::error!(event_id = %id, error = %e, "failed to fetch event for role check");
            AppError::Internal(format!("failed to read event: {e}"))
        })?
        .ok_or_else(|| AppError::NotFound(format!("event '{id}' not found")))?;

    let role = crate::auth::resolve_user_role(&claims.email, &state, Some(&existing_event)).await;
    if role < crate::auth::UserRole::Organizer {
        return Err(AppError::Forbidden(
            "only super admins or organizers can modify events".into(),
        )
        .into());
    }

    let config = crate::event_store::update_event(kv, &id, &body)
        .await
        .map_err(|e| {
            tracing::error!(event_id = %id, error = %e, "failed to update event");
            AppError::Internal(e.to_string())
        })?;

    tracing::info!(
        event_id = %config.id,
        status = %config.status.as_str(),
        staff_email = %claims.email,
        "event updated",
    );

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &config.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::EventUpdated,
            &config.id,
            &format!("event '{}' updated", config.name),
        ),
    )
    .await;

    Ok(ApiOk::new(json!({
        "id": config.id,
        "name": config.name,
        "slug": config.slug,
        "status": config.status.as_str(),
        "updated_at": config.updated_at,
    })))
}

/// DELETE /api/events/{id}
/// Archive (soft-delete) an event.
///
/// Sets status to "archived" — the event is hidden from active listings
/// but data is preserved. The event can be reactivated via PUT with
/// `status: "draft"` or `status: "active"`.
/// Requires SuperAdmin or Organizer role for this event.
#[worker::send]
pub async fn archive_event(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(event_id = %id, staff_email = %claims.email, "archive event requested");

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("events KV namespace not configured".into()))?;

    // Role check: fetch existing event to resolve per-event role
    let existing_event = crate::event_store::get_event(kv, &id)
        .await
        .map_err(|e| {
            tracing::error!(event_id = %id, error = %e, "failed to fetch event for role check");
            AppError::Internal(format!("failed to read event: {e}"))
        })?
        .ok_or_else(|| AppError::NotFound(format!("event '{id}' not found")))?;

    let role = crate::auth::resolve_user_role(&claims.email, &state, Some(&existing_event)).await;
    if role < crate::auth::UserRole::Organizer {
        return Err(AppError::Forbidden(
            "only super admins or organizers can archive events".into(),
        )
        .into());
    }

    crate::event_store::archive_event(kv, &id)
        .await
        .map_err(|e| {
            tracing::error!(event_id = %id, error = %e, "failed to archive event");
            AppError::Internal(e.to_string())
        })?;

    tracing::info!(event_id = %id, staff_email = %claims.email, "event archived");

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::EventArchived,
            &id,
            "event archived",
        ),
    )
    .await;

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

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("events KV namespace not configured".into()))?;

    // Role check: fetch existing event to resolve per-event role
    let existing_event = crate::event_store::get_event(kv, &id)
        .await
        .map_err(|e| {
            tracing::error!(event_id = %id, error = %e, "failed to fetch event for role check");
            AppError::Internal(format!("failed to read event: {e}"))
        })?
        .ok_or_else(|| AppError::NotFound(format!("event '{id}' not found")))?;

    let role = crate::auth::resolve_user_role(&claims.email, &state, Some(&existing_event)).await;
    if role < crate::auth::UserRole::Organizer {
        return Err(AppError::Forbidden(
            "only super admins or organizers can restore events".into(),
        )
        .into());
    }

    crate::event_store::restore_event(kv, &id)
        .await
        .map_err(|e| {
            tracing::error!(event_id = %id, error = %e, "failed to restore event");
            AppError::Internal(e.to_string())
        })?;

    tracing::info!(event_id = %id, staff_email = %claims.email, "event restored from archive");

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::EventRestored,
            &id,
            "event restored from archive",
        ),
    )
    .await;

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
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let force = params.get("force").map(|v| v == "true").unwrap_or(false);
    tracing::info!(event_id = %id, staff_email = %claims.email, force, "hard delete event requested");

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("events KV namespace not configured".into()))?;

    // Role check: SuperAdmin only for permanent deletion
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(
            AppError::Forbidden("only super admins can permanently delete events".into()).into(),
        );
    }

    crate::event_store::hard_delete_event(kv, &id, force)
        .await
        .map_err(|e| {
            tracing::error!(event_id = %id, error = %e, "failed to hard-delete event");
            AppError::Validation(e.to_string())
        })?;

    tracing::info!(event_id = %id, staff_email = %claims.email, force, "event permanently deleted");

    // Audit log — use global log since event KV entry is gone
    let action = if force {
        crate::audit_store::AuditAction::ForceDeleteUsed
    } else {
        crate::audit_store::AuditAction::EventHardDeleted
    };
    let _ = crate::audit_store::append_global_audit(
        kv,
        crate::audit_store::create_entry_with_meta(
            &claims.email,
            action,
            &id,
            "event permanently deleted",
            serde_json::json!({"force": force}),
        ),
    )
    .await;

    Ok(ApiOk::new(json!({
        "id": id,
        "status": "deleted",
    })))
}

/// GET /api/events/{id}/audit — Get audit trail for an event.
/// Returns the last 100 audit entries for the event, newest first.
#[worker::send]
pub async fn get_event_audit(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    // Role check: must be admin or organizer of this event
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("events KV namespace not configured".into()))?;

    let existing_event = crate::event_store::get_event(kv, &id)
        .await
        .map_err(|e| {
            tracing::error!(event_id = %id, error = %e, "failed to fetch event for role check");
            AppError::Internal(format!("failed to read event: {e}"))
        })?
        .ok_or_else(|| AppError::NotFound(format!("event '{id}' not found")))?;

    let role = crate::auth::resolve_user_role(&claims.email, &state, Some(&existing_event)).await;

    if role == crate::auth::UserRole::Staff {
        return Err(
            AppError::Forbidden("only admins and organizers can view audit logs".into()).into(),
        );
    }

    let entries = crate::audit_store::get_event_audit(kv, &id, 100)
        .await
        .map_err(|e| {
            tracing::error!(event_id = %id, error = %e, "failed to get audit log");
            AppError::Internal(e.to_string())
        })?;

    tracing::info!(event_id = %id, count = entries.len(), "audit log retrieved");

    Ok(ApiOk::new(serde_json::json!({
        "event_id": id,
        "entries": entries,
    })))
}
