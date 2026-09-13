//! The single writer of a *virtual* check-in.
//!
//! `checked_in_at` is the branch condition for badge claiming, refund eligibility
//! and every attendance number the dashboards report, and the claim mint path
//! reads a set `checked_in_at` as proof that an approval-gated path produced it.
//! Two self-serve paths can set it without a staff scan — the adventure
//! quest-complete endpoint and the claim mint auto check-in — and plan 022 §2
//! found them carrying different gates. Both now go through
//! [`commit_virtual_check_in`], so the gate set has one home.
//!
//! The staff scan (`handlers::checkin`) is deliberately *not* a caller: it writes
//! `checked_in_by = <staff email>` plus a claim token, a different transition. It
//! runs the same domain gate directly.

use worker::KvStore;

use event_checkin_domain::models::attendee::{Attendee, ColumnMapping};
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::EventConfig;

use crate::state::AppState;

/// Gate, then record a virtual check-in in D1 and mirror it to Sheets.
///
/// Returns the timestamp written, so the caller can update its in-memory copy
/// without a re-read.
///
/// Gates, in order:
/// 1. the event has an online track — otherwise there is nothing to check into;
/// 2. [`Attendee::can_check_in_virtually`] — not already checked in, and
///    approved. Quest completion is the *caller's* gate: the two callers verify
///    different quests.
///
/// D1 is written synchronously and first: `my-registration` reads D1-first, so a
/// Sheets-only write leaves the frontend polling a `checked_in_at` that is still
/// NULL. Both writes are non-fatal — a check-in that reached D1 is durable, and
/// the Sheets mirror is reconciled by the next sync.
pub async fn commit_virtual_check_in(
    state: &AppState,
    attendee: &Attendee,
    event: &EventConfig,
    mapping: &ColumnMapping,
    kv: Option<&KvStore>,
    claim_token: &str,
) -> Result<String, AppError> {
    if !event.event_format.has_online() {
        tracing::warn!(
            attendee_id = %attendee.api_id,
            event_format = %event.event_format,
            "virtual check-in denied: event has no online track",
        );
        return Err(AppError::Validation(
            "online check-in not available for this event format".into(),
        ));
    }

    if let Err(e) = attendee.can_check_in_virtually() {
        tracing::warn!(
            attendee_id = %attendee.api_id,
            approval_status = %attendee.approval_status,
            error = %e,
            "virtual check-in denied",
        );
        return Err(AppError::Validation(e.to_string()));
    }

    let timestamp = chrono::Utc::now().to_rfc3339();

    if let Some(ref d1) = state.d1 {
        match crate::db::attendees::check_in_attendee(
            d1,
            &attendee.api_id,
            &timestamp,
            "virtual",
            claim_token,
        )
        .await
        {
            Ok(()) => tracing::info!(
                attendee_id = %attendee.api_id,
                checked_in_at = %timestamp,
                "D1 virtual check-in written",
            ),
            Err(e) => {
                tracing::warn!(error = %e, "D1 virtual check-in failed (non-fatal)")
            }
        }
    }

    match &state.worker_ctx {
        Some(ctx) => ctx.wait_until(crate::sheets::bg_sync::mark_virtual_checked_in(
            state.clone(),
            attendee.row_index,
            mapping.clone(),
            event.sheet_id.clone(),
            event.sheet_name.clone(),
            kv.cloned(),
            timestamp.clone(),
        )),
        // No wait_until available (tests) — write through synchronously.
        None => {
            if let Err(e) = crate::sheets::write::mark_virtual_checked_in(
                attendee.row_index,
                mapping,
                state,
                &event.sheet_id,
                &event.sheet_name,
                kv,
            )
            .await
            {
                tracing::error!(error = %e, "virtual check-in sheet write failed (non-fatal)");
            }
        }
    }

    Ok(timestamp)
}
