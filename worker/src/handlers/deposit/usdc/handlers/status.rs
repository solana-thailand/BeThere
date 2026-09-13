//! GET /api/deposit/status/{attendee_id}

use axum::{extract::Path, extract::Query, extract::State};
use event_checkin_domain::models::deposit::DepositStatusResponse;

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::handlers::deposit::usdc::{
    check_and_switch_deadline, check_in_person_capacity, recover_and_verify_deposit,
};
use crate::handlers::ext::EventIdQuery;
use crate::state::AppState;

/// Check deposit status for an attendee.
/// Public endpoint — attendee can check their own status.
#[worker::send]
pub async fn get_deposit_status_handler(
    State(state): State<AppState>,
    Path(attendee_id): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<DepositStatusResponse>, WorkerError> {
    let kv = state.events_kv.as_ref();
    let d1 = state.d1.as_deref();

    let event =
        event_store::resolve_event_or_fallback(kv, query.event_id.as_deref(), &state.config, d1)
            .await
            .map_err(event_checkin_domain::models::error::AppError::from)?;

    let status = event_store::get_deposit_status_with_fallback(kv, d1, &event.id, &attendee_id)
        .await
        .map_err(event_checkin_domain::models::error::AppError::Internal)?;

    // --- Read-path self-heal ---
    // Recover a missing tx_signature from on-chain PDA history and verify the
    // deposit via signer cross-check. This closes the gap where the deposit
    // page's `AlreadyDeposited` view never polls `/confirm` for an existing
    // unverified record — the deposit page self-heals on load instead.
    // Idempotent: returns immediately when already verified.
    let status = match status {
        Some(s) => Some(recover_and_verify_deposit(&state, &event, s).await),
        None => None,
    };

    // --- Deposit deadline edge-trigger ---
    // If event has a deposit_deadline_hours and attendee hasn't deposited,
    // check if deadline passed since registration. If so, auto-switch to Online.
    let mut deadline_expired = false;
    let mut registration_date: Option<String> = None;
    let mut in_person_available: Option<bool> = None;

    if status.is_none() && event.deposit_deadline_hours.is_some() {
        // Look up the attendee to get registration_date and participation_type
        if let Ok(Some(attendee)) = crate::sheets::get_attendee_by_id(
            &attendee_id,
            &state,
            &event.sheet_id,
            &event.sheet_name,
            kv,
        )
        .await
        {
            registration_date = attendee.registration_date.clone();

            // Only auto-switch if currently in-person
            if attendee.is_in_person()
                && let Some(reg_str) = &attendee.registration_date
            {
                deadline_expired =
                    check_and_switch_deadline(&state, kv, &event, &attendee, reg_str).await;
            }

            // Check if in-person capacity is still available (reclaim flow)
            if deadline_expired {
                in_person_available = Some(check_in_person_capacity(&state, &event, kv).await);
            }
        }
    }

    let usdc_deposits_accepted = event.accepts_usdc_deposits();

    // --- Refund window inputs (plan 005 §3.2 — divergence fix #19) ---
    // The on-chain refund instruction enforces a two-path model:
    //   - checked-in attendee → refund window [event_end, ∞)
    //   - no-show attendee     → refund window [event_end, refund_deadline)
    // Surface both inputs so the frontend gate can mirror the on-chain check
    // instead of the previous event_end-only approximation. The on-chain
    // program remains the source of truth; these are advisory inputs for
    // hiding a CTA that would revert.
    //
    // `checked_in` reflects the off-chain check-in state (Google Sheets / D1),
    // which the organizer keeps in sync with the on-chain `mark_checked_in`
    // instruction. Only fetched when a deposit exists — no-show vs checked-in
    // is irrelevant without an on-chain deposit to refund.
    let checked_in = if status.is_some() {
        crate::sheets::get_attendee_by_id(
            &attendee_id,
            &state,
            &event.sheet_id,
            &event.sheet_name,
            kv,
        )
        .await
        .ok()
        .flatten()
        .is_some_and(|a| a.is_checked_in())
    } else {
        false
    };

    // Absolute refund deadline in epoch ms. Mirrors `compute_refund_info` in
    // the frontend and the on-chain invariant `refund_deadline > event_end`.
    // `0` when not configured (legacy/missing data) — frontend gate fails safe.
    let refund_deadline_ms = if event.event_end_ms > 0 && event.refund_deadline_hours > 0 {
        event.event_end_ms + (i64::from(event.refund_deadline_hours) * 3_600_000)
    } else {
        0
    };

    Ok(ApiOk::new(DepositStatusResponse {
        deposit_enabled: event.deposit_enabled,
        deposit_amount_usdc: event.deposit_amount_usdc,
        deposit_amount_thb: event.deposit_amount_thb,
        promptpay_id: event.promptpay_id,
        event_start_ms: event.event_start_ms,
        event_end_ms: event.event_end_ms,
        refund_deadline_hours: event.refund_deadline_hours,
        refund_deadline_ms,
        checked_in,
        event_name: event.name,
        event_tagline: event.tagline,
        event_slug: event.slug,
        status,
        dev_mode: state.config.dev_mode,
        deposit_deadline_hours: event.deposit_deadline_hours,
        deadline_expired,
        registration_date,
        in_person_available,
        usdc_deposits_accepted,
    }))
}
