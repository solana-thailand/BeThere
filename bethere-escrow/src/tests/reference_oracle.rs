//! Reference oracle for the `bethere-escrow` state machine.
//!
//! This is a SELF-CONTAINED copy of the `is_valid` transition truth table
//! from the `ns-engine` crate's `BethereEscrowPruner`
//! (`mac helper/ns-engine/src/pruners/escrow.rs`). It is intentionally
//! copied here as a reference, NOT linked as a path dependency, to keep the
//! `bethere-escrow` and `ns-engine` workspaces decoupled (SOLID: the program
//! must not depend on the inference engine).
//!
//! # Purpose
//!
//! This module pins the INTENDED state-machine contract as executable,
//! auditable code that lives next to the program. The existing SVM tests
//! (checkin.rs, refund.rs, close.rs, ...) exercise the real on-chain guards
//! via `quasar_svm`; this oracle encodes the same contract as a pure
//! function so that:
//!
//! 1. The contract is documented in code, not only in prose.
//! 2. If the program's guards and this reference ever diverge, that is a bug
//!    to investigate — they must agree on every transition.
//! 3. New tests can consult `is_valid_transition` to predict the program's
//!    accept/reject decision before running the instruction.
//!
//! # Version sync
//!
//! Kept in sync manually with `ns-engine` `BethereEscrowPruner`. The
//! `REFERENCE_SOURCE_REV` constant below tracks the originating commit. When
//! the pruner's truth table changes, update this file and bump the rev.
//!
//! # Scope (same as the originating pruner)
//!
//! - Single (event, attendee) deposit lifecycle.
//! - Coarse clock phases (Active / RefundWindow / PostDeadline); exact
//!   boundary values (`now == event_end`) are not distinguished.
//! - Signer roles (organizer vs attendee) are abstracted away; the oracle
//!   models only the structural transition, not WHO signs.

extern crate alloc;

use alloc::string::String;
use alloc::format;

/// Originating commit of `ns-engine`'s `BethereEscrowPruner` this reference
/// mirrors. Bump when the truth table changes.
pub const REFERENCE_SOURCE_REV: &str = "052b3d3";

// ── Action IDs (oracle-local 1-indexed; NOT on-chain discriminators) ──────────

pub const ACTION_CREATE_EVENT: u32 = 1;
pub const ACTION_DEPOSIT: u32 = 2;
pub const ACTION_MARK_CHECKED_IN: u32 = 3;
pub const ACTION_REFUND: u32 = 4;
pub const ACTION_CLAIM_FORFEITED: u32 = 5;
pub const ACTION_DEACTIVATE_EVENT: u32 = 6;
pub const ACTION_CLOSE_EVENT: u32 = 7;
pub const ACTION_CLOSE_DEPOSIT: u32 = 8;
pub const ACTION_ROLLOVER_DEPOSIT: u32 = 9;
pub const ACTION_INTROSPECTION: u32 = 10;

// ── Clock phase ────────────────────────────────────────────────

/// Coarse wall-clock phase relative to event time boundaries.
///
/// - [`ClockPhase::Active`]: `now <= event_end` — create, deposit, checkin.
/// - [`ClockPhase::RefundWindow`]: `event_end < now < refund_deadline`.
/// - [`ClockPhase::PostDeadline`]: `now >= refund_deadline`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClockPhase {
    Active,
    RefundWindow,
    PostDeadline,
}

impl ClockPhase {
    /// Advance to the next phase. [`ClockPhase::PostDeadline`] is terminal.
    pub fn next(self) -> Self {
        match self {
            Self::Active => Self::RefundWindow,
            Self::RefundWindow => Self::PostDeadline,
            Self::PostDeadline => Self::PostDeadline,
        }
    }
}

// ── Structural state (derived by replaying instruction history) ───────────────

/// Structural state of a single (event, attendee) deposit lifecycle.
///
/// Every field is a pure function of the replayed instruction history. The
/// clock phase is held separately and gates only candidate transitions, not
/// the replay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EscrowState {
    /// `EventEscrow` account initialized.
    pub event_created: bool,
    /// `EventEscrow.is_active` — true at create, false after deactivate.
    pub is_active: bool,
    /// `AttendeeDeposit` account initialized for this (event, attendee).
    pub deposit_exists: bool,
    /// `AttendeeDeposit.checked_in` — set by `mark_checked_in`.
    pub checked_in: bool,
    /// Unified "settled" bit — set by `refund`, `claim_forfeited`, `rollover`.
    /// Mirrors the program's `refunded` field, which all three settling
    /// instructions set true to prevent double-spend.
    pub settled: bool,
    /// `EventEscrow` closed (data zeroed) via `close_event`.
    pub event_closed: bool,
    /// `AttendeeDeposit` closed via `close_deposit`.
    pub deposit_closed: bool,
}

impl EscrowState {
    /// Reconstruct state by replaying `history` structurally.
    ///
    /// Each token's effect is applied only if its structural precondition
    /// holds at that point in the replay (clock gates are intentionally
    /// ignored during replay — the history is assumed to be a valid trace).
    pub fn derive(history: &[u32]) -> Self {
        let mut state = Self::default();
        for &tok in history {
            state.apply(tok);
        }
        state
    }

    /// Apply a single token's structural effect, guarded by preconditions.
    fn apply(&mut self, tok: u32) {
        match tok {
            ACTION_CREATE_EVENT => {
                if !self.event_created {
                    self.event_created = true;
                    self.is_active = true;
                }
            }
            ACTION_DEPOSIT => {
                if self.event_created && self.is_active && !self.deposit_exists {
                    self.deposit_exists = true;
                }
            }
            ACTION_MARK_CHECKED_IN => {
                if self.deposit_exists && !self.checked_in && !self.settled {
                    self.checked_in = true;
                }
            }
            ACTION_REFUND | ACTION_CLAIM_FORFEITED | ACTION_ROLLOVER_DEPOSIT => {
                if self.deposit_exists && !self.settled {
                    self.settled = true;
                }
            }
            ACTION_DEACTIVATE_EVENT => {
                if self.event_created && self.is_active {
                    self.is_active = false;
                }
            }
            ACTION_CLOSE_EVENT => {
                if self.event_created
                    && !self.is_active
                    && !self.event_closed
                    && (!self.deposit_exists || self.settled)
                {
                    self.event_closed = true;
                }
            }
            ACTION_CLOSE_DEPOSIT => {
                if self.deposit_exists
                    && !self.deposit_closed
                    && (self.settled || self.event_closed)
                {
                    self.deposit_closed = true;
                }
            }
            ACTION_INTROSPECTION => {}
            _ => {}
        }
    }

    /// Human-readable one-line description for test diagnostics.
    pub fn describe(&self) -> String {
        format!(
            "created={created} active={active} deposit={deposit} checked_in={checked_in} \
             settled={settled} event_closed={event_closed} deposit_closed={deposit_closed}",
            created = self.event_created,
            active = self.is_active,
            deposit = self.deposit_exists,
            checked_in = self.checked_in,
            settled = self.settled,
            event_closed = self.event_closed,
            deposit_closed = self.deposit_closed,
        )
    }
}

// ── Non-derivable config ───────────────────────────────────────

/// Invariants that cannot be reconstructed from instruction history alone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EscrowConfig {
    /// Whether a valid target event exists for `rollover_deposit`.
    ///
    /// The real guard requires the target event to be active, same organizer,
    /// same mint, same deposit amount. The oracle abstracts these to one bool
    /// the test sets to model the target event's availability.
    pub target_event_active: bool,
}

impl EscrowConfig {
    /// Config with `target_event_active = true` (rollover available).
    pub fn with_rollover_target() -> Self {
        Self { target_event_active: true }
    }
}

// ── The reference truth table ──────────────────────────────────

/// Pure transition legality check — the reference `is_valid`.
///
/// Returns `true` iff `action` is a legal next instruction given the
/// structural `state`, the clock `phase`, and the `config`. Mirrors
/// `ns-engine`'s `BethereEscrowPruner::is_valid` exactly.
pub fn is_valid_transition(action: u32, state: &EscrowState, phase: ClockPhase, config: &EscrowConfig) -> bool {
    match action {
        ACTION_CREATE_EVENT => valid_create(state, phase),
        ACTION_DEPOSIT => valid_deposit(state),
        ACTION_MARK_CHECKED_IN => valid_checkin(state, phase),
        ACTION_REFUND => valid_refund(state, phase),
        ACTION_CLAIM_FORFEITED => valid_claim(state, phase),
        ACTION_DEACTIVATE_EVENT => valid_deactivate(state),
        ACTION_CLOSE_EVENT => valid_close_event(state),
        ACTION_CLOSE_DEPOSIT => valid_close_deposit(state),
        ACTION_ROLLOVER_DEPOSIT => valid_rollover(state, config),
        ACTION_INTROSPECTION => true,
        _ => false,
    }
}

fn valid_create(s: &EscrowState, phase: ClockPhase) -> bool {
    !s.event_closed && !s.event_created && phase == ClockPhase::Active
}

fn valid_deposit(s: &EscrowState) -> bool {
    s.event_created && s.is_active && !s.event_closed && !s.deposit_exists
}

fn valid_checkin(s: &EscrowState, phase: ClockPhase) -> bool {
    s.deposit_exists
        && !s.checked_in
        && !s.settled
        && !s.deposit_closed
        && phase == ClockPhase::Active
}

fn valid_refund(s: &EscrowState, phase: ClockPhase) -> bool {
    if !s.deposit_exists || s.settled || s.deposit_closed {
        return false;
    }
    match s.checked_in {
        // Checked-in attendees refund anytime after event_end.
        true => matches!(phase, ClockPhase::RefundWindow | ClockPhase::PostDeadline),
        // No-shows must refund before the deadline.
        false => phase == ClockPhase::RefundWindow,
    }
}

fn valid_claim(s: &EscrowState, phase: ClockPhase) -> bool {
    s.deposit_exists
        && !s.checked_in
        && !s.settled
        && !s.deposit_closed
        && phase == ClockPhase::PostDeadline
}

fn valid_deactivate(s: &EscrowState) -> bool {
    s.event_created && s.is_active && !s.event_closed
}

fn valid_close_event(s: &EscrowState) -> bool {
    s.event_created && !s.is_active && !s.event_closed && (!s.deposit_exists || s.settled)
}

fn valid_close_deposit(s: &EscrowState) -> bool {
    s.deposit_exists && !s.deposit_closed && (s.settled || s.event_closed)
}

fn valid_rollover(s: &EscrowState, config: &EscrowConfig) -> bool {
    s.deposit_exists && s.checked_in && !s.settled && !s.deposit_closed && config.target_event_active
}

// ── Exhaustive truth-table tests ───────────────────────────────
//
// These tests pin the reference truth table by asserting the oracle's verdict
// for every (state, action) across the lifecycle. If the program's on-chain
// guards and this reference ever disagree, the SVM tests in the sibling
// modules should catch it — this module only guards the reference itself
// against drift from the documented contract.

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;

    fn default_config() -> EscrowConfig {
        EscrowConfig::default() // target_event_active = false
    }

    fn rollover_config() -> EscrowConfig {
        EscrowConfig::with_rollover_target()
    }

    /// Assert `action` validity and return it, with a descriptive panic.
    fn expect(action: u32, state: &EscrowState, phase: ClockPhase, config: &EscrowConfig, want: bool) {
        let got = is_valid_transition(action, state, phase, config);
        assert_eq!(
            got, want,
            "action {action} on [{}] @ {phase:?} (target_active={}): expected {want}, got {got}",
            state.describe(),
            config.target_event_active,
        );
    }

    // ── Initial state: nothing created ──

    #[test]
    fn test_initial_state() {
        let s = EscrowState::default();
        // Only create_event (during Active) and introspection are valid.
        expect(ACTION_CREATE_EVENT, &s, ClockPhase::Active, &default_config(), true);
        expect(ACTION_INTROSPECTION, &s, ClockPhase::Active, &default_config(), true);
        // Everything else needs prior state.
        for &a in &[
            ACTION_DEPOSIT,
            ACTION_MARK_CHECKED_IN,
            ACTION_REFUND,
            ACTION_CLAIM_FORFEITED,
            ACTION_DEACTIVATE_EVENT,
            ACTION_CLOSE_EVENT,
            ACTION_CLOSE_DEPOSIT,
            ACTION_ROLLOVER_DEPOSIT,
        ] {
            expect(a, &s, ClockPhase::Active, &default_config(), false);
        }
        // create_event outside Active is blocked.
        expect(ACTION_CREATE_EVENT, &s, ClockPhase::RefundWindow, &default_config(), false);
        expect(ACTION_CREATE_EVENT, &s, ClockPhase::PostDeadline, &default_config(), false);
    }

    // ── After create_event: active, no deposit yet ──

    #[test]
    fn test_after_create() {
        let s = EscrowState::derive(&[ACTION_CREATE_EVENT]);
        assert!(s.event_created && s.is_active && !s.deposit_exists);
        expect(ACTION_DEPOSIT, &s, ClockPhase::Active, &default_config(), true);
        expect(ACTION_DEACTIVATE_EVENT, &s, ClockPhase::Active, &default_config(), true);
        // Double-create blocked.
        expect(ACTION_CREATE_EVENT, &s, ClockPhase::Active, &default_config(), false);
        // Refund/claim/rollover/close_deposit need a deposit.
        expect(ACTION_REFUND, &s, ClockPhase::RefundWindow, &default_config(), false);
        expect(ACTION_CLAIM_FORFEITED, &s, ClockPhase::PostDeadline, &default_config(), false);
        expect(ACTION_CLOSE_DEPOSIT, &s, ClockPhase::Active, &default_config(), false);
        expect(ACTION_ROLLOVER_DEPOSIT, &s, ClockPhase::Active, &rollover_config(), false);
        // close_event blocked while active (unless no deposit — but here active).
        expect(ACTION_CLOSE_EVENT, &s, ClockPhase::Active, &default_config(), false);
    }

    // ── After deposit: not checked in, Active ──

    #[test]
    fn test_after_deposit() {
        let s = EscrowState::derive(&[ACTION_CREATE_EVENT, ACTION_DEPOSIT]);
        assert!(s.deposit_exists && !s.checked_in && !s.settled);
        expect(ACTION_MARK_CHECKED_IN, &s, ClockPhase::Active, &default_config(), true);
        // Checkin blocked outside Active.
        expect(ACTION_MARK_CHECKED_IN, &s, ClockPhase::RefundWindow, &default_config(), false);
        expect(ACTION_MARK_CHECKED_IN, &s, ClockPhase::PostDeadline, &default_config(), false);
        // No-show refund only during RefundWindow (not PostDeadline).
        expect(ACTION_REFUND, &s, ClockPhase::RefundWindow, &default_config(), true);
        expect(ACTION_REFUND, &s, ClockPhase::PostDeadline, &default_config(), false);
        // Claim needs PostDeadline + no-show.
        expect(ACTION_CLAIM_FORFEITED, &s, ClockPhase::PostDeadline, &default_config(), true);
        expect(ACTION_CLAIM_FORFEITED, &s, ClockPhase::RefundWindow, &default_config(), false);
        // Rollover needs checked_in first.
        expect(ACTION_ROLLOVER_DEPOSIT, &s, ClockPhase::Active, &rollover_config(), false);
    }

    // ── Checked in (no-show path avoided): refund anytime after event_end ──

    #[test]
    fn test_checked_in_refund_window() {
        let s = EscrowState::derive(&[ACTION_CREATE_EVENT, ACTION_DEPOSIT, ACTION_MARK_CHECKED_IN]);
        assert!(s.checked_in && !s.settled);
        // Checked-in refunds allowed in RefundWindow AND PostDeadline.
        expect(ACTION_REFUND, &s, ClockPhase::RefundWindow, &default_config(), true);
        expect(ACTION_REFUND, &s, ClockPhase::PostDeadline, &default_config(), true);
        // Not during Active (before event_end).
        expect(ACTION_REFUND, &s, ClockPhase::Active, &default_config(), false);
        // Rollover available (checked_in, not settled, target active).
        expect(ACTION_ROLLOVER_DEPOSIT, &s, ClockPhase::Active, &rollover_config(), true);
        expect(ACTION_ROLLOVER_DEPOSIT, &s, ClockPhase::Active, &default_config(), false);
    }

    // ── Settled (after refund/claim/rollover): no double-settle ──

    #[test]
    fn test_settled_blocks_double_spend() {
        // refund, claim, AND rollover all set the unified settled bit.
        for &settle in &[ACTION_REFUND, ACTION_CLAIM_FORFEITED, ACTION_ROLLOVER_DEPOSIT] {
            let mut hist = alloc::vec![ACTION_CREATE_EVENT, ACTION_DEPOSIT];
            if settle == ACTION_ROLLOVER_DEPOSIT {
                hist.push(ACTION_MARK_CHECKED_IN);
            }
            hist.push(settle);
            let s = EscrowState::derive(&hist);
            assert!(s.settled, "settle action {settle} must set settled");
            // No second settling action valid, regardless of phase.
            for &a in &[ACTION_REFUND, ACTION_CLAIM_FORFEITED, ACTION_ROLLOVER_DEPOSIT] {
                expect(a, &s, ClockPhase::RefundWindow, &rollover_config(), false);
                expect(a, &s, ClockPhase::PostDeadline, &rollover_config(), false);
            }
            // close_deposit now valid (settled).
            expect(ACTION_CLOSE_DEPOSIT, &s, ClockPhase::Active, &default_config(), true);
            // mark_checked_in blocked after settle.
            expect(ACTION_MARK_CHECKED_IN, &s, ClockPhase::Active, &default_config(), false);
        }
    }

    // ── Deactivate → close_event path ──

    #[test]
    fn test_close_event_requires_inactive_and_settled() {
        // Without a deposit: deactivate then close_event works.
        let s = EscrowState::derive(&[ACTION_CREATE_EVENT, ACTION_DEACTIVATE_EVENT]);
        assert!(!s.is_active);
        expect(ACTION_CLOSE_EVENT, &s, ClockPhase::Active, &default_config(), true);
        // With an unsettled deposit: close_event blocked (vault not empty).
        let s2 = EscrowState::derive(&[
            ACTION_CREATE_EVENT,
            ACTION_DEPOSIT,
            ACTION_DEACTIVATE_EVENT,
        ]);
        expect(ACTION_CLOSE_EVENT, &s2, ClockPhase::Active, &default_config(), false);
        // After settle + deactivate: close_event allowed.
        let s3 = EscrowState::derive(&[
            ACTION_CREATE_EVENT,
            ACTION_DEPOSIT,
            ACTION_REFUND,
            ACTION_DEACTIVATE_EVENT,
        ]);
        assert!(s3.settled && !s3.is_active);
        expect(ACTION_CLOSE_EVENT, &s3, ClockPhase::RefundWindow, &default_config(), true);
        // Double close_event blocked.
        let s4 = EscrowState::derive(&[
            ACTION_CREATE_EVENT,
            ACTION_DEACTIVATE_EVENT,
            ACTION_CLOSE_EVENT,
        ]);
        assert!(s4.event_closed);
        expect(ACTION_CLOSE_EVENT, &s4, ClockPhase::Active, &default_config(), false);
    }

    // ── close_deposit: attendee path (settled) + GC path (event_closed) ──

    #[test]
    fn test_close_deposit_paths() {
        // Path 1: settled deposit → close_deposit valid (attendee path).
        let s1 = EscrowState::derive(&[ACTION_CREATE_EVENT, ACTION_DEPOSIT, ACTION_REFUND]);
        assert!(s1.settled);
        expect(ACTION_CLOSE_DEPOSIT, &s1, ClockPhase::RefundWindow, &default_config(), true);
        // After close_deposit: double-close blocked.
        let s1b = EscrowState::derive(&[
            ACTION_CREATE_EVENT,
            ACTION_DEPOSIT,
            ACTION_REFUND,
            ACTION_CLOSE_DEPOSIT,
        ]);
        assert!(s1b.deposit_closed);
        expect(ACTION_CLOSE_DEPOSIT, &s1b, ClockPhase::Active, &default_config(), false);

        // Path 2: event_closed (GC path) → close_deposit valid even if not settled.
        let s2 = EscrowState::derive(&[
            ACTION_CREATE_EVENT,
            ACTION_DEPOSIT,
            ACTION_DEACTIVATE_EVENT,
            ACTION_CLOSE_EVENT, // needs settled deposit? No — deposit unsettled blocks it.
        ]);
        // Actually close_event with an unsettled deposit is blocked in apply(),
        // so s2.event_closed stays false. Verify that GC path requires the
        // event to actually be closed, which needs the deposit settled first.
        assert!(!s2.event_closed, "unsettled deposit blocks close_event");
        expect(ACTION_CLOSE_DEPOSIT, &s2, ClockPhase::Active, &default_config(), false);
    }

    // ── Clock phase enum ──

    #[test]
    fn test_clock_phase_progression() {
        assert_eq!(ClockPhase::Active.next(), ClockPhase::RefundWindow);
        assert_eq!(ClockPhase::RefundWindow.next(), ClockPhase::PostDeadline);
        // PostDeadline is terminal.
        assert_eq!(ClockPhase::PostDeadline.next(), ClockPhase::PostDeadline);
    }

    // ── Unknown action ──

    #[test]
    fn test_unknown_action_rejected() {
        let s = EscrowState::default();
        expect(0, &s, ClockPhase::Active, &default_config(), false);
        expect(99, &s, ClockPhase::Active, &default_config(), false);
        expect(u32::MAX, &s, ClockPhase::Active, &default_config(), false);
    }

    // ── Introspection always valid (read-only) ──

    #[test]
    fn test_introspection_always_valid() {
        let s = EscrowState::default();
        for &phase in &[ClockPhase::Active, ClockPhase::RefundWindow, ClockPhase::PostDeadline] {
            expect(ACTION_INTROSPECTION, &s, phase, &default_config(), true);
        }
        // Even after the whole lifecycle.
        let s2 = EscrowState::derive(&[
            ACTION_CREATE_EVENT,
            ACTION_DEPOSIT,
            ACTION_REFUND,
            ACTION_CLOSE_DEPOSIT,
            ACTION_DEACTIVATE_EVENT,
            ACTION_CLOSE_EVENT,
        ]);
        expect(ACTION_INTROSPECTION, &s2, ClockPhase::PostDeadline, &default_config(), true);
    }

    // ── End-to-end lifecycle consistency: replay validity ──
    //
    // A full valid lifecycle replayed through derive() must reach a terminal
    // state where the only remaining actions are introspection / close_deposit.

    #[test]
    fn test_full_refund_lifecycle_reaches_terminal() {
        let history = alloc::vec![
            ACTION_CREATE_EVENT,
            ACTION_DEPOSIT,
            ACTION_MARK_CHECKED_IN,
            ACTION_REFUND,
            ACTION_CLOSE_DEPOSIT,
            ACTION_DEACTIVATE_EVENT,
            ACTION_CLOSE_EVENT,
        ];
        let s = EscrowState::derive(&history);
        assert!(s.settled, "deposit settled by refund");
        assert!(s.deposit_closed, "deposit closed");
        assert!(s.event_closed, "event closed");
        // Nothing left to do except introspection.
        for &a in &[
            ACTION_CREATE_EVENT,
            ACTION_DEPOSIT,
            ACTION_MARK_CHECKED_IN,
            ACTION_REFUND,
            ACTION_CLAIM_FORFEITED,
            ACTION_DEACTIVATE_EVENT,
            ACTION_CLOSE_EVENT,
            ACTION_CLOSE_DEPOSIT,
            ACTION_ROLLOVER_DEPOSIT,
        ] {
            expect(a, &s, ClockPhase::PostDeadline, &rollover_config(), false);
        }
        expect(ACTION_INTROSPECTION, &s, ClockPhase::PostDeadline, &default_config(), true);
    }
}
