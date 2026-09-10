//! GET /api/events/{id}/pr-pack — generate copy-pasteable marketing copy.
//!
//! Deterministic, no persistence, no external API calls (Plan 008 §3.4).
//! Generation lives in `domain::pr_pack::generate` so it's unit-testable
//! without a worker harness. This handler is a thin auth + load wrapper that
//! delegates entirely to the domain layer.
//!
//! Why organizer-gated: the PR pack surfaces deposit terms, organizer emails,
//! and the claim URL — internal-facing context. Attendees get the public
//! event page (`/e/{slug}`), not the PR pack.

use axum::Extension;
use axum::extract::{Path, State};
use chrono::Utc;
use serde_json::json;

use crate::error::ApiOk;
use crate::state::AppState;

use super::common::{enforce_organizer, load_event};

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::pr_pack;

/// GET /api/events/{id}/pr-pack — generate the PR pack for an event.
///
/// Returns a `PrPack` (headline, blurb, social post, calendar text, email
/// snippet, deposit terms, organizer list) plus `generated_at` and
/// `source_config_version` (the event's `updated_at`) so the UI can show
/// whether the pack reflects the latest edit.
#[worker::send]
pub async fn get_pr_pack(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let kv = state.events_kv.as_ref();

    // ── 1. Resolve event + role gate ───────────────────────────────────────
    let event = load_event(&state, kv, &id).await?;
    enforce_organizer(&claims, &state, &event, "view the PR pack").await?;

    // ── 2. Generate (pure function — no I/O, deterministic) ────────────────
    let pack = pr_pack::generate(&event);

    tracing::info!(
        event_id = %event.id,
        staff_email = %claims.email,
        "pr pack generated"
    );

    // ── 3. Return ──────────────────────────────────────────────────────────
    Ok(ApiOk::new(json!({
        "event_id": event.id,
        "pack": pack,
        "generated_at": Utc::now().to_rfc3339(),
        "source_config_version": event.updated_at,
    })))
}

// ---------------------------------------------------------------------------
// Internal helpers — identical to `recap.rs` / `summary.rs`. Kept local per
// the same rationale (avoid premature coupling; extract to `events::common`
// only if a fourth consumer appears).
// ---------------------------------------------------------------------------
