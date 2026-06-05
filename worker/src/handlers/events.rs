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
//!   POST   /api/events/reseed-kv — reseed KV index from D1 (super admin only)

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

    // D1 dual-write (non-fatal)
    crate::event_store::sync_event_to_d1(state.d1.as_deref(), &config).await;

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

/// POST /api/events/reseed-kv
/// Rebuild the KV event index and configs from D1.
/// SuperAdmin only. Idempotent — overwrites KV entries with D1 data.
#[worker::send]
pub async fn reseed_kv_from_d1(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(staff_email = %claims.email, "reseed KV from D1 requested");

    // Role check: SuperAdmin only
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(AppError::Forbidden("only super admins can reseed KV from D1".into()).into());
    }

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("events KV namespace not configured".into()))?;
    let d1 = state
        .d1
        .as_ref()
        .ok_or_else(|| AppError::Internal("D1 database not configured".into()))?;

    let count = crate::event_store::seed_kv_from_d1(kv, d1)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to reseed KV from D1");
            AppError::Internal(e.to_string())
        })?;

    tracing::info!(count, staff_email = %claims.email, "KV reseeded from D1");

    Ok(ApiOk::new(json!({
        "synced": count,
        "message": format!("{count} events synced from D1 to KV"),
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
        audit_d1_only(
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
    sync_event_to_tab(&state, &config, 0, kv).await;

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
        sync_event_to_tab(&state, &config, 0, Some(kv_ref)).await;
    } else if let Some(ref db) = state.d1 {
        audit_d1_only(
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
            audit_d1_only(
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
        sync_event_to_tab(&state, &config, 0, None).await;
    }

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
        audit_d1_only(
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
        audit_d1_only(
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
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
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
            if let Some(kv_ref) = kv {
                crate::org_store::resolve_contacts_sheet(kv_ref, config, &state.config.sheets).await
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
        audit_d1_only(
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

/// GET /api/events/{id}/audit — Get audit trail for an event.
/// Returns the last 100 audit entries for the event, newest first.
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Non-fatal sync of an event to the Events tab in the contacts Google Sheet.
///
/// Called after event create/update. Errors are logged but never block the
/// main operation.
async fn sync_event_to_tab(
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
async fn audit_d1_only(
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
