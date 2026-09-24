//! Issuing the ticket QR — the one thing that actually lets somebody in.
//!
//! Until now this lived inside `slip_verify.rs`, written out twice (once for
//! the `wait_until` path and once for the blocking fallback). That duplication
//! is the reason this module exists rather than a third copy: a guard added to
//! one state-transition path while its sibling keeps the old behaviour is the
//! recurring defect in this codebase, and the comp action (`.issues/129` Gap 1)
//! needs to issue exactly the same QR as an approval.
//!
//! The important consequence is a product one. Receiving this QR is the *only*
//! thing that distinguishes an admitted attendee from a pending one — and
//! before `.issues/129` Gap 1 the only route to it was approving a payment
//! slip, which is simultaneously the promise to refund ฿500. Separating "let
//! them in" from "we owe them money" starts here.

use event_checkin_domain::models::attendee::{Attendee, ColumnMapping};
use event_checkin_domain::models::event::EventConfig;

use crate::state::AppState;

/// Give an attendee their ticket QR, unless they already have one.
///
/// Idempotent by inspection: an attendee who already has a QR is left alone, so
/// calling this from both approval and comp cannot mint a second URL or
/// overwrite one an organizer set by hand.
///
/// **Writes to D1 inline and to the Sheet in the background.** The D1 write is
/// not detachable: the public ticket page reads D1 first, so deferring it shows
/// a verified attendee a page with no QR on it. The Sheet is a legacy mirror
/// and may lag. Both failures are logged and non-fatal — refusing to admit
/// somebody because a spreadsheet write failed would be the worse outcome.
pub(super) async fn issue_ticket_qr_if_absent(
    state: &AppState,
    kv: &worker::KvStore,
    event: &EventConfig,
    attendee: &Attendee,
    mapping: &ColumnMapping,
) {
    if attendee.qr_code_url.as_ref().is_some_and(|u| !u.is_empty()) {
        return;
    }

    let qr_url = format!(
        "{}/staff/?scan={}",
        state.config.server.url, attendee.api_id
    );

    if let Some(d1) = state.d1.as_deref()
        && let Err(e) = crate::db::attendees::set_qr_url(d1, &attendee.api_id, &qr_url).await
    {
        tracing::warn!(
            attendee_id = %attendee.api_id,
            error = %e,
            "D1 set_qr_url failed — the ticket page will show no QR until this is retried"
        );
    }

    match &state.worker_ctx {
        Some(wctx) => wctx.wait_until(crate::sheets::bg_sync::update_qr_urls(
            state.clone(),
            vec![(attendee.row_index, qr_url)],
            mapping.clone(),
            event.sheet_id.clone(),
            event.sheet_name.clone(),
            Some(kv.clone()),
        )),
        // No fetch context (tests, scheduled handlers): write through.
        None => {
            if let Err(e) = crate::sheets::write::update_qr_urls(
                &[(attendee.row_index, qr_url)],
                mapping,
                state,
                &event.sheet_id,
                &event.sheet_name,
                Some(kv),
            )
            .await
            {
                tracing::warn!(
                    attendee_id = %attendee.api_id,
                    error = %e,
                    "failed to mirror the ticket QR to the sheet (non-fatal)"
                );
            }
        }
    }
}
