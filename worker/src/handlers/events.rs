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

use event_checkin_domain::models::auth::Claims;
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
) -> Json<serde_json::Value> {
    tracing::info!("list events requested by {}", claims.email);

    let kv = match state.events_kv {
        Some(ref kv) => kv,
        None => {
            return Json(json!({
                "success": false,
                "error": "events KV namespace not configured — add EVENTS binding in wrangler.toml",
            }));
        }
    };

    let all_events = match crate::event_store::list_events(kv).await {
        Ok(events) => events,
        Err(e) => {
            tracing::error!("failed to list events: {e}");
            return Json(json!({
                "success": false,
                "error": format!("failed to list events: {e}"),
            }));
        }
    };

    // SuperAdmin sees everything
    if state
        .config
        .super_admin_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(&claims.email))
    {
        return Json(json!({
            "success": true,
            "data": {
                "events": all_events,
            },
        }));
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

    Json(json!({
        "success": true,
        "data": {
            "events": visible,
        },
    }))
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
) -> Json<serde_json::Value> {
    tracing::info!("seed event requested by {}", claims.email);

    // Role check: SuperAdmin only
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Json(json!({
            "success": false,
            "error": "only super admins can seed events",
        }));
    }

    let kv = match state.events_kv {
        Some(ref kv) => kv,
        None => {
            return Json(json!({
                "success": false,
                "error": "events KV namespace not configured — add EVENTS binding in wrangler.toml",
            }));
        }
    };

    match crate::event_store::seed_from_config(kv, &state.config, &state).await {
        Ok(config) => {
            tracing::info!(
                "event seeded: id={} name='{}' by {}",
                config.id,
                config.name,
                claims.email,
            );
            Json(json!({
                "success": true,
                "data": {
                    "id": config.id,
                    "name": config.name,
                    "slug": config.slug,
                    "status": config.status.as_str(),
                },
            }))
        }
        Err(e) => {
            tracing::error!("failed to seed event: {e}");
            Json(json!({
                "success": false,
                "error": format!("{e}"),
            }))
        }
    }
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
) -> Json<serde_json::Value> {
    tracing::info!("quiz migration requested by {}", claims.email);

    // Role check: SuperAdmin only
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Json(json!({
            "success": false,
            "error": "only super admins can migrate quiz data",
        }));
    }

    let events_kv = match state.events_kv {
        Some(ref kv) => kv,
        None => {
            return Json(json!({
                "success": false,
                "error": "events KV namespace not configured — add EVENTS binding in wrangler.toml",
            }));
        }
    };

    let quiz_kv = match state.quiz_kv {
        Some(ref kv) => kv,
        None => {
            return Json(json!({
                "success": false,
                "error": "quiz KV namespace not configured — add QUIZ binding in wrangler.toml",
            }));
        }
    };

    match crate::event_store::migrate_quiz_to_event(events_kv, quiz_kv, "default").await {
        Ok(result) => {
            tracing::info!(
                "quiz migration: event_id={} migrated={} by {}",
                result.event_id,
                result.migrated,
                claims.email,
            );
            Json(json!({
                "success": true,
                "data": {
                    "migrated": result.migrated,
                    "event_id": result.event_id,
                    "message": result.message,
                },
            }))
        }
        Err(e) => {
            tracing::error!("failed to migrate quiz data: {e}");
            Json(json!({
                "success": false,
                "error": format!("{e}"),
            }))
        }
    }
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
) -> Json<serde_json::Value> {
    tracing::info!(
        "create event requested by {} — name='{}'",
        claims.email,
        body.name,
    );

    // Role check: SuperAdmin or Organizer required
    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role < crate::auth::UserRole::Organizer {
        return Json(json!({
            "success": false,
            "error": "only super admins or organizers can create events",
        }));
    }

    let kv = match state.events_kv {
        Some(ref kv) => kv,
        None => {
            return Json(json!({
                "success": false,
                "error": "events KV namespace not configured — add EVENTS binding in wrangler.toml",
            }));
        }
    };

    match crate::event_store::create_event(kv, &body).await {
        Ok(config) => {
            tracing::info!(
                "event created: id={} name='{}' by {}",
                config.id,
                config.name,
                claims.email,
            );
            Json(json!({
                "success": true,
                "data": {
                    "id": config.id,
                    "name": config.name,
                    "slug": config.slug,
                    "status": config.status.as_str(),
                },
            }))
        }
        Err(e) => {
            tracing::error!("failed to create event: {e}");
            Json(json!({
                "success": false,
                "error": format!("{e}"),
            }))
        }
    }
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
) -> Json<serde_json::Value> {
    tracing::info!("get event '{}' requested by {}", id, claims.email);

    let kv = match state.events_kv {
        Some(ref kv) => kv,
        None => {
            return Json(json!({
                "success": false,
                "error": "events KV namespace not configured",
            }));
        }
    };

    let config = match crate::event_store::get_event(kv, &id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            return Json(json!({
                "success": false,
                "error": format!("event '{id}' not found"),
            }));
        }
        Err(e) => {
            tracing::error!("failed to get event '{id}': {e}");
            return Json(json!({
                "success": false,
                "error": format!("failed to read event: {e}"),
            }));
        }
    };

    // Access check: non-super_admin must be assigned to this event
    let is_super_admin = state
        .config
        .super_admin_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(&claims.email));

    if !is_super_admin && !crate::event_store::has_event_access(&config, &claims.email) {
        tracing::warn!(
            "get event denied: {} has no access to event '{}' ({})",
            claims.email,
            config.name,
            config.id,
        );
        return Json(json!({
            "success": false,
            "error": format!("you do not have access to event '{id}'"),
        }));
    }

    Json(json!({
        "success": true,
        "data": {
            "event": config,
        },
    }))
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
) -> Json<serde_json::Value> {
    tracing::info!("update event '{}' requested by {}", id, claims.email);

    let kv = match state.events_kv {
        Some(ref kv) => kv,
        None => {
            return Json(json!({
                "success": false,
                "error": "events KV namespace not configured",
            }));
        }
    };

    // Role check: fetch existing event to resolve per-event role
    let existing_event = match crate::event_store::get_event(kv, &id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            return Json(json!({
                "success": false,
                "error": format!("event '{id}' not found"),
            }));
        }
        Err(e) => {
            tracing::error!("failed to fetch event '{id}' for role check: {e}");
            return Json(json!({
                "success": false,
                "error": format!("failed to read event: {e}"),
            }));
        }
    };

    let role = crate::auth::resolve_user_role(&claims.email, &state, Some(&existing_event)).await;
    if role < crate::auth::UserRole::Organizer {
        return Json(json!({
            "success": false,
            "error": "only super admins or organizers can modify events",
        }));
    }

    match crate::event_store::update_event(kv, &id, &body).await {
        Ok(config) => {
            tracing::info!(
                "event updated: id={} status={} by {}",
                config.id,
                config.status.as_str(),
                claims.email,
            );
            Json(json!({
                "success": true,
                "data": {
                    "id": config.id,
                    "name": config.name,
                    "slug": config.slug,
                    "status": config.status.as_str(),
                    "updated_at": config.updated_at,
                },
            }))
        }
        Err(e) => {
            tracing::error!("failed to update event '{id}': {e}");
            Json(json!({
                "success": false,
                "error": format!("{e}"),
            }))
        }
    }
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
) -> Json<serde_json::Value> {
    tracing::info!("archive event '{}' requested by {}", id, claims.email);

    let kv = match state.events_kv {
        Some(ref kv) => kv,
        None => {
            return Json(json!({
                "success": false,
                "error": "events KV namespace not configured",
            }));
        }
    };

    // Role check: fetch existing event to resolve per-event role
    let existing_event = match crate::event_store::get_event(kv, &id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            return Json(json!({
                "success": false,
                "error": format!("event '{id}' not found"),
            }));
        }
        Err(e) => {
            tracing::error!("failed to fetch event '{id}' for role check: {e}");
            return Json(json!({
                "success": false,
                "error": format!("failed to read event: {e}"),
            }));
        }
    };

    let role = crate::auth::resolve_user_role(&claims.email, &state, Some(&existing_event)).await;
    if role < crate::auth::UserRole::Organizer {
        return Json(json!({
            "success": false,
            "error": "only super admins or organizers can archive events",
        }));
    }

    match crate::event_store::archive_event(kv, &id).await {
        Ok(()) => {
            tracing::info!("event archived: id={id} by {}", claims.email);
            Json(json!({
                "success": true,
                "data": {
                    "id": id,
                    "status": "archived",
                },
            }))
        }
        Err(e) => {
            tracing::error!("failed to archive event '{id}': {e}");
            Json(json!({
                "success": false,
                "error": format!("{e}"),
            }))
        }
    }
}
