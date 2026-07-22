//! PUT /api/events/{id}/post-event-registration — toggle post-event lead capture.
//!
//! Plan 008 — Phase 3 §3.3.1. Organizer-gated. Toggles the
//! `post_event_registration_open` flag + optional `post_event_registration_until_ms`
//! deadline on the `events` row (migration 0020). The flag is mirrored onto the
//! KV `EventConfig` + `EventIndex` so the KV-first read path stays consistent.
//!
//! Post-event registration only makes sense for completed events — for an active
//! event, that's just normal registration. The handler rejects non-Completed
//! events and validates `until_ms > now_ms` when a deadline is supplied.

use axum::Extension;
use axum::extract::{Path, State};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::audit_store::{AuditAction, append_event_audit, create_entry_with_meta};
use crate::db::events::set_post_event_registration;
use crate::error::ApiOk;
use crate::state::AppState;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::{EventConfig, EventStatus};

/// PUT /api/events/{id}/post-event-registration request body.
#[derive(Debug, Deserialize)]
pub struct PutPostEventRegistrationRequest {
    /// `true` opens post-event registration (shows the CTA on the recap page);
    /// `false` closes it.
    pub open: bool,
    /// Optional deadline (Unix epoch ms). When `open == true` and `Some`, must be
    /// in the future. `None` = open indefinitely. Ignored when `open == false`.
    #[serde(default)]
    pub until_ms: Option<i64>,
}

/// PUT /api/events/{id}/post-event-registration — toggle + deadline.
///
/// Organizer-gated. Requires `event.status == Completed`. On success mirrors the
/// flag + deadline onto the KV EventConfig + event index and appends an audit entry.
#[worker::send]
pub async fn put_post_event_registration(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    axum::Json(req): axum::Json<PutPostEventRegistrationRequest>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let kv = state.events_kv.as_ref();

    // ── 1. Resolve event + role gate ───────────────────────────────────────
    let event = load_event(&state, kv, &id).await?;
    enforce_organizer(&claims, &state, &event).await?;

    // ── 2. Validate: only completed events can open post-event registration ─
    // Opening lead capture for an active/draft event is nonsensical — that's
    // just normal registration. Archived events are hidden and cannot capture.
    if req.open && event.status != EventStatus::Completed {
        return Err(AppError::Validation(format!(
            "post-event registration can only be opened for completed events (current status: {})",
            event.status.as_str()
        ))
        .into());
    }

    // ── 3. Validate the deadline (when opening with one) ───────────────────
    if req.open
        && let Some(until) = req.until_ms
    {
        let now_ms = Utc::now().timestamp_millis();
        if until <= now_ms {
            return Err(AppError::Validation(format!(
                "post-event registration deadline ({until}) must be in the future (now: {now_ms})"
            ))
            .into());
        }
    }

    // ── 4. Persist the toggle + deadline on the events row ─────────────────
    let db = state.d1.as_deref().ok_or_else(|| {
        AppError::Internal("D1 is required to toggle post-event registration".into())
    })?;

    // Closing clears the deadline (None); opening honours the supplied deadline.
    let effective_until = if req.open { req.until_ms } else { None };
    set_post_event_registration(db, &event.id, req.open, effective_until)
        .await
        .map_err(AppError::Internal)?;

    // ── 5. Sync the KV EventConfig + event index ───────────────────────────
    // D1 is canonical; KV is the fast read path. Without this the KV-first
    // load would return stale `post_event_registration_open: false` after an
    // open, and the public recap page would not show the CTA.
    if let Some(kv_ref) = state.events_kv.as_ref() {
        let mut updated = event.clone();
        updated.post_event_registration_open = req.open;
        updated.post_event_registration_until_ms = effective_until;
        let _ = crate::event_store::save_event_config(kv_ref, &updated).await;

        if let Ok(mut index) = crate::event_store::get_event_index(kv_ref).await {
            let meta = updated.to_meta();
            if let Some(existing) = index.events.iter_mut().find(|e| e.id == meta.id) {
                *existing = meta;
            } else {
                index.events.push(meta);
            }
            let _ = crate::event_store::save_event_index(kv_ref, &index).await;
        }
    }

    // ── 6. Audit ───────────────────────────────────────────────────────────
    let description = format!(
        "event '{}' post-event registration {} by {}",
        event.id,
        if req.open { "opened" } else { "closed" },
        claims.email
    );
    let meta = json!({
        "event_id": event.id,
        "open": req.open,
        "until_ms": effective_until,
    });

    if let Some(kv_ref) = state.events_kv.as_ref() {
        let _ = append_event_audit(
            kv_ref,
            &event.id,
            create_entry_with_meta(
                &claims.email,
                AuditAction::PostEventRegistrationToggled,
                &event.id,
                &description,
                meta,
            ),
            Some(db),
        )
        .await;
    } else {
        super::audit::audit_d1_only(
            db,
            &event.id,
            &claims.email,
            AuditAction::PostEventRegistrationToggled,
            &event.id,
            &description,
            Some(meta),
        )
        .await;
    }

    tracing::info!(
        event_id = %event.id,
        open = req.open,
        until_ms = ?effective_until,
        staff_email = %claims.email,
        "post-event registration toggled"
    );

    Ok(ApiOk::new(json!({
        "id": event.id,
        "open": req.open,
        "until_ms": effective_until,
    })))
}

// ---------------------------------------------------------------------------
// Internal helpers — mirror recap.rs::load_event / enforce_organizer.
// ---------------------------------------------------------------------------

/// Load an event by id, KV first then D1 fallback.
async fn load_event(
    state: &AppState,
    kv: Option<&worker::KvStore>,
    id: &str,
) -> Result<EventConfig, AppError> {
    if let Some(kv_ref) = kv
        && let Ok(Some(c)) = crate::event_store::get_event(kv_ref, id).await
    {
        return Ok(c);
    }
    if let Some(ref d1) = state.d1
        && let Ok(Some(row)) = crate::db::events::get_event(d1, id).await
    {
        return Ok(row.to_event_config());
    }
    Err(AppError::NotFound(format!("event '{id}' not found")))
}

/// Reject non-organizers.
async fn enforce_organizer(
    claims: &Claims,
    state: &AppState,
    event: &EventConfig,
) -> Result<(), AppError> {
    let role = crate::auth::resolve_user_role(&claims.email, state, Some(event)).await;
    if role < crate::auth::UserRole::Organizer {
        return Err(AppError::Forbidden(
            "only super admins or organizers can toggle post-event registration".into(),
        ));
    }
    Ok(())
}
