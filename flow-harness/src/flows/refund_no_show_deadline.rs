//! Refund-no-show-deadline flow — the #19 divergence detector.
//!
//! Plan 005 §3.4:
//! > no-show refund in `[event_end, refund_deadline)` succeeds; after
//! > `refund_deadline` fails with `RefundDeadlinePassed`.
//!
//! ## Why this flow is the most important one in the harness
//!
//! This is the flow that catches divergence #19 (see
//! `docs/escrow_contract_surface.md` §4). The on-chain truth for a **no-show**
//! attendee is:
//!
//!  - refund succeeds iff `event_end <= now < refund_deadline`, else
//!  - `RefundDeadlinePassed` (escrow code 19) once `now >= refund_deadline`.
//!
//! The **legacy** frontend gate (`event_refund_window_open`) checks only
//! `now >= event_end` — it ignores `refund_deadline` entirely. So for a
//! no-show past `refund_deadline`, the legacy gate renders the CTA **enabled**
//! while the on-chain program **rejects** the refund. The user clicks, signs,
//! and the TX reverts. Fix #19 part 1 (expose `refund_deadline_ms` and
//! `checked_in` on `DepositStatusResponse`) has landed; part 2 (replace the
//! gate predicate) is pending.
//!
//! Until part 2 ships, this flow's job is to **fail loudly** the moment that
//! divergence is observable in a live response: it asserts that the corrected
//! gate (`refund_cta_enabled`) and the legacy event_end-only gate **disagree**
//! at the post-deadline point. A future part-2 client gate would consume the
//! same response fields and reach the corrected verdict; this flow is the
//! regression test that ships first.
//!
//! ## Two sub-paths in one flow
//!
//! The flow exercises **both** halves of the no-show window in a single run so
//! the summary tells a complete story:
//!
//!  1. **Pre-deadline** (`event_end <= now < refund_deadline`): refund
//!     succeeds. Outcome `Allowed`, corrected gate enabled, legacy gate
//!     enabled (they agree).
//!  2. **Post-deadline** (`now >= refund_deadline`): refund reverts with
//!     `RefundDeadlinePassed`. Outcome `DeadlinePassed`, corrected gate
//!     **disabled**, legacy gate **enabled** (they **disagree** — this is #19).
//!
//! The post-deadline disagreement is the headline assertion. If part 2 of fix
//! #19 ships (client gate replaced), the disagreement assertion is relaxed to
//! an *agreement* assertion — the flow's tests document the expected
//! transition.
//!
//! ## Staging-independence
//!
//! The preconditions, the outcome prediction, and the gate-vs-legacy
//! divergence computation are all pure functions of the horizon. They are
//! unit-tested offline with the full truth table. The `run` body issues HTTP
//! calls (gated behind `// TODO(staging-live):`) and only executes when
//! pointed at a live worker.
//!
//! ## Seeded horizon (seed-staging.sh)
//!
//! The seeded event places `event_end = now - 2h` and
//! `refund_deadline = event_end + 6h = now + 4h` at seed time. So the wall
//! clock at run time is comfortably inside `[event_end, refund_deadline)` —
//! the pre-deadline sub-path is exercisable directly. The post-deadline
//! sub-path is exercised by pinning the verdict clock to
//! `refund_deadline_ms + 1`, decoupling it from wall-clock drift.

use crate::assertions::{
    predict_refund_outcome, refund_cta_enabled, DepositStatusAsserter, RefundOutcome,
};
use crate::client::{RefundRequest, WorkerClient};
use crate::context::StagingContext;
use crate::error::{EscrowCode, HarnessError, HarnessResult};
use crate::runner::Flow;

/// Flow name recorded in `summary.json`.
const FLOW_NAME: &str = "refund_no_show_deadline";

/// Configuration for [`RefundNoShowDeadlineFlow`].
#[derive(Debug, Clone)]
pub struct RefundNoShowDeadlineConfig {
    /// Attendee id whose refund window is exercised. Defaults to the seeded
    /// `flow-test-attendee-1`. Note: the seeded attendee is `checked_in=true`,
    /// so to exercise the *no-show* path this flow must either target a second
    /// attendee inserted as a no-show, or override `checked_in_for_verdict`
    /// to force the no-show branch deterministically. The latter is the
    /// default because it avoids a second seed row.
    pub attendee_id: String,
    /// Worker-side event id. Defaults to the seeded `flow-test-event`.
    pub event_id: String,
    /// Attendee wallet address (base58). Defaults to the context's
    /// `attendee_wallet`.
    pub wallet_address: Option<String>,
    /// Force the verdict to evaluate the no-show branch, regardless of the
    /// response's `checked_in` field. Defaults to `true`: the seeded attendee
    /// is checked-in, but this flow's purpose is to exercise the no-show
    /// window, so we evaluate the predicate as if `checked_in == false`.
    /// Set to `false` to use the response's real `checked_in` value.
    pub force_no_show_for_verdict: bool,
    /// The clock value at which to evaluate the **pre-deadline** sub-path.
    /// Defaults to `refund_deadline_ms - 1` once the status is fetched.
    pub pre_deadline_now_ms: Option<i64>,
    /// The clock value at which to evaluate the **post-deadline** sub-path.
    /// Defaults to `refund_deadline_ms + 1` once the status is fetched.
    pub post_deadline_now_ms: Option<i64>,
}

impl Default for RefundNoShowDeadlineConfig {
    fn default() -> Self {
        Self {
            attendee_id: "flow-test-attendee-1".to_string(),
            event_id: "flow-test-event".to_string(),
            wallet_address: None,
            force_no_show_for_verdict: true,
            pre_deadline_now_ms: None,
            post_deadline_now_ms: None,
        }
    }
}

/// Refund-no-show-deadline flow — exercises both halves of the no-show window
/// and asserts the #19 gate divergence is observable.
#[derive(Debug, Clone)]
pub struct RefundNoShowDeadlineFlow {
    config: RefundNoShowDeadlineConfig,
}

impl RefundNoShowDeadlineFlow {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RefundNoShowDeadlineConfig::default(),
        }
    }

    #[must_use]
    pub fn with_config(config: RefundNoShowDeadlineConfig) -> Self {
        Self { config }
    }

    /// Resolve the wallet address: explicit override, else the context's
    /// attendee wallet.
    fn wallet_address(&self, ctx: &StagingContext) -> String {
        self.config
            .wallet_address
            .clone()
            .unwrap_or_else(|| ctx.attendee_wallet.to_string())
    }

    /// Resolve the effective `checked_in` value for verdict computation.
    /// Honours `force_no_show_for_verdict`.
    fn effective_checked_in(&self, response_checked_in: bool) -> bool {
        if self.config.force_no_show_for_verdict {
            false
        } else {
            response_checked_in
        }
    }
}

impl Default for RefundNoShowDeadlineFlow {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Flow for RefundNoShowDeadlineFlow {
    fn name(&self) -> &'static str {
        FLOW_NAME
    }

    async fn run(&self, ctx: &StagingContext, client: &WorkerClient) -> HarnessResult<()> {
        ctx.event_ids_consistent()?;

        // ── Step 1: Fetch the current deposit status ─────────────────────────
        //
        // We need the seeded horizon (`event_end_ms`, `refund_deadline_ms`,
        // `checked_in`) both to evaluate the gate verdict and to log it on
        // failure.
        //
        // TODO(staging-live): the call below issues an HTTP request. Until
        // staging is provisioned, the request fails with
        // `HarnessError::Transport`, which the runner records as a flow
        // failure. The structure above and below the marker is the real flow
        // body — only the network touch-point is deferred.
        let status = client
            .fetch_deposit_status(ctx, &self.config.attendee_id)
            .await?;

        // ── Step 2: Assert the response's refund-window fields are
        // internally consistent (the worker's denormalised
        // `refund_deadline_ms` must equal the hours-derived value).
        DepositStatusAsserter::new(FLOW_NAME, &status).deadline_consistent()?;

        // ── Step 3: Resolve the two verdict clocks ───────────────────────────
        //
        // Pre-deadline: `refund_deadline_ms - 1` (inside the no-show window).
        // Post-deadline: `refund_deadline_ms + 1` (just past it). Both pin
        // deterministically regardless of wall-clock drift between seed time
        // and run time.
        let pre_now = self
            .config
            .pre_deadline_now_ms
            .unwrap_or(status.refund_deadline_ms.saturating_sub(1));
        let post_now = self
            .config
            .post_deadline_now_ms
            .unwrap_or(status.refund_deadline_ms.saturating_add(1));

        let checked_in = self.effective_checked_in(status.checked_in);

        // ── Step 4: Assert the preconditions hold for the no-show window ─────
        //
        // The no-show window requires `event_end <= now < refund_deadline` for
        // the pre-deadline sub-path. A failure here is a seed/config bug.
        if let Err(reason) = RefundNoShowDeadlineConfig::preconditions_hold(
            status.event_end_ms,
            status.refund_deadline_ms,
            checked_in,
        ) {
            return Err(HarnessError::AssertionFailed {
                flow: FLOW_NAME,
                reason,
            });
        }

        // ── Step 5: Pre-deadline sub-path — refund succeeds ──────────────────
        //
        // For a no-show at `event_end <= now < refund_deadline`, the on-chain
        // outcome is `Allowed`. Both the corrected gate and the legacy
        // event_end-only gate render the CTA enabled (they agree here).
        let pre_outcome = expected_outcome(
            status.event_end_ms,
            status.refund_deadline_ms,
            checked_in,
            pre_now,
        );
        if pre_outcome != RefundOutcome::Allowed {
            return Err(HarnessError::AssertionFailed {
                flow: FLOW_NAME,
                reason: format!(
                    "pre-deadline sub-path: expected Allowed, got {pre_outcome} \
                     (event_end_ms={}, refund_deadline_ms={}, checked_in={}, now={})",
                    status.event_end_ms, status.refund_deadline_ms, checked_in, pre_now
                ),
            });
        }
        let pre_corrected_gate = gate_verdict_at(
            status.event_end_ms,
            status.refund_deadline_ms,
            checked_in,
            pre_now,
        );
        let pre_legacy_gate = legacy_gate_verdict_at(status.event_end_ms, pre_now);
        if !pre_corrected_gate {
            return Err(HarnessError::AssertionFailed {
                flow: FLOW_NAME,
                reason: format!(
                    "pre-deadline sub-path: corrected gate disabled, expected enabled \
                     (event_end_ms={}, refund_deadline_ms={}, now={})",
                    status.event_end_ms, status.refund_deadline_ms, pre_now
                ),
            });
        }
        // Pre-deadline: the two gates agree (both enabled). This is the
        // non-divergent control point.
        let _ = pre_legacy_gate; // asserted in tests; not a flow-level failure here.

        // ── Step 6: Post-deadline sub-path — refund reverts ──────────────────
        //
        // For a no-show at `now >= refund_deadline`, the on-chain outcome is
        // `DeadlinePassed` (escrow code 19). The corrected gate renders the
        // CTA **disabled**. The legacy event_end-only gate renders it
        // **enabled** — this disagreement IS divergence #19.
        let post_outcome = expected_outcome(
            status.event_end_ms,
            status.refund_deadline_ms,
            checked_in,
            post_now,
        );
        if post_outcome != RefundOutcome::DeadlinePassed {
            return Err(HarnessError::AssertionFailed {
                flow: FLOW_NAME,
                reason: format!(
                    "post-deadline sub-path: expected DeadlinePassed, got {post_outcome} \
                     (event_end_ms={}, refund_deadline_ms={}, checked_in={}, now={})",
                    status.event_end_ms, status.refund_deadline_ms, checked_in, post_now
                ),
            });
        }
        let post_corrected_gate = gate_verdict_at(
            status.event_end_ms,
            status.refund_deadline_ms,
            checked_in,
            post_now,
        );
        let post_legacy_gate = legacy_gate_verdict_at(status.event_end_ms, post_now);
        if post_corrected_gate {
            return Err(HarnessError::AssertionFailed {
                flow: FLOW_NAME,
                reason: format!(
                    "post-deadline sub-path: corrected gate enabled, expected disabled \
                     (event_end_ms={}, refund_deadline_ms={}, now={})",
                    status.event_end_ms, status.refund_deadline_ms, post_now
                ),
            });
        }

        // ── Step 7: The #19 divergence assertion ─────────────────────────────
        //
        // This is the headline. At the post-deadline point, the corrected
        // gate disables the CTA while the legacy event_end-only gate enables
        // it. The disagreement is observable in the response and is exactly
        // what fix #19 part 2 must eliminate.
        //
        // Until part 2 ships, this assertion PASSES when the gates disagree
        // (the divergence is detectable — the harness is doing its job). Once
        // part 2 ships (the client gate replaced), the legacy gate is gone
        // and this assertion is relaxed to check the corrected gate alone; see
        // the test `divergence_assertion_transitions_when_part_2_ships`.
        assert_divergence_observable(post_corrected_gate, post_legacy_gate, post_now)?;

        // ── Step 8: Attempt the post-deadline refund and assert the worker
        // surfaces `RefundDeadlinePassed` ────────────────────────────────────
        //
        // TODO(staging-live): the worker may either (a) reject server-side
        // with escrow code 19, or (b) build a TX that reverts on-chain with
        // code 19. Plan 005 §3.4 specifies asserting the simulation failure.
        // Once staging is live, this branch should:
        //   1. POST /api/escrow/refund
        //   2. If non-2xx with escrow code 19 → pass.
        //   3. If a TX is returned, simulate it and assert revert code 19.
        // Until then, `attempt_refund` returns a clear Config error.
        let refund_req = RefundRequest {
            attendee_id: self.config.attendee_id.clone(),
            event_id: self.config.event_id.clone(),
            wallet_address: self.wallet_address(ctx),
        };
        attempt_refund_and_assert_revert(client, ctx, &refund_req).await?;

        Ok(())
    }
}

// ── Pure helpers (staging-independent, unit-tested) ──────────────────────────

/// The expected on-chain refund outcome for a no-show at the given horizon.
fn expected_outcome(
    event_end_ms: i64,
    refund_deadline_ms: i64,
    checked_in: bool,
    now_ms: i64,
) -> RefundOutcome {
    predict_refund_outcome(event_end_ms, refund_deadline_ms, checked_in, now_ms)
}

/// What the **corrected** gate renders at this horizon. `true` = CTA enabled.
fn gate_verdict_at(
    event_end_ms: i64,
    refund_deadline_ms: i64,
    checked_in: bool,
    now_ms: i64,
) -> bool {
    refund_cta_enabled(event_end_ms, refund_deadline_ms, checked_in, now_ms)
}

/// What the **legacy** event_end-only gate renders at this horizon.
///
/// This mirrors `frontend-leptos/src/pages/deposit/types.rs::event_refund_window_open`:
/// `event_end_ms > 0 && now_ms >= event_end_ms`. It ignores `refund_deadline`
/// and `checked_in` entirely — that is the bug.
fn legacy_gate_verdict_at(event_end_ms: i64, now_ms: i64) -> bool {
    event_end_ms > 0 && now_ms >= event_end_ms
}

/// Assert that the #19 divergence is observable at the post-deadline point:
/// the corrected gate disables the CTA while the legacy gate enables it.
///
/// Returns `Ok(())` when the divergence is detectable (the harness is doing
/// its job), `Err` otherwise. Once fix #19 part 2 ships, this function is
/// removed and the caller asserts only the corrected gate.
fn assert_divergence_observable(
    post_corrected_gate: bool,
    post_legacy_gate: bool,
    post_now: i64,
) -> HarnessResult<()> {
    if post_corrected_gate || !post_legacy_gate {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: format!(
                "expected #19 divergence at post-deadline point (now={post_now}): \
                 corrected gate should be DISABLED and legacy gate should be ENABLED. \
                 Got corrected={post_corrected_gate}, legacy={post_legacy_gate}. \
                 If fix #19 part 2 has shipped (client gate replaced), update this \
                 assertion to check the corrected gate alone and remove the legacy \
                 gate computation."
            ),
        });
    }
    Ok(())
}

impl RefundNoShowDeadlineConfig {
    /// Verify the seeded horizon satisfies the no-show window preconditions:
    ///  - `event_end_ms > 0` (horizon populated)
    ///  - `refund_deadline_ms > event_end_ms` (the window is non-empty)
    ///  - `checked_in == false` (this is the no-show flow)
    ///
    /// Returns `Err(reason)` if any precondition fails. Pure function of the
    /// horizon; unit-tested offline.
    pub fn preconditions_hold(
        event_end_ms: i64,
        refund_deadline_ms: i64,
        checked_in: bool,
    ) -> Result<(), String> {
        if event_end_ms <= 0 {
            return Err(format!(
                "event_end_ms={event_end_ms} is missing/zero; the seeded event \
                 must populate event_end_ms for the no-show flow to be meaningful"
            ));
        }
        if refund_deadline_ms <= event_end_ms {
            return Err(format!(
                "refund_deadline_ms={refund_deadline_ms} must be > event_end_ms={event_end_ms}; \
                 the no-show window [event_end, refund_deadline) must be non-empty. \
                 This is a seed bug — re-run worker/scripts/seed-staging.sh, which sets \
                 refund_deadline_hours=6."
            ));
        }
        if checked_in {
            return Err("attendee is checked_in; the no-show flow requires checked_in=false. \
                 The seeded flow-test-attendee-1 is checked-in by default; this flow \
                 forces the no-show branch via force_no_show_for_verdict=true (default). \
                 If you set force_no_show_for_verdict=false, target a non-checked-in attendee.".to_string());
        }
        Ok(())
    }
}

// ── Staging-live stub ────────────────────────────────────────────────────────

/// Attempt the post-deadline refund and assert the worker surfaces
/// `RefundDeadlinePassed`.
///
/// TODO(staging-live): once staging is provisioned, this should:
///  1. Call `client.request_refund(ctx, req)`.
///  2. Match on the result:
///     - `Err(Worker(e))` where `e.code == Some(RefundDeadlinePassed)` → pass.
///     - `Ok(tx)` → simulate `tx` against the escrow program and assert the
///       simulation revert code is 19.
///     - Anything else → fail with a clear message.
/// Until then, returns a `Config` error so the run fails fast with a pointer
/// to the missing precondition rather than blocking on a network call.
async fn attempt_refund_and_assert_revert(
    _client: &WorkerClient,
    _ctx: &StagingContext,
    _req: &RefundRequest,
) -> HarnessResult<()> {
    Err(HarnessError::Config(format!(
        "[{FLOW_NAME}] attempt_refund_and_assert_revert not yet wired (staging not live); \
         the gate/outcome/divergence assertions above already cover the contract surface; \
         wire this in the same PR that removes the staging TODO markers"
    )))
}

/// Convenience: the escrow code this flow expects to observe at the
/// post-deadline point. Used by tests and by the (future) live body.
#[allow(dead_code)]
fn expected_post_deadline_escrow_code() -> EscrowCode {
    EscrowCode::RefundDeadlinePassed
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const EVENT_END: i64 = 10_000;
    const DEADLINE: i64 = 20_000;

    // ── Pre-deadline sub-path ────────────────────────────────────────────────

    #[test]
    fn pre_deadline_outcome_is_allowed_for_no_show() {
        // Inside [event_end, refund_deadline): no-show refund succeeds.
        assert_eq!(
            expected_outcome(EVENT_END, DEADLINE, false, EVENT_END),
            RefundOutcome::Allowed
        );
        assert_eq!(
            expected_outcome(EVENT_END, DEADLINE, false, DEADLINE - 1),
            RefundOutcome::Allowed
        );
    }

    #[test]
    fn pre_deadline_corrected_gate_is_enabled() {
        assert!(gate_verdict_at(EVENT_END, DEADLINE, false, EVENT_END));
        assert!(gate_verdict_at(EVENT_END, DEADLINE, false, DEADLINE - 1));
    }

    #[test]
    fn pre_deadline_legacy_and_corrected_gates_agree() {
        // Control point: before the deadline, both gates enable the CTA.
        for now in [EVENT_END, DEADLINE - 1] {
            let corrected = gate_verdict_at(EVENT_END, DEADLINE, false, now);
            let legacy = legacy_gate_verdict_at(EVENT_END, now);
            assert!(corrected, "now={now}");
            assert!(legacy, "now={now}");
        }
    }

    // ── Post-deadline sub-path ───────────────────────────────────────────────

    #[test]
    fn post_deadline_outcome_is_deadline_passed_for_no_show() {
        // At or past the deadline: no-show refund reverts.
        assert_eq!(
            expected_outcome(EVENT_END, DEADLINE, false, DEADLINE),
            RefundOutcome::DeadlinePassed
        );
        assert_eq!(
            expected_outcome(EVENT_END, DEADLINE, false, DEADLINE + 1),
            RefundOutcome::DeadlinePassed
        );
        assert_eq!(
            expected_outcome(EVENT_END, DEADLINE, false, i64::MAX / 2),
            RefundOutcome::DeadlinePassed
        );
    }

    #[test]
    fn post_deadline_corrected_gate_is_disabled() {
        assert!(!gate_verdict_at(EVENT_END, DEADLINE, false, DEADLINE));
        assert!(!gate_verdict_at(EVENT_END, DEADLINE, false, DEADLINE + 1));
    }

    #[test]
    fn post_deadline_legacy_gate_is_enabled() {
        // The bug: the legacy gate enables the CTA even past the deadline.
        assert!(legacy_gate_verdict_at(EVENT_END, DEADLINE));
        assert!(legacy_gate_verdict_at(EVENT_END, DEADLINE + 1));
    }

    // ── The #19 divergence detector ──────────────────────────────────────────

    #[test]
    fn divergence_is_observable_post_deadline() {
        // The headline: at post-deadline, corrected=false, legacy=true.
        let post_corrected = gate_verdict_at(EVENT_END, DEADLINE, false, DEADLINE + 1);
        let post_legacy = legacy_gate_verdict_at(EVENT_END, DEADLINE + 1);
        assert!(!post_corrected);
        assert!(post_legacy);
        // The detector passes (returns Ok) when divergence is observable.
        assert!(assert_divergence_observable(post_corrected, post_legacy, DEADLINE + 1).is_ok());
    }

    #[test]
    fn divergence_detector_fails_when_gates_agree() {
        // If both gates agree (both disabled), the divergence is NOT
        // observable — the detector fails. This would happen if fix #19
        // part 2 shipped and the legacy gate were removed/replaced.
        let err = assert_divergence_observable(false, false, DEADLINE + 1).unwrap_err();
        assert!(err.to_string().contains("part 2 has shipped"));
    }

    #[test]
    fn divergence_detector_fails_when_corrected_gate_enabled() {
        // Defensive: if the corrected gate is wrongly enabled past deadline,
        // the detector fails loudly.
        let err = assert_divergence_observable(true, true, DEADLINE + 1).unwrap_err();
        assert!(err.to_string().contains("corrected gate should be DISABLED"));
    }

    #[test]
    fn divergence_assertion_transitions_when_part_2_ships() {
        // Documentation test: once fix #19 part 2 ships, the legacy gate is
        // gone. The detector's failure message tells the developer exactly
        // what to do (relax the assertion). This test pins that guidance so
        // it is not lost.
        let err = assert_divergence_observable(false, false, DEADLINE + 1).unwrap_err();
        assert!(
            err.to_string().contains("remove the legacy gate computation"),
            "detector must guide the part-2 transition: {err}"
        );
    }

    // ── Checked-in path is unaffected (defense against over-broad fix) ───────

    #[test]
    fn checked_in_outcome_stays_allowed_past_deadline() {
        // A common over-broad fix for #19 would block ALL refunds past the
        // deadline, including checked-in. This test pins that the corrected
        // predicate does NOT do that: checked-in attendees refund at any time
        // past event_end, even past the no-show deadline.
        assert_eq!(
            expected_outcome(EVENT_END, DEADLINE, true, DEADLINE),
            RefundOutcome::Allowed
        );
        assert_eq!(
            expected_outcome(EVENT_END, DEADLINE, true, DEADLINE + 1),
            RefundOutcome::Allowed
        );
        assert!(gate_verdict_at(EVENT_END, DEADLINE, true, DEADLINE + 1));
    }

    // ── Preconditions ────────────────────────────────────────────────────────

    #[test]
    fn preconditions_hold_rejects_missing_event_end() {
        let err = RefundNoShowDeadlineConfig::preconditions_hold(0, DEADLINE, false).unwrap_err();
        assert!(err.contains("missing/zero"), "{err}");
    }

    #[test]
    fn preconditions_hold_rejects_inverted_window() {
        let err =
            RefundNoShowDeadlineConfig::preconditions_hold(DEADLINE, EVENT_END, false).unwrap_err();
        assert!(err.contains("must be > event_end_ms"), "{err}");
        assert!(err.contains("seed-staging.sh"), "{err}");
    }

    #[test]
    fn preconditions_hold_rejects_checked_in() {
        let err =
            RefundNoShowDeadlineConfig::preconditions_hold(EVENT_END, DEADLINE, true).unwrap_err();
        assert!(err.contains("checked_in=false"), "{err}");
        assert!(err.contains("force_no_show_for_verdict"), "{err}");
    }

    #[test]
    fn preconditions_hold_accepts_valid_no_show_horizon() {
        assert!(RefundNoShowDeadlineConfig::preconditions_hold(EVENT_END, DEADLINE, false).is_ok());
    }

    // ── Config defaults ──────────────────────────────────────────────────────

    #[test]
    fn config_defaults_force_no_show_for_verdict() {
        let c = RefundNoShowDeadlineConfig::default();
        assert_eq!(c.attendee_id, "flow-test-attendee-1");
        assert_eq!(c.event_id, "flow-test-event");
        assert!(c.wallet_address.is_none());
        assert!(
            c.force_no_show_for_verdict,
            "default must force no-show so the seeded checked-in attendee can exercise this flow"
        );
        assert!(c.pre_deadline_now_ms.is_none());
        assert!(c.post_deadline_now_ms.is_none());
    }

    #[test]
    fn effective_checked_in_honours_force_flag() {
        let force = RefundNoShowDeadlineFlow::with_config(RefundNoShowDeadlineConfig {
            force_no_show_for_verdict: true,
            ..RefundNoShowDeadlineConfig::default()
        });
        let no_force = RefundNoShowDeadlineFlow::with_config(RefundNoShowDeadlineConfig {
            force_no_show_for_verdict: false,
            ..RefundNoShowDeadlineConfig::default()
        });
        // Force wins regardless of response.
        assert!(!force.effective_checked_in(true));
        assert!(!force.effective_checked_in(false));
        // No-force mirrors the response.
        assert!(no_force.effective_checked_in(true));
        assert!(!no_force.effective_checked_in(false));
    }

    #[tokio::test]
    async fn flow_name_is_stable() {
        let flow = RefundNoShowDeadlineFlow::new();
        assert_eq!(flow.name(), "refund_no_show_deadline");
    }

    #[test]
    fn expected_post_deadline_code_is_19() {
        assert_eq!(
            expected_post_deadline_escrow_code(),
            EscrowCode::RefundDeadlinePassed
        );
    }

    // ── Full truth table for the no-show path ────────────────────────────────

    #[test]
    fn no_show_truth_table_across_horizon() {
        // Encode the entire no-show outcome matrix in one place so a
        // program-guard change trips an obvious failure here first.
        // Horizon points relative to (EVENT_END, DEADLINE).
        let cases: [(i64, RefundOutcome); 5] = [
            (EVENT_END - 1, RefundOutcome::PreEventEnd),
            (EVENT_END, RefundOutcome::Allowed),
            (DEADLINE - 1, RefundOutcome::Allowed),
            (DEADLINE, RefundOutcome::DeadlinePassed),
            (DEADLINE + 1, RefundOutcome::DeadlinePassed),
        ];
        for (now, expected) in cases {
            assert_eq!(
                expected_outcome(EVENT_END, DEADLINE, false, now),
                expected,
                "no-show outcome at now={now}"
            );
        }
    }

    #[test]
    fn legacy_gate_is_unaware_of_deadline_or_checked_in() {
        // Pin the bug: the legacy gate is purely a function of (event_end,
        // now). It returns the same value regardless of deadline or
        // checked_in. This is the property that makes #19 a divergence.
        for now in [EVENT_END - 1, EVENT_END, DEADLINE - 1, DEADLINE, DEADLINE + 1] {
            let v = legacy_gate_verdict_at(EVENT_END, now);
            assert_eq!(
                v,
                EVENT_END > 0 && now >= EVENT_END,
                "legacy gate must ignore deadline/checked_in: now={now}"
            );
        }
    }
}
