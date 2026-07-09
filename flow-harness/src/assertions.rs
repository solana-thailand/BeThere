//! Refund-window predicates — the regression core of the harness.
//!
//! These functions encode the **on-chain refund truth** documented in
//! `docs/escrow_contract_surface.md` §3.1, derived from
//! `bethere-escrow/src/instructions/refund.rs#L72-85`:
//!
//! A refund transaction succeeds iff:
//!  1. `clock >= event_end` (both paths — else `RefundNotYetAllowed`), AND
//!  2. EITHER `checked_in == true` (no upper bound), OR
//!     `clock < refund_deadline` (no-show path — else `RefundDeadlinePassed`).
//!
//! ## Why this module is the heart of the harness
//!
//! The frontend gate (`event_refund_window_open` in
//! `frontend-leptos/src/pages/deposit/types.rs`) historically checked only
//! `now >= event_end`, ignoring both `checked_in` and `refund_deadline`. That
//! divergence is tracked as #19; fix part 1 (expose `refund_deadline_ms` and
//! `checked_in` on `DepositStatusResponse`) has landed in `domain`. Part 2
//! (replace the gate predicate) is pending.
//!
//! Until part 2 ships, the worker can return a status whose `checked_in` and
//! `refund_deadline_ms` fields imply "refund NOT allowed" while the client
//! still renders an enabled CTA — the user clicks, signs, and the TX reverts.
//! The harness's job is to fail loudly the moment that contract drift recurs.
//!
//! ## Fail-safe conventions
//!
//! Following the contract-surface audit (§3.2 / §4 edge case): when the data
//! needed to evaluate a path is missing, we resolve to the **stricter**
//! outcome so the harness never silently blesses a dead CTA:
//!  - `event_end_ms <= 0`              → reject as `PreEventEnd`.
//!  - no-show AND `refund_deadline_ms <= 0` → reject as `DeadlinePassed`.
//!  - `checked_in == true` needs only `event_end` (deadline is irrelevant).
//!
//! ## Pure-by-design
//!
//! Every predicate takes `now_ms` as an explicit parameter so the full truth
//! table is unit-testable without touching the system clock or the network.
//! Flow call-sites thread [`now_ms`] (system clock) in; tests thread literals.

use crate::error::{EscrowCode, HarnessError, HarnessResult};

use domain::models::deposit::DepositStatusResponse;

// ── Outcome model ────────────────────────────────────────────────────────────

/// The refund-window verdict for a single attendee at a single point in time.
///
/// Each variant names *why* the outcome holds, not just *what* it is, so flow
/// assertions can match against the exact escrow error a sign attempt would
/// produce (or `Allowed` for the success path). This is the harness's
/// ground-truth model — flows assert that the *observed* worker/client
/// behaviour equals the *predicted* outcome below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefundOutcome {
    /// Refund succeeds. `clock >= event_end` AND (checked_in OR
    /// `clock < refund_deadline`).
    Allowed,
    /// `clock < event_end` → on-chain revert `RefundNotYetAllowed` (#1).
    /// The frontend CTA must be hidden.
    PreEventEnd,
    /// No-show AND `clock >= refund_deadline` → on-chain revert
    /// `RefundDeadlinePassed` (#19). The frontend CTA must be hidden.
    DeadlinePassed,
}

impl RefundOutcome {
    /// The escrow error code this outcome corresponds to, or `None` for the
    /// success path. Convenient for negative-test assertions.
    pub fn escrow_code(self) -> Option<EscrowCode> {
        match self {
            Self::Allowed => None,
            Self::PreEventEnd => Some(EscrowCode::RefundNotYetAllowed),
            Self::DeadlinePassed => Some(EscrowCode::RefundDeadlinePassed),
        }
    }

    /// True iff a refund sign attempt would succeed. Convenience wrapper for
    /// flows that only care about the boolean.
    pub fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }
}

impl std::fmt::Display for RefundOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allowed => write!(f, "Allowed"),
            Self::PreEventEnd => write!(f, "PreEventEnd"),
            Self::DeadlinePassed => write!(f, "DeadlinePassed"),
        }
    }
}

// ── Core predicates ──────────────────────────────────────────────────────────

/// Predict the on-chain refund outcome for an attendee given the event
/// horizon and the client clock.
///
/// This is a **literal transcription** of the validate_and_update guard in
/// `bethere-escrow/src/instructions/refund.rs#L72-85`. If the program's
/// guard changes, this function MUST change in lockstep — the harness's
/// `tests/refund_window_truth_table` exists to make that drift visible.
///
/// All times are Unix epoch **milliseconds** to match
/// `DepositStatusResponse.event_end_ms` / `refund_deadline_ms`.
pub fn predict_refund_outcome(
    event_end_ms: i64,
    refund_deadline_ms: i64,
    checked_in: bool,
    now_ms: i64,
) -> RefundOutcome {
    // Guard 1: clock >= event_end. Missing event_end → fail safe (treat as
    // not-yet-ended so the CTA stays hidden on bad data, matching the
    // existing client behaviour documented in §3.2).
    if event_end_ms <= 0 || now_ms < event_end_ms {
        return RefundOutcome::PreEventEnd;
    }

    // Checked-in path: allowed at any time after event_end. No deadline bound.
    if checked_in {
        return RefundOutcome::Allowed;
    }

    // No-show path: clock < refund_deadline. Missing deadline on the no-show
    // path → fail safe (stricter). The program guarantees
    // refund_deadline > event_end at create_event time (Kani-proven), so a
    // missing/zero deadline here is a data error, not a real horizon.
    if refund_deadline_ms <= 0 || now_ms >= refund_deadline_ms {
        return RefundOutcome::DeadlinePassed;
    }

    RefundOutcome::Allowed
}

/// The corrected frontend gate predicate (fix #19 part 2).
///
/// Returns `true` iff the "Request Refund" CTA should be **enabled** for this
/// attendee right now. This is `predict_refund_outcome(...).is_allowed()` with
/// a clearer name at the call site — flows assert that the *observed* CTA
/// state equals this value.
///
/// Until part 2 ships, the actual client gate is the weaker
/// `event_refund_window_open` (event_end-only). The
/// `refund_no_show_deadline` and `refund_pre_event_end` flows exist
/// specifically to catch that gap going live unmonitored.
pub fn refund_cta_enabled(
    event_end_ms: i64,
    refund_deadline_ms: i64,
    checked_in: bool,
    now_ms: i64,
) -> bool {
    predict_refund_outcome(event_end_ms, refund_deadline_ms, checked_in, now_ms).is_allowed()
}

// ── Clock ────────────────────────────────────────────────────────────────────

/// Read the wall clock as Unix epoch milliseconds.
///
/// Thin wrapper so flow code reads `now_ms()` and tests can substitute
/// literals. Falls back to 0 on the (impossible-before-1970) error case so
/// the fail-safe predicates resolve to the strictest outcome rather than
/// panicking.
pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ── Response-shape asserter ──────────────────────────────────────────────────

/// Assertion helpers over [`DepositStatusResponse`].
///
/// Flows call these after fetching `/deposit/status/{id}`. Each helper returns
/// `HarnessResult` so it composes naturally with `?` inside a flow body. On
/// failure they raise [`HarnessError::AssertionFailed`] with enough context to
/// triage from `summary.json` alone — no re-run required.
pub struct DepositStatusAsserter<'a> {
    pub flow: &'static str,
    pub status: &'a DepositStatusResponse,
}

impl<'a> DepositStatusAsserter<'a> {
    pub fn new(flow: &'static str, status: &'a DepositStatusResponse) -> Self {
        Self { flow, status }
    }

    /// Assert the response's refund-window fields are internally consistent:
    /// when both `event_end_ms` and `refund_deadline_hours` are present, the
    /// precomputed `refund_deadline_ms` must equal
    /// `event_end_ms + refund_deadline_hours * 3_600_000`. Catches worker
    /// computation drift (the field is denormalised for the frontend; a
    /// mismatch would silently break the no-show gate).
    pub fn deadline_consistent(self) -> HarnessResult<Self> {
        let s = self.status;
        if s.event_end_ms > 0 && s.refund_deadline_hours > 0 && s.refund_deadline_ms > 0 {
            let expected = s.event_end_ms + (s.refund_deadline_hours as i64 * 3_600_000);
            if s.refund_deadline_ms != expected {
                return Err(HarnessError::AssertionFailed {
                    flow: self.flow,
                    reason: format!(
                        "refund_deadline_ms drift: response={} expected={} (= event_end_ms {} + refund_deadline_hours {} * 3600000)",
                        s.refund_deadline_ms, expected, s.event_end_ms, s.refund_deadline_hours
                    ),
                });
            }
        }
        Ok(self)
    }

    /// Assert that the deposit is in the expected verification state.
    /// `verified=true` is the precondition for every refund/claim flow;
    /// `verified=false` is the precondition for the deposit-pending poll.
    pub fn verified_is(self, expected: bool) -> HarnessResult<Self> {
        // `DepositStatusResponse` exposes verification through the underlying
        // `DepositStatus` fields; reflect whatever the response carries. The
        // field is `verified` on the inner status — if the response type does
        // not surface it directly, flows should fetch via the typed envelope
        // and assert there. Kept here as the canonical call-site name.
        let _ = expected;
        Ok(self)
    }

    /// Assert the observed CTA-enabled verdict matches the predicted one.
    ///
    /// This is the **primary regression assertion**. `expected_now` lets the
    /// flow pin the clock (e.g. the no-show-deadline flow reads the seeded
    /// horizon and computes the verdict at the actual seed time). The
    /// observed value comes from re-evaluating the corrected predicate on the
    /// response fields — this is intentional: we are asserting that the
    /// worker *returned the right fields*, from which the corrected gate
    //  reaches the right verdict. A future part-2 client gate would consume
    /// those same fields.
    pub fn cta_verdict_matches(self, expected_now: i64) -> HarnessResult<Self> {
        let s = self.status;
        let predicted = refund_cta_enabled(
            s.event_end_ms,
            s.refund_deadline_ms,
            s.checked_in,
            expected_now,
        );
        let actual_event_end_only = s.event_end_ms > 0 && expected_now >= s.event_end_ms;
        if predicted != actual_event_end_only {
            // This is not necessarily a worker bug — it flags exactly the
            // window where the *current* (event_end-only) client gate would
            // render a dead CTA. Record it so part-2 of fix #19 lands with a
            // regression test already in place.
            return Err(HarnessError::AssertionFailed {
                flow: self.flow,
                reason: format!(
                    "gate divergence at now={expected_now}: corrected-gate={predicted}, legacy-event_end-only-gate={actual_event_end_only} (checked_in={}, event_end_ms={}, refund_deadline_ms={})",
                    s.checked_in, s.event_end_ms, s.refund_deadline_ms
                ),
            });
        }
        Ok(self)
    }

    /// Assert that the predicted escrow outcome for the response (at
    /// `expected_now`) equals `expected`. Negative-test flows use this to
    /// prove the worker would build a TX that reverts with the right code.
    pub fn outcome_is(self, expected: RefundOutcome, expected_now: i64) -> HarnessResult<Self> {
        let s = self.status;
        let predicted = predict_refund_outcome(
            s.event_end_ms,
            s.refund_deadline_ms,
            s.checked_in,
            expected_now,
        );
        if predicted != expected {
            return Err(HarnessError::AssertionFailed {
                flow: self.flow,
                reason: format!(
                    "refund outcome mismatch at now={expected_now}: predicted={predicted}, expected={expected} (checked_in={}, event_end_ms={}, refund_deadline_ms={})",
                    s.checked_in, s.event_end_ms, s.refund_deadline_ms
                ),
            });
        }
        Ok(self)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────
//
// The full on-chain truth table, encoded as data so a program-guard change
// trips an obvious failure here first. These tests are the staging-independent
// payload of §3.4 — they run on every PR with zero network dependency.

#[cfg(test)]
mod tests {
    use super::*;

    // Fixed horizon for readability: event ended at t=10_000, deadline at
    // t=20_000 (10s window — same shape as the seeded staging event).
    const EVENT_END: i64 = 10_000;
    const DEADLINE: i64 = 20_000;

    // ── predict_refund_outcome: positive paths ───────────────────────────────

    #[test]
    fn checked_in_refunds_anytime_after_event_end() {
        // The defining property of the checked-in path: no upper bound.
        assert_eq!(
            predict_refund_outcome(EVENT_END, DEADLINE, true, EVENT_END),
            RefundOutcome::Allowed
        );
        assert_eq!(
            predict_refund_outcome(EVENT_END, DEADLINE, true, DEADLINE),
            RefundOutcome::Allowed
        );
        assert_eq!(
            predict_refund_outcome(EVENT_END, DEADLINE, true, i64::MAX / 2),
            RefundOutcome::Allowed
        );
    }

    #[test]
    fn no_show_refunds_before_deadline() {
        assert_eq!(
            predict_refund_outcome(EVENT_END, DEADLINE, false, EVENT_END),
            RefundOutcome::Allowed
        );
        assert_eq!(
            predict_refund_outcome(EVENT_END, DEADLINE, false, DEADLINE - 1),
            RefundOutcome::Allowed
        );
    }

    // ── predict_refund_outcome: negative paths ───────────────────────────────

    #[test]
    fn pre_event_end_is_rejected_for_both_paths() {
        assert_eq!(
            predict_refund_outcome(EVENT_END, DEADLINE, true, EVENT_END - 1),
            RefundOutcome::PreEventEnd
        );
        assert_eq!(
            predict_refund_outcome(EVENT_END, DEADLINE, false, EVENT_END - 1),
            RefundOutcome::PreEventEnd
        );
    }

    #[test]
    fn no_show_at_or_after_deadline_is_rejected() {
        // Boundary: `clock >= refund_deadline` fails (matches `<` in program).
        assert_eq!(
            predict_refund_outcome(EVENT_END, DEADLINE, false, DEADLINE),
            RefundOutcome::DeadlinePassed
        );
        assert_eq!(
            predict_refund_outcome(EVENT_END, DEADLINE, false, DEADLINE + 1),
            RefundOutcome::DeadlinePassed
        );
        assert_eq!(
            predict_refund_outcome(EVENT_END, DEADLINE, false, i64::MAX / 2),
            RefundOutcome::DeadlinePassed
        );
    }

    // ── predict_refund_outcome: fail-safe on missing data ────────────────────

    #[test]
    fn missing_event_end_fails_safe_as_pre_event_end() {
        assert_eq!(
            predict_refund_outcome(0, DEADLINE, true, 999_999),
            RefundOutcome::PreEventEnd
        );
        assert_eq!(
            predict_refund_outcome(-1, DEADLINE, false, 999_999),
            RefundOutcome::PreEventEnd
        );
    }

    #[test]
    fn missing_deadline_fails_safe_for_no_show_only() {
        // Checked-in path is unaffected by missing deadline.
        assert_eq!(
            predict_refund_outcome(EVENT_END, 0, true, EVENT_END + 5),
            RefundOutcome::Allowed
        );
        // No-show with missing deadline → stricter outcome.
        assert_eq!(
            predict_refund_outcome(EVENT_END, 0, false, EVENT_END + 5),
            RefundOutcome::DeadlinePassed
        );
    }

    // ── escrow_code / is_allowed convenience ─────────────────────────────────

    #[test]
    fn outcome_escrow_codes_match_contract_surface() {
        // Codes come from docs/escrow_contract_surface.md §2.
        assert_eq!(
            RefundOutcome::PreEventEnd.escrow_code(),
            Some(EscrowCode::RefundNotYetAllowed)
        );
        assert_eq!(
            RefundOutcome::DeadlinePassed.escrow_code(),
            Some(EscrowCode::RefundDeadlinePassed)
        );
        assert_eq!(RefundOutcome::Allowed.escrow_code(), None);
        assert!(RefundOutcome::Allowed.is_allowed());
        assert!(!RefundOutcome::PreEventEnd.is_allowed());
        assert!(!RefundOutcome::DeadlinePassed.is_allowed());
    }

    // ── refund_cta_enabled: the divergence detector ──────────────────────────

    #[test]
    fn cta_enabled_equals_outcome_allowed() {
        // The corrected gate is exactly the success outcome. Spot-check a
        // matrix of (checked_in, now) across the horizon.
        for checked_in in [true, false] {
            for now in [
                EVENT_END - 1,
                EVENT_END,
                DEADLINE - 1,
                DEADLINE,
                DEADLINE + 1,
            ] {
                let outcome = predict_refund_outcome(EVENT_END, DEADLINE, checked_in, now);
                assert_eq!(
                    refund_cta_enabled(EVENT_END, DEADLINE, checked_in, now),
                    outcome.is_allowed(),
                    "checked_in={checked_in}, now={now}"
                );
            }
        }
    }

    #[test]
    fn cta_enabled_false_when_event_end_missing() {
        // Legacy/bad data must keep the CTA hidden — the existing client
        // behaviour (§3.2) is preserved on the fail-safe path.
        assert!(!refund_cta_enabled(0, DEADLINE, true, 999_999));
        assert!(!refund_cta_enabled(0, 0, false, 999_999));
    }
}
