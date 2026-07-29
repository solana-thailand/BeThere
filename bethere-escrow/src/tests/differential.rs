//! Differential tests: run the REAL program and assert its accept/reject verdict
//! matches the `reference_oracle` truth table (`is_valid_transition`).
//!
//! The oracle's own tests only check it against hand-written expectations — nothing
//! previously compared the oracle's verdict to what the deployed bytecode actually does.
//! This harness closes that gap for the lifecycle actions whose accept/reject is purely
//! state + clock-phase gated and needs no token transfer or instruction introspection:
//! `mark_checked_in` and `deactivate_event`. (deposit / refund / claim_forfeited /
//! rollover / close move tokens or use introspection pairing; they have dedicated
//! adversarial tests and are candidates for a future harness extension.)
//!
//! Designing this harness surfaced one real oracle↔program divergence — see
//! `documented_divergence_checkin_ignores_settled` below.

use super::reference_oracle::{
    is_valid_transition, ClockPhase, EscrowConfig, EscrowState, ACTION_CREATE_EVENT,
    ACTION_DEACTIVATE_EVENT, ACTION_DEPOSIT, ACTION_MARK_CHECKED_IN, ACTION_REFUND,
};
use super::*;

/// A concrete timestamp inside each coarse clock phase (relative to EVENT_END / REFUND_DEADLINE).
fn warp_for(phase: ClockPhase) -> i64 {
    match phase {
        ClockPhase::Active => EVENT_END - 1_000, // now <= event_end
        ClockPhase::RefundWindow => EVENT_END + 1, // event_end < now < refund_deadline
        ClockPhase::PostDeadline => REFUND_DEADLINE + 1, // now >= refund_deadline
    }
}

/// Build accounts reflecting an oracle `EscrowState`, run `action` against the real
/// program, and return whether it was ACCEPTED (`is_ok`). Covers the state/phase-gated
/// lifecycle actions only.
fn run_action(action: u32, state: &EscrowState, phase: ClockPhase) -> bool {
    let mut svm = setup();
    svm.warp_to_timestamp(warp_for(phase));

    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    // EventEscrow reflecting the state: totals track deposit/settled so the accounting
    // invariants line up with the oracle's structural model.
    let total_deposited = if state.deposit_exists { DEPOSIT_AMOUNT } else { 0 };
    let total_refunded = if state.settled { DEPOSIT_AMOUNT } else { 0 };
    let escrow_acct = event_escrow_account(
        escrow,
        ORGANIZER,
        EVENT_ID,
        USDC_MINT,
        VAULT,
        DEPOSIT_AMOUNT,
        EVENT_END,
        REFUND_DEADLINE,
        total_deposited,
        total_refunded,
        0,
        state.is_active,
        escrow_bump,
    );

    match action {
        ACTION_MARK_CHECKED_IN => {
            let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);
            // If no deposit exists in this state, pass an uninitialized (system-owned)
            // account so the program's Account<AttendeeDeposit> load fails → reject.
            let deposit_acct = if state.deposit_exists {
                attendee_deposit_account(
                    deposit,
                    ATTENDEE,
                    escrow,
                    DEPOSIT_AMOUNT,
                    1_699_000_000,
                    state.checked_in,
                    state.settled,
                    deposit_bump,
                )
            } else {
                empty(deposit)
            };
            let ix = with_signers(
                MarkCheckedInInstruction {
                    organizer: ORGANIZER,
                    event_escrow: escrow,
                    attendee_deposit: deposit,
                    _event_id: EVENT_ID,
                }
                .into(),
                &[0],
            );
            svm.process_instruction(&ix, &[signer(ORGANIZER), escrow_acct, deposit_acct])
                .is_ok()
        }
        ACTION_DEACTIVATE_EVENT => {
            let ix = with_signers(
                DeactivateEventInstruction {
                    organizer: ORGANIZER,
                    event_escrow: escrow,
                    _event_id: EVENT_ID,
                }
                .into(),
                &[0],
            );
            svm.process_instruction(&ix, &[signer(ORGANIZER), escrow_acct])
                .is_ok()
        }
        other => panic!("differential harness does not cover action {other}"),
    }
}

#[test]
fn differential_oracle_matches_program() {
    let cfg = EscrowConfig::default();

    // (action, history, phase, note). Each is a transition where the oracle and the real
    // program are EXPECTED to agree (verified against the handler logic). derive(history)
    // replays the structural effects to build the starting state.
    let cases: &[(u32, &[u32], ClockPhase, &str)] = &[
        // deactivate_event: gated on is_active (EscrowError::EventNotActive)
        (
            ACTION_DEACTIVATE_EVENT,
            &[ACTION_CREATE_EVENT],
            ClockPhase::Active,
            "active event deactivates",
        ),
        (
            ACTION_DEACTIVATE_EVENT,
            &[ACTION_CREATE_EVENT, ACTION_DEACTIVATE_EVENT],
            ClockPhase::Active,
            "already-inactive event rejects",
        ),
        // mark_checked_in: gated on !checked_in + clock <= event_end + deposit exists
        (
            ACTION_MARK_CHECKED_IN,
            &[ACTION_CREATE_EVENT, ACTION_DEPOSIT],
            ClockPhase::Active,
            "checkin in the active window",
        ),
        (
            ACTION_MARK_CHECKED_IN,
            &[ACTION_CREATE_EVENT, ACTION_DEPOSIT],
            ClockPhase::RefundWindow,
            "checkin after event_end rejects",
        ),
        (
            ACTION_MARK_CHECKED_IN,
            &[ACTION_CREATE_EVENT, ACTION_DEPOSIT],
            ClockPhase::PostDeadline,
            "checkin post-deadline rejects",
        ),
        (
            ACTION_MARK_CHECKED_IN,
            &[ACTION_CREATE_EVENT, ACTION_DEPOSIT, ACTION_MARK_CHECKED_IN],
            ClockPhase::Active,
            "double check-in rejects",
        ),
        (
            ACTION_MARK_CHECKED_IN,
            &[ACTION_CREATE_EVENT],
            ClockPhase::Active,
            "checkin without a deposit rejects",
        ),
    ];

    for (action, history, phase, note) in cases {
        let state = EscrowState::derive(history);
        let oracle = is_valid_transition(*action, &state, *phase, &cfg);
        let program = run_action(*action, &state, *phase);
        assert_eq!(
            program,
            oracle,
            "DIFFERENTIAL MISMATCH [{note}] action={action} phase={phase:?} state=({}): \
             program accepted={program} but oracle valid={oracle}",
            state.describe(),
        );
    }
}

#[test]
fn documented_divergence_checkin_ignores_settled() {
    // A real oracle↔program divergence surfaced by building this harness, pinned here so it
    // stays visible and any future change (either side) is caught:
    //
    // The oracle's `valid_checkin` requires `!settled`, but the real `mark_checked_in`
    // handler (instructions/mark_checked_in.rs) gates ONLY on `!checked_in` + `clock <=
    // event_end`. So the PROGRAM accepts checking-in a refunded-but-not-closed deposit
    // while the ORACLE rejects it. This is BENIGN: the refund already moved the money and
    // the `settled` bit blocks a second refund, so setting `checked_in` on a settled
    // deposit changes nothing about funds. It is an oracle over-specification, not a
    // program gap.
    let cfg = EscrowConfig::default();
    let state = EscrowState::derive(&[ACTION_CREATE_EVENT, ACTION_DEPOSIT, ACTION_REFUND]);
    assert!(state.deposit_exists && state.settled && !state.checked_in);

    assert!(
        !is_valid_transition(ACTION_MARK_CHECKED_IN, &state, ClockPhase::Active, &cfg),
        "oracle rejects checking-in a settled deposit",
    );
    assert!(
        run_action(ACTION_MARK_CHECKED_IN, &state, ClockPhase::Active),
        "program ALLOWS checking-in a settled (refunded) deposit — no !settled gate in the handler",
    );
}
