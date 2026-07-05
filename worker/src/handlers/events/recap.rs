//! PUT  /api/events/{id}/recap — author + publish/unpublish the public recap.
//! GET  /api/events/{id}/recap — fetch the current draft/published recap.
//!
//! Recap content (markdown + hero image) lives on the `event_summaries` row
//! alongside the frozen snapshot. Authoring requires a frozen summary to exist
//! — the recap columns are co-located on that row, and "recaps without numbers
//! are misleading" (Plan 008 §3.2.1). The denormalized `events.recap_published`
//! flag is mirrored on every publish/unpublish so `GET /api/public/events/past`
//! can filter without a join.

use axum::Extension;
use axum::extract::{Path, State};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::audit_store::{AuditAction, append_event_audit, create_entry_with_meta};
use crate::db::event_summaries::{MAX_RECAP_MARKDOWN_BYTES, get_recap, set_recap};
use crate::db::events::set_recap_published_flag;
use crate::error::ApiOk;
use crate::state::AppState;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::EventConfig;

/// PUT /api/events/{id}/recap request body.
#[derive(Debug, Deserialize)]
pub struct PutRecapRequest {
    /// Markdown body (≤ 16 KB). Empty string clears existing content.
    pub recap_markdown: String,
    /// Hero image URL. Must be `https://` when non-empty. Empty = no image.
    pub recap_image_url: String,
    /// `true` publishes (sets `recap_published_at = now`); `false` saves as
    /// draft (clears `recap_published_at`, unpublishing a live recap).
    pub publish: bool,
}

/// PUT /api/events/{id}/recap — author + publish/unpublish the public recap.
///
/// Organizer-gated. Requires a frozen `event_summaries` row to exist (the
/// recap columns are co-located on that row, and recaps without numbers are
/// misleading). On publish, mirrors the `recap_published` flag onto the
/// events row so the public past-events listing can filter without a join.
#[worker::send]
pub async fn put_recap(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    axum::Json(req): axum::Json<PutRecapRequest>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let kv = state.events_kv.as_ref();

    // ── 1. Resolve event + role gate ───────────────────────────────────────
    let event = load_event(&state, kv, &id).await?;
    enforce_organizer(&claims, &state, &event).await?;

    // ── 2. Validate body ───────────────────────────────────────────────────
    if req.recap_markdown.len() > MAX_RECAP_MARKDOWN_BYTES {
        return Err(AppError::Validation(format!(
            "recap_markdown exceeds {} bytes (got {})",
            MAX_RECAP_MARKDOWN_BYTES,
            req.recap_markdown.len()
        ))
        .into());
    }
    if !req.recap_image_url.is_empty() && !req.recap_image_url.starts_with("https://") {
        return Err(AppError::Validation(
            "recap_image_url must be an https:// URL when non-empty".into(),
        )
        .into());
    }
    // Publishing an empty recap would surface a blank public page — reject.
    if req.publish && req.recap_markdown.trim().is_empty() {
        return Err(
            AppError::Validation("cannot publish a recap with empty markdown".into()).into(),
        );
    }

    // ── 3. Require a frozen summary row (recap columns live on it) ─────────
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 is required to author a recap".into()))?;

    let _existing = get_recap(db, &event.id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| {
            AppError::Validation(
                "no frozen summary exists for this event — freeze the summary first".into(),
            )
        })?;

    // ── 4. Persist recap content + publish state ───────────────────────────
    let published_at = if req.publish {
        Some(Utc::now().to_rfc3339())
    } else {
        None
    };
    set_recap(
        db,
        &event.id,
        &req.recap_markdown,
        &req.recap_image_url,
        published_at.as_deref(),
    )
    .await
    .map_err(AppError::Internal)?;

    // ── 5. Mirror the denormalized flag on the events row ──────────────────
    set_recap_published_flag(db, &event.id, req.publish)
        .await
        .map_err(AppError::Internal)?;

    // ── 5b. Sync the KV EventConfig + event index ──────────────────────────
    // The D1 update above only touches the events table. The KV-side
    // EventConfig and EventIndex also store `recap_published` (mirrored via
    // `to_meta`), so we update them in-place to keep both stores consistent.
    // Without this, the KV-first read path (`get_event_config_with_fallback`)
    // would return stale `recap_published: false` even after a publish, and
    // the public past-events listing's KV fallback would miss the event.
    if let Some(kv_ref) = state.events_kv.as_ref() {
        let mut updated = event.clone();
        updated.recap_published = req.publish;
        let _ = crate::event_store::save_event_config(kv_ref, &updated).await;

        // Best-effort index update. Failure here is non-fatal — D1 is the
        // canonical source and the next index reseed (`reseed_kv_from_d1`)
        // would reconcile. We log via the ignored `_` to avoid noisy traces
        // on transient KV errors.
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
    let action = if req.publish {
        AuditAction::EventRecapPublished
    } else {
        AuditAction::EventRecapUnpublished
    };
    let description = if req.publish {
        format!("event '{}' recap published by {}", event.id, claims.email)
    } else {
        format!(
            "event '{}' recap saved as draft by {}",
            event.id, claims.email
        )
    };
    let meta = json!({
        "event_id": event.id,
        "publish": req.publish,
        "markdown_bytes": req.recap_markdown.len(),
        "has_image": !req.recap_image_url.is_empty(),
    });

    if let Some(kv_ref) = state.events_kv.as_ref() {
        let _ = append_event_audit(
            kv_ref,
            &event.id,
            create_entry_with_meta(&claims.email, action, &event.id, &description, meta),
            Some(db),
        )
        .await;
    } else {
        super::audit::audit_d1_only(
            db,
            &event.id,
            &claims.email,
            action,
            &event.id,
            &description,
            Some(meta),
        )
        .await;
    }

    tracing::info!(
        event_id = %event.id,
        publish = req.publish,
        markdown_bytes = req.recap_markdown.len(),
        staff_email = %claims.email,
        "recap authored"
    );

    // ── 7. Return the freshly-persisted recap state ────────────────────────
    let updated = get_recap(db, &event.id)
        .await
        .map_err(AppError::Internal)?
        .unwrap_or_default();

    Ok(ApiOk::new(json!({
        "id": event.id,
        "recap": updated,
        "publish": req.publish,
    })))
}

/// GET /api/events/{id}/recap — fetch the current recap (draft or published).
///
/// Organizer-gated. Returns the recap slice of the `event_summaries` row.
/// When no summary row exists yet, returns an empty draft with `event_id` set
/// and a `summary_frozen: false` flag so the UI can prompt the organizer to
/// freeze the summary before authoring the recap.
#[worker::send]
pub async fn get_recap_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let kv = state.events_kv.as_ref();
    let event = load_event(&state, kv, &id).await?;
    enforce_organizer(&claims, &state, &event).await?;

    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 is required to read a recap".into()))?;

    let existing = get_recap(db, &event.id).await.map_err(AppError::Internal)?;

    let (recap, summary_frozen) = match existing {
        Some(r) => (json!(r), true),
        None => (
            json!({
                "event_id": event.id,
                "recap_markdown": "",
                "recap_image_url": "",
                "recap_published_at": null,
                "frozen_at": null,
            }),
            false,
        ),
    };

    Ok(ApiOk::new(json!({
        "id": event.id,
        "recap": recap,
        "summary_frozen": summary_frozen,
    })))
}

// ---------------------------------------------------------------------------
// Internal helpers — mirror the `summary.rs` load_event / enforce_organizer.
// Kept local to avoid coupling the modules; if a third consumer appears, move
// these into a shared `events::common` module.
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

/// Reject non-organizers (Staff cannot author recaps).
async fn enforce_organizer(
    claims: &Claims,
    state: &AppState,
    event: &EventConfig,
) -> Result<(), AppError> {
    let role = crate::auth::resolve_user_role(&claims.email, state, Some(event)).await;
    if role < crate::auth::UserRole::Organizer {
        return Err(AppError::Forbidden(
            "only super admins or organizers can author event recaps".into(),
        ));
    }
    Ok(())
}
