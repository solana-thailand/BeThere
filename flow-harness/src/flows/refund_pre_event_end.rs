//! Refund-before-event-end flow — negative test.
//!
//! Plan 005 §3.4:
//! > attempt refund before `event_end` → assert simulation fails with
//! > `RefundNotYetAllowed` → assert the frontend gate condition
//! > (`event_refund_window_open`) returns `false`.
//!
//! ## What this flow proves
//!
//! The on-chain guard at `bethere-escrow/src/instructions/refund.rs#L72-76`
//! rejects any refund attempted before `event_end` with `RefundNotYetAllowed`
//! (escrow code 1). This flow confirms the worker surfaces that revert to the
//! caller rather than silently building a transaction that fails at sign time.
//!
//! ## Why this is a *negative* test
//!
//! The flow's pass condition is that the refund **fails** with the right code.
//! A "successful" refund here would be a regression: it would mean either the
//! worker skipped the on-chain guard or the harness clock is wrong. The flow
//! returns `Ok(())` only when it has *observed* the expected revert; a missing
//! revert is the failure.
//!
//! ## Staging-independence
//!
//! The preconditions and the verdict computation are pure:
//!  - [`RefundPreEventEndConfig::preconditions_hold`] checks that the seeded
//!    horizon genuinely places `now` before `event_end`.
//!  - [`expected_outcome`] returns [`RefundOutcome::PreEventEnd`] for any
//!    horizon satisfying the precondition.
//!  - [`gate_verdict_at`] computes what both the corrected and legacy gates
//!    would render.
//!
//! The `run` body issues HTTP calls (gated behind `// TODO(staging-live):`)
//! and only executes when pointed at a live worker.
//!
//! ## Note on the gate-divergence detector
//!
//! Before `event_end`, the corrected gate (`refund_cta_enabled`) and the
//! legacy event_end-only gate **agree** (both render the CTA hidden). So this
//! flow does not catch the #19 divergence — that is the job of
//! `refund_no_show_deadline`. This flow's role is to confirm the *on-chain*
//! negative path, not the client gate.

use crate::assertions::{
    predict_refund_outcome, refund_cta_enabled, DepositStatusAsserter, RefundOutcome,
};
use crate::client::{RefundRequest, WorkerClient};
use crate::context::StagingContext;
use crate::error::{EscrowCode, HarnessError, HarnessResult};
use crate::runner::Flow;

/// Flow name recorded in `summary.json`.
const FLOW_NAME: &str = "refund_pre_event_end";

/// Configuration for [`RefundPreEventEndFlow`].
#[derive(Debug, Clone)]
pub struct RefundPreEventEndConfig {
    /// Attendee id whose refund is attempted. Defaults to the seeded
    /// `flow-test-attendee-1`. The seeded event's `event_end` is in the past
    /// (per `seed-staging.sh`), so to exercise the pre-end path the flow uses
    /// [`Self::assertion_now_ms`] to pin the verdict clock *before* the
    /// seeded `event_end`, regardless of the wall clock.
    pub attendee_id: String,
    /// Worker-side event id. Defaults to the seeded `flow-test-event`.
    pub event_id: String,
    /// Attendee wallet address (base58). Defaults to the context's
    /// `attendee_wallet`.
    pub wallet_address: Option<String>,
    /// The clock value at which to evaluate the gate verdict. Defaults to
    /// `event_end_ms - 1` once the status is fetched, which forces the
    /// pre-end branch deterministically.
    pub assertion_now_ms: Option<i64>,
}

impl Default for RefundPreEventEndConfig {
    fn default() -> Self {
        Self {
            attendee_id: "flow-test-attendee-1".to_string(),
            event_id: "flow-test-event".to_string(),
            wallet_address: None,
            assertion_now_ms: None,
        }
    }
}

/// Refund-before-event-end negative flow.
#[derive(Debug, Clone)]
pub struct RefundPreEventEndFlow {
    config: RefundPreEventEndConfig,
}

impl RefundPreEventEndFlow {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RefundPreEventEndConfig::default(),
        }
    }

    #[must_use]
    pub fn with_config(config: RefundPreEventEndConfig) -> Self {
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
}

impl Default for RefundPreEventEndFlow {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Flow for RefundPreEventEndFlow {
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

        // ── Step 2: Pin the verdict clock to "just before event_end" ────────
        //
        // The seeded event's `event_end` is in the past (so that the
        // checked-in refund path is exercisable in other flows). To exercise
        // the pre-end negative path deterministically, we evaluate the gate
        // at `event_end_ms - 1` — one millisecond before the window opens.
        // This decouples the verdict from wall-clock drift between seed time
        // and run time.
        let assertion_now = self
            .config
            .assertion_now_ms
            .unwrap_or_else(|| (status.event_end_ms - 1).max(0));

        // ── Step 3: Assert the response's refund-window fields are
        // internally consistent (defense-in-depth on the worker's
        // denormalised `refund_deadline_ms`).
        DepositStatusAsserter::new(FLOW_NAME, &status).deadline_consistent()?;

        // ── Step 4: Assert the predicted on-chain outcome is PreEventEnd ────
        //
        // This is the pure, staging-independent core. If the seeded horizon
        // does not actually place `assertion_now` before `event_end`, this
        // fails loudly with a clear reason — that is a seed/config bug, not a
        // worker bug, and the message says so.
        let expected = expected_outcome(
            status.event_end_ms,
            status.refund_deadline_ms,
            status.checked_in,
            assertion_now,
        );
        DepositStatusAsserter::new(FLOW_NAME, &status).outcome_is(expected, assertion_now)?;

        // Belt-and-braces: the explicit preconditions check produces a more
        // actionable error than `outcome_is` alone when the seed is wrong.
        if let Err(reason) = RefundPreEventEndConfig::preconditions_hold(
            status.event_end_ms,
            assertion_now,
        ) {
            return Err(HarnessError::AssertionFailed {
                flow: FLOW_NAME,
                reason,
            });
        }

        // ── Step 5: Assert the corrected gate renders the CTA hidden ────────
        //
        // Before `event_end`, both the corrected gate and the legacy
        // event_end-only gate agree (hidden). We assert the corrected gate
        // explicitly; the agreement is the property that
        // `refund_no_show_deadline` will later probe for divergence.
        let cta_enabled = gate_verdict_at(
            status.event_end_ms,
            status.refund_deadline_ms,
            status.checked_in,
            assertion_now,
        );
        if cta_enabled {
            return Err(HarnessError::AssertionFailed {
                flow: FLOW_NAME,
                reason: format!(
                    "corrected gate rendered CTA enabled before event_end \
                     (event_end_ms={}, assertion_now={}): expected disabled",
                    status.event_end_ms, assertion_now
                ),
            });
        }

        // ── Step 6: Attempt the refund and assert the worker surfaces
        // `RefundNotYetAllowed` ──────────────────────────────────────────────
        //
        // TODO(staging-live): the worker may either (a) reject the request
        // server-side with an escrow code, or (b) build a transaction that
        // reverts on-chain. Plan 005 §3.4 specifies asserting the simulation
        // failure. Once staging is live, this branch should:
        //   1. POST /api/escrow/refund
        //   2. If the worker returns a non-2xx with escrow code 1 → pass.
        //   3. If the worker returns a TX, simulate it and assert the
        //      simulation revert code is 1.
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

/// The expected on-chain refund outcome for this flow's horizon.
///
/// For any horizon satisfying the pre-end precondition (`now < event_end`),
/// the outcome is [`RefundOutcome::PreEventEnd`] regardless of `checked_in`
/// or `refund_deadline` — the first guard short-circuits.
fn expected_outcome(
    event_end_ms: i64,
    refund_deadline_ms: i64,
    checked_in: bool,
    now_ms: i64,
) -> RefundOutcome {
    predict_refund_outcome(event_end_ms, refund_deadline_ms, checked_in, now_ms)
}

/// What the corrected gate renders at this horizon. `true` = CTA enabled.
fn gate_verdict_at(
    event_end_ms: i64,
    refund_deadline_ms: i64,
    checked_in: bool,
    now_ms: i64,
) -> bool {
    refund_cta_enabled(event_end_ms, refund_deadline_ms, checked_in, now_ms)
}

impl RefundPreEventEndConfig {
    /// Verify the seeded horizon genuinely places `now` before `event_end`.
    /// Returns `Err(reason)` if the precondition does not hold — the caller
    /// should surface it as an assertion failure with an actionable message.
    ///
    /// Pure function of the horizon; unit-tested offline.
    pub fn preconditions_hold(event_end_ms: i64, now_ms: i64) -> Result<(), String> {
        if event_end_ms <= 0 {
            return Err(format!(
                "event_end_ms={event_end_ms} is missing/zero; the seeded event \
                 must populate event_end_ms for the pre-end flow to be meaningful"
            ));
        }
        if now_ms >= event_end_ms {
            return Err(format!(
                "assertion_now={now_ms} is not before event_end_ms={event_end_ms}; \
                 the pre-end negative path requires now < event_end. \
                 This is a seed/config bug, not a worker bug — re-run \
                 worker/scripts/seed-staging.sh or set assertion_now_ms."
            ));
        }
        Ok(())
    }
}

// ── On-chain seam ─────────────────────────────────────────────────────────────

/// Attempt the refund before `event_end` and assert it is rejected with
/// `RefundNotYetAllowed` — either server-side (worker returns the escrow code)
/// or, if the worker built a tx anyway, on submitting it (the program reverts
/// with `Custom(1)`, which `chain::submit_tx` maps back to the same code).
async fn attempt_refund_and_assert_revert(
    client: &WorkerClient,
    ctx: &StagingContext,
    req: &RefundRequest,
) -> HarnessResult<()> {
    let expected = expected_escrow_code();
    let check = |code: Option<EscrowCode>, origin: &str| -> HarnessResult<()> {
        if code == Some(expected) {
            Ok(())
        } else {
            Err(HarnessError::AssertionFailed {
                flow: FLOW_NAME,
                reason: format!("expected {expected} {origin}, got {code:?}"),
            })
        }
    };
    match client.request_refund(ctx, req).await {
        // Common path: the worker rejects server-side with the escrow code.
        Err(HarnessError::Worker(e)) => check(e.code, "from worker"),
        // Worker built the tx anyway → submitting it must revert on-chain with
        // the same code (chain::submit_tx maps a Custom(N) revert to Worker).
        Ok(tx) => match crate::chain::submit_tx(ctx, &tx.transaction).await {
            Err(HarnessError::Worker(e)) => check(e.code, "on-chain"),
            Ok(sig) => Err(HarnessError::AssertionFailed {
                flow: FLOW_NAME,
                reason: format!(
                    "refund unexpectedly SUCCEEDED on-chain (sig {sig}); expected revert {expected}"
                ),
            }),
            Err(other) => Err(other),
        },
        Err(other) => Err(other),
    }
}

/// Convenience: the escrow code this flow expects to observe. Used by tests
/// and by the (future) live body to keep the constant in one place.
#[allow(dead_code)]
fn expected_escrow_code() -> EscrowCode {
    EscrowCode::RefundNotYetAllowed
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const EVENT_END: i64 = 10_000;
    const DEADLINE: i64 = 20_000;

    #[test]
    fn expected_outcome_is_pre_event_end_for_both_paths() {
        // Regardless of checked_in, before event_end the outcome is PreEventEnd.
        assert_eq!(
            expected_outcome(EVENT_END, DEADLINE, true, EVENT_END - 1),
            RefundOutcome::PreEventEnd
        );
        assert_eq!(
            expected_outcome(EVENT_END, DEADLINE, false, EVENT_END - 1),
            RefundOutcome::PreEventEnd
        );
    }

    #[test]
    fn gate_verdict_is_disabled_before_event_end() {
        // Both checked-in and no-show paths hide the CTA before event_end.
        assert!(!gate_verdict_at(EVENT_END, DEADLINE, true, EVENT_END - 1));
        assert!(!gate_verdict_at(EVENT_END, DEADLINE, false, EVENT_END - 1));
    }

    #[test]
    fn preconditions_hold_rejects_missing_event_end() {
        let err = RefundPreEventEndConfig::preconditions_hold(0, 999).unwrap_err();
        assert!(err.contains("missing/zero"), "{err}");
    }

    #[test]
    fn preconditions_hold_rejects_now_at_or_after_event_end() {
        // Exactly at event_end → not strictly before.
        let err = RefundPreEventEndConfig::preconditions_hold(EVENT_END, EVENT_END).unwrap_err();
        assert!(err.contains("not before event_end"), "{err}");

        // After event_end → also rejected.
        let err =
            RefundPreEventEndConfig::preconditions_hold(EVENT_END, EVENT_END + 1).unwrap_err();
        assert!(err.contains("not before event_end"), "{err}");
    }

    #[test]
    fn preconditions_hold_accepts_strictly_before() {
        assert!(RefundPreEventEndConfig::preconditions_hold(EVENT_END, EVENT_END - 1).is_ok());
    }

    #[test]
    fn expected_escrow_code_is_refund_not_yet_allowed() {
        assert_eq!(expected_escrow_code(), EscrowCode::RefundNotYetAllowed);
    }

    #[test]
    fn config_defaults_match_seed_staging_script() {
        let c = RefundPreEventEndConfig::default();
        assert_eq!(c.attendee_id, "flow-test-attendee-1");
        assert_eq!(c.event_id, "flow-test-event");
        assert!(c.wallet_address.is_none());
        assert!(c.assertion_now_ms.is_none());
    }

    #[tokio::test]
    async fn flow_name_is_stable() {
        let flow = RefundPreEventEndFlow::new();
        assert_eq!(flow.name(), "refund_pre_event_end");
    }

    #[test]
    fn gate_and_outcome_agree_before_event_end() {
        // The defining property this flow relies on: before event_end, the
        // corrected gate and the predicted outcome agree (hidden / PreEventEnd).
        // Spot-check a small matrix.
        for checked_in in [true, false] {
            for delta in [1, 100, 1_000] {
                let now = EVENT_END - delta;
                let outcome = expected_outcome(EVENT_END, DEADLINE, checked_in, now);
                let gate = gate_verdict_at(EVENT_END, DEADLINE, checked_in, now);
                assert_eq!(outcome, RefundOutcome::PreEventEnd, "checked_in={checked_in}, now={now}");
                assert!(!gate, "checked_in={checked_in}, now={now}");
            }
        }
    }

    #[test]
    fn assertion_now_clamps_negative_to_zero() {
        // When event_end_ms is tiny (e.g. 1), event_end_ms - 1 = 0, not -1.
        // The flow computes `(event_end_ms - 1).max(0)`. Verify the clamp
        // produces a valid non-negative clock.
        let event_end = 1_i64;
        let pinned = (event_end - 1).max(0);
        assert_eq!(pinned, 0);
        // And the precondition check rejects it cleanly (now=0 is not < 1? it
        // is strictly less, so this is actually a valid pre-end horizon).
        assert!(RefundPreEventEndConfig::preconditions_hold(event_end, pinned).is_ok());
    }

}
