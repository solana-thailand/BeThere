//! POST/GET /api/events/{id}/summary — post-event summary (Plan 008 — Phase 1).
//!
//! A "summary" is a frozen point-in-time snapshot of the funnel + financials.
//! `GET` is lazy: returns a persisted freeze if one exists, else either freezes
//! on read (event is over) or returns a live preview (event still running).
//! `POST /summary/freeze` is a manual trigger that forces the freeze.

use axum::Extension;
use axum::extract::{Path, State};
use chrono::Utc;
use serde_json::json;

use crate::audit_store::{AuditAction, create_entry_with_meta};
use crate::error::ApiOk;
use crate::state::AppState;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::{EventConfig, EventStatus};

/// GET /api/events/{id}/summary — lazy freeze / preview.
///
/// Flow:
///   1. Load source event (KV first, D1 fallback) — mirrors `restore_event`.
///   2. Role check: Organizer+ for this event.
///   3. Draft events → 400 "event not yet active".
///   4. If a frozen row exists → return it (`frozen: true`).
///   5. If `now_ms >= event_end_ms` → compute + persist + audit; `frozen: true`.
///   6. Else (event still running) → return live preview; `frozen: false`.
#[worker::send]
pub async fn get_event_summary(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let kv = state.events_kv.as_ref();
    let now_ms = Utc::now().timestamp_millis();

    let event = load_event(&state, kv, &id).await?;
    enforce_organizer(&claims, &state, &event).await?;

    if event.status == EventStatus::Draft {
        return Err(AppError::Validation(
            "event is still a draft — activate it before viewing a summary".into(),
        )
        .into());
    }

    // 4. Existing frozen row wins.
    if let Some(db) = state.d1.as_deref()
        && let Ok(Some(frozen)) = crate::db::event_summaries::get_summary(db, &event.id).await {
            return Ok(summary_response(frozen, true));
        }

    // 5. Event is over → freeze on read.
    if now_ms >= event.event_end_ms {
        let frozen = freeze_now(&state, &event, &claims.email, /*manual=*/ false).await?;
        return Ok(summary_response(frozen, true));
    }

    // 6. Still running → live preview (not persisted).
    let preview = compute_live(&state, &event).await?;
    Ok(summary_response(preview, false))
}

/// POST /api/events/{id}/summary/freeze — manual freeze trigger.
///
/// Allowed only once the event has ended (`now_ms >= event_end_ms`) or its
/// status is `Completed`. Freezing an in-progress event would mislead, so we
/// reject with 400 in that case.
#[worker::send]
pub async fn freeze_event_summary(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let kv = state.events_kv.as_ref();
    let now_ms = Utc::now().timestamp_millis();

    let event = load_event(&state, kv, &id).await?;
    enforce_organizer(&claims, &state, &event).await?;

    if event.status == EventStatus::Draft {
        return Err(AppError::Validation("event is still a draft — cannot freeze".into()).into());
    }
    // Guard against freezing an in-progress event.
    let ended = now_ms >= event.event_end_ms || event.status == EventStatus::Completed;
    if !ended {
        return Err(AppError::Validation(format!(
            "event '{}' has not ended yet (ends {}) — cannot freeze early",
            event.id, event.event_end_ms
        ))
        .into());
    }

    let frozen = freeze_now(&state, &event, &claims.email, /*manual=*/ true).await?;
    Ok(summary_response(frozen, true))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Load an event by id, KV first then D1 fallback.
///
/// Mirrors the pattern in `duplicate_event` / `restore_event`.
async fn load_event(
    state: &AppState,
    kv: Option<&worker::KvStore>,
    id: &str,
) -> Result<EventConfig, AppError> {
    if let Some(kv_ref) = kv
        && let Ok(Some(c)) = crate::event_store::get_event(kv_ref, id).await {
            return Ok(c);
        }
    if let Some(ref d1) = state.d1
        && let Ok(Some(row)) = crate::db::events::get_event(d1, id).await {
            return Ok(row.to_event_config());
        }
    Err(AppError::NotFound(format!("event '{id}' not found")))
}

/// Reject non-organizers (Staff cannot view summaries).
async fn enforce_organizer(
    claims: &Claims,
    state: &AppState,
    event: &EventConfig,
) -> Result<(), AppError> {
    let role = crate::auth::resolve_user_role(&claims.email, state, Some(event)).await;
    if role < crate::auth::UserRole::Organizer {
        return Err(AppError::Forbidden(
            "only super admins or organizers can view event summaries".into(),
        ));
    }
    Ok(())
}

/// Compute + persist a freeze, and write an audit entry.
///
/// `manual` flips the audit description wording; both paths use the actor's
/// email as `frozen_by`.
async fn freeze_now(
    state: &AppState,
    event: &EventConfig,
    actor_email: &str,
    manual: bool,
) -> Result<event_checkin_domain::models::event_summary::EventSummary, AppError> {
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 is required to freeze an event summary".into()))?;

    let mut snapshot = crate::db::event_summaries::compute_snapshot(db, event)
        .await
        .map_err(AppError::Internal)?;
    snapshot.frozen_at = Some(Utc::now().to_rfc3339());
    snapshot.frozen_by = actor_email.to_string();

    crate::db::event_summaries::upsert_summary(db, &snapshot, actor_email)
        .await
        .map_err(AppError::Internal)?;

    let description = if manual {
        format!("event '{}' summary frozen by {actor_email}", event.id)
    } else {
        format!("event '{}' summary auto-frozen on read", event.id)
    };
    let meta = json!({ "event_id": event.id, "manual": manual });

    if let Some(kv_ref) = state.events_kv.as_ref() {
        let _ = crate::audit_store::append_event_audit(
            kv_ref,
            &event.id,
            create_entry_with_meta(
                actor_email,
                AuditAction::EventSummaryFrozen,
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
            actor_email,
            AuditAction::EventSummaryFrozen,
            &event.id,
            &description,
            Some(meta),
        )
        .await;
    }

    Ok(snapshot)
}

/// Compute a live (non-frozen) snapshot for the preview path.
async fn compute_live(
    state: &AppState,
    event: &EventConfig,
) -> Result<event_checkin_domain::models::event_summary::EventSummary, AppError> {
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 is required to compute an event summary".into()))?;
    crate::db::event_summaries::compute_snapshot(db, event)
        .await
        .map_err(AppError::Internal)
}

/// Wrap an `EventSummary` into the API response payload with the `frozen` flag.
fn summary_response(
    summary: event_checkin_domain::models::event_summary::EventSummary,
    frozen: bool,
) -> ApiOk<serde_json::Value> {
    ApiOk::new(json!({
        "event_id": summary.event_id,
        "summary": summary,
        "frozen": frozen,
    }))
}
