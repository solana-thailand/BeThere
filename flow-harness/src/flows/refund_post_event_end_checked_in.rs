//! Refund-after-event-end (checked-in) flow — the canonical positive path.
//!
//! Plan 005 §3.4:
//! > check-in attendee → advance clock past `event_end` → refund succeeds.
//!
//! ## What this flow proves
//!
//! The on-chain guard at `bethere-escrow/src/instructions/refund.rs#L72-85`
//! allows a refund for a checked-in attendee at any time after `event_end` —
//! there is **no upper bound** for the checked-in path. This flow confirms:
//!
//!  1. The seeded attendee is genuinely `checked_in == true` on the worker.
//!  2. The seeded horizon places the run clock past `event_end`.
//!  3. The worker builds a `refund + close_deposit` TX that the program
//!     accepts (not the `RefundNotYetAllowed` revert the negative flow asserts).
//!  4. The corrected frontend gate would render the CTA enabled.
//!
//! ## Why this is the *positive* counterpart to `refund_pre_event_end`
//!
//! Both flows use the same seeded attendee (`flow-test-attendee-1`, checked-in
//! by `seed-staging.sh`). The difference is *when* the refund is attempted
//! relative to `event_end`. This flow attempts it post-end and expects
//! success; `refund_pre_event_end` pins the verdict clock to `event_end - 1`
//! and expects `RefundNotYetAllowed`. Together they bracket the first on-chain
//! guard.
//!
//! ## Staging-independence
//!
//! The preconditions and the verdict computation are pure:
//!  - [`RefundPostEventEndCheckedInConfig::preconditions_hold`] verifies the
//!    seeded horizon places `now` at-or-after `event_end` AND that the
//!    attendee is checked in.
//!  - [`expected_outcome`] returns [`RefundOutcome::Allowed`] for any horizon
//!    satisfying the preconditions (the checked-in path has no deadline bound).
//!  - [`gate_verdict_at`] computes what the corrected gate renders.
//!
//! The `run` body issues HTTP calls (gated behind `// TODO(staging-live):`)
//! and only executes when pointed at a live worker.
//!
//! ## Note on the gate-divergence detector
//!
//! On the checked-in path past `event_end`, the corrected gate
//! (`refund_cta_enabled`) and the legacy event_end-only gate **agree** — both
//! render the CTA enabled. So this flow does not catch the #19 divergence;
//! that is the job of `refund_no_show_deadline`. This flow's role is to
//! confirm the on-chain positive path and the `checked_in` data plumbing.

use crate::assertions::{
    predict_refund_outcome, refund_cta_enabled, DepositStatusAsserter, RefundOutcome,
};
use crate::client::{RefundRequest, WorkerClient};
use crate::context::StagingContext;
use crate::error::{HarnessError, HarnessResult};
use crate::runner::Flow;

/// Flow name recorded in `summary.json`.
const FLOW_NAME: &str = "refund_post_event_end_checked_in";

/// Configuration for [`RefundPostEventEndCheckedInFlow`].
#[derive(Debug, Clone)]
pub struct RefundPostEventEndCheckedInConfig {
    /// Attendee id whose refund is attempted. Defaults to the seeded
    /// `flow-test-attendee-1` (already `checked_in` per `seed-staging.sh`).
    pub attendee_id: String,
    /// Worker-side event id. Defaults to the seeded `flow-test-event`.
    pub event_id: String,
    /// Attendee wallet address (base58). Defaults to the context's
    /// `attendee_wallet`.
    pub wallet_address: Option<String>,
    /// The clock value at which to evaluate the gate verdict. Defaults to the
    /// wall clock at run time. Override only when pinning the verdict for a
    /// deterministic offline test.
    pub assertion_now_ms: Option<i64>,
}

impl Default for RefundPostEventEndCheckedInConfig {
    fn default() -> Self {
        Self {
            attendee_id: "flow-test-attendee-1".to_string(),
            event_id: "flow-test-event".to_string(),
            wallet_address: None,
            assertion_now_ms: None,
        }
    }
}

/// Refund-after-event-end (checked-in) positive flow.
#[derive(Debug, Clone)]
pub struct RefundPostEventEndCheckedInFlow {
    config: RefundPostEventEndCheckedInConfig,
}

impl RefundPostEventEndCheckedInFlow {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RefundPostEventEndCheckedInConfig::default(),
        }
    }

    #[must_use]
    pub fn with_config(config: RefundPostEventEndCheckedInConfig) -> Self {
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

impl Default for RefundPostEventEndCheckedInFlow {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Flow for RefundPostEventEndCheckedInFlow {
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

        // ── Step 2: Resolve the verdict clock ────────────────────────────────
        //
        // The seeded event's `event_end` is `now - 2h` at seed time
        // (`seed-staging.sh`), so the wall clock already places us past
        // `event_end`. We default to the run-time wall clock; tests pin it
        // explicitly.
        let assertion_now = self.config.assertion_now_ms.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0)
        });

        // ── Step 3: Assert the response's refund-window fields are
        // internally consistent (the worker's denormalised
        // `refund_deadline_ms` must equal the hours-derived value).
        DepositStatusAsserter::new(FLOW_NAME, &status).deadline_consistent()?;

        // ── Step 4: Assert the preconditions hold ────────────────────────────
        //
        // This flow requires BOTH (a) the attendee is checked in and (b) the
        // clock is at-or-after `event_end`. A failure here is a seed/config
        // bug, not a worker bug — the message says so.
        if let Err(reason) = RefundPostEventEndCheckedInConfig::preconditions_hold(
            status.event_end_ms,
            status.checked_in,
            assertion_now,
        ) {
            return Err(HarnessError::AssertionFailed {
                flow: FLOW_NAME,
                reason,
            });
        }

        // ── Step 5: Assert the predicted on-chain outcome is Allowed ─────────
        //
        // The defining property of the checked-in path: past `event_end`, the
        // outcome is `Allowed` regardless of `refund_deadline_ms`. This is the
        // regression safety net — if the program's guard or the worker's
        // response ever starts returning a non-Allowed outcome here, the flow
        // fails loudly.
        let expected = expected_outcome(
            status.event_end_ms,
            status.refund_deadline_ms,
            status.checked_in,
            assertion_now,
        );
        DepositStatusAsserter::new(FLOW_NAME, &status).outcome_is(expected, assertion_now)?;

        // ── Step 6: Assert the corrected gate renders the CTA enabled ────────
        //
        // On the checked-in path past `event_end`, the corrected gate enables
        // the CTA. We assert this explicitly; the legacy event_end-only gate
        // agrees here, so this branch does not catch #19 (that is the no-show
        // flow's job), but it pins the positive-gate behaviour for future
        // regressions.
        let cta_enabled = gate_verdict_at(
            status.event_end_ms,
            status.refund_deadline_ms,
            status.checked_in,
            assertion_now,
        );
        if !cta_enabled {
            return Err(HarnessError::AssertionFailed {
                flow: FLOW_NAME,
                reason: format!(
                    "corrected gate rendered CTA disabled for checked-in attendee past \
                     event_end (event_end_ms={}, assertion_now={}, refund_deadline_ms={}): \
                     expected enabled",
                    status.event_end_ms, assertion_now, status.refund_deadline_ms
                ),
            });
        }

        // ── Step 7: Submit the refund and assert success ─────────────────────
        //
        // TODO(staging-live): the worker returns a paired `refund + close`
        // TX. Once staging is live, this branch should:
        //   1. POST /api/escrow/refund → receive `TxResponse`.
        //   2. Sign + submit the TX via the configured RPC.
        //   3. Poll the attendee's `DepositStatus` until `verified == false`
        //      OR the deposit PDA is closed on-chain.
        //   4. Assert the attendee's deposit PDA no longer carries the
        //      expected deposit amount (the refund drained it).
        // Until then, `submit_refund_and_assert_success` returns a clear
        // Config error so the run fails fast with a pointer to the missing
        // precondition.
        let refund_req = RefundRequest {
            attendee_id: self.config.attendee_id.clone(),
            event_id: self.config.event_id.clone(),
            wallet_address: self.wallet_address(ctx),
        };
        submit_refund_and_assert_success(client, ctx, &refund_req).await?;

        Ok(())
    }
}

// ── Pure helpers (staging-independent, unit-tested) ──────────────────────────

/// The expected on-chain refund outcome for this flow's horizon.
///
/// For any horizon satisfying the preconditions (`checked_in && now >=
/// event_end`), the outcome is [`RefundOutcome::Allowed`] — the checked-in
/// path has no deadline bound.
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

impl RefundPostEventEndCheckedInConfig {
    /// Verify the seeded horizon satisfies this flow's preconditions:
    ///  - `event_end_ms > 0` (horizon is populated)
    ///  - `checked_in == true` (this is the checked-in flow)
    ///  - `now >= event_end` (we are past the end)
    ///
    /// Returns `Err(reason)` if any precondition fails. Pure function of the
    /// horizon; unit-tested offline.
    pub fn preconditions_hold(
        event_end_ms: i64,
        checked_in: bool,
        now_ms: i64,
    ) -> Result<(), String> {
        if event_end_ms <= 0 {
            return Err(format!(
                "event_end_ms={event_end_ms} is missing/zero; the seeded event \
                 must populate event_end_ms for the post-end flow to be meaningful"
            ));
        }
        if !checked_in {
            return Err("attendee is not checked_in; this flow requires checked_in=true. \
                 This is a seed bug (seed-staging.sh marks flow-test-attendee-1 \
                 as checked-in) — re-run worker/scripts/seed-staging.sh, or use \
                 the refund_no_show_deadline flow for a non-checked-in attendee.".to_string());
        }
        if now_ms < event_end_ms {
            return Err(format!(
                "assertion_now={now_ms} is before event_end_ms={event_end_ms}; \
                 the post-end flow requires now >= event_end. The seeded event \
                 ends at now-2h at seed time, so this is either a stale seed \
                 (re-run seed-staging.sh) or a pinned assertion_now_ms that is \
                 too early."
            ));
        }
        Ok(())
    }
}

// ── On-chain seam ─────────────────────────────────────────────────────────────

/// Submit the checked-in refund past `event_end` and assert it succeeds: the
/// worker builds the paired refund+close tx, it lands on-chain, and the
/// `AttendeeDeposit` PDA is closed as a result (the defining post-condition).
async fn submit_refund_and_assert_success(
    client: &WorkerClient,
    ctx: &StagingContext,
    req: &RefundRequest,
) -> HarnessResult<()> {
    // Positive path: the worker must build the paired refund+close tx …
    let tx = client.request_refund(ctx, req).await?;
    // … which must land on-chain (any Custom(N) revert surfaces as Worker).
    let _sig = crate::chain::submit_tx(ctx, &tx.transaction).await?;

    // The refund+close pair closes the AttendeeDeposit PDA, so it must no longer
    // exist (or be zeroed) on-chain — the defining post-condition of success.
    let (pda, _) = ctx.attendee_deposit_pda();
    match crate::chain::fetch_account(ctx, &pda).await? {
        None => Ok(()),
        Some(a) if a.data.is_empty() => Ok(()),
        Some(a) => Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: format!(
                "AttendeeDeposit PDA {pda} still present ({} bytes) after refund+close; expected closed",
                a.data.len()
            ),
        }),
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const EVENT_END: i64 = 10_000;
    const DEADLINE: i64 = 20_000;

    #[test]
    fn expected_outcome_is_allowed_for_checked_in_past_event_end() {
        // The defining property: checked-in attendees refund at any time past
        // event_end, including past the no-show deadline.
        assert_eq!(
            expected_outcome(EVENT_END, DEADLINE, true, EVENT_END),
            RefundOutcome::Allowed
        );
        assert_eq!(
            expected_outcome(EVENT_END, DEADLINE, true, DEADLINE),
            RefundOutcome::Allowed
        );
        assert_eq!(
            expected_outcome(EVENT_END, DEADLINE, true, DEADLINE + 1),
            RefundOutcome::Allowed
        );
        assert_eq!(
            expected_outcome(EVENT_END, DEADLINE, true, i64::MAX / 2),
            RefundOutcome::Allowed
        );
    }

    #[test]
    fn gate_verdict_is_enabled_for_checked_in_past_event_end() {
        // Corrected gate enables the CTA for checked-in attendees past end.
        assert!(gate_verdict_at(EVENT_END, DEADLINE, true, EVENT_END));
        assert!(gate_verdict_at(EVENT_END, DEADLINE, true, DEADLINE));
        assert!(gate_verdict_at(EVENT_END, DEADLINE, true, DEADLINE + 1));
    }

    #[test]
    fn preconditions_hold_rejects_missing_event_end() {
        let err =
            RefundPostEventEndCheckedInConfig::preconditions_hold(0, true, 999).unwrap_err();
        assert!(err.contains("missing/zero"), "{err}");
    }

    #[test]
    fn preconditions_hold_rejects_non_checked_in() {
        let err = RefundPostEventEndCheckedInConfig::preconditions_hold(EVENT_END, false, EVENT_END)
            .unwrap_err();
        assert!(err.contains("not checked_in"), "{err}");
        assert!(err.contains("seed-staging.sh"), "{err}");
    }

    #[test]
    fn preconditions_hold_rejects_now_before_event_end() {
        let err =
            RefundPostEventEndCheckedInConfig::preconditions_hold(EVENT_END, true, EVENT_END - 1)
                .unwrap_err();
        assert!(err.contains("before event_end_ms"), "{err}");
    }

    #[test]
    fn preconditions_hold_accepts_valid_horizon() {
        // At event_end (boundary): `now >= event_end` holds.
        assert!(RefundPostEventEndCheckedInConfig::preconditions_hold(EVENT_END, true, EVENT_END)
            .is_ok());
        // After event_end.
        assert!(RefundPostEventEndCheckedInConfig::preconditions_hold(EVENT_END, true, EVENT_END + 1)
            .is_ok());
        // Way past the deadline — still OK for checked-in.
        assert!(RefundPostEventEndCheckedInConfig::preconditions_hold(EVENT_END, true, DEADLINE + 1)
            .is_ok());
    }

    #[test]
    fn config_defaults_match_seed_staging_script() {
        let c = RefundPostEventEndCheckedInConfig::default();
        assert_eq!(c.attendee_id, "flow-test-attendee-1");
        assert_eq!(c.event_id, "flow-test-event");
        assert!(c.wallet_address.is_none());
        assert!(c.assertion_now_ms.is_none());
    }

    #[tokio::test]
    async fn flow_name_is_stable() {
        let flow = RefundPostEventEndCheckedInFlow::new();
        assert_eq!(flow.name(), "refund_post_event_end_checked_in");
    }

    #[test]
    fn gate_and_outcome_agree_for_checked_in_past_event_end() {
        // The defining property this flow relies on: for checked-in attendees
        // past event_end, the corrected gate (enabled) and the predicted
        // outcome (Allowed) agree across the horizon — including past the
        // no-show deadline, which would block a non-checked-in attendee.
        for now in [EVENT_END, DEADLINE - 1, DEADLINE, DEADLINE + 1, i64::MAX / 2] {
            let outcome = expected_outcome(EVENT_END, DEADLINE, true, now);
            let gate = gate_verdict_at(EVENT_END, DEADLINE, true, now);
            assert_eq!(outcome, RefundOutcome::Allowed, "now={now}");
            assert!(gate, "now={now}");
        }
    }

    #[test]
    fn checked_in_path_unaffected_by_missing_deadline() {
        // Even if refund_deadline_ms is missing (0/legacy data), the
        // checked-in path stays Allowed past event_end. This is the property
        // that distinguishes the checked-in path from the no-show path
        // (which would resolve to DeadlinePassed on missing deadline).
        assert_eq!(
            expected_outcome(EVENT_END, 0, true, EVENT_END + 1),
            RefundOutcome::Allowed
        );
        assert!(gate_verdict_at(EVENT_END, 0, true, EVENT_END + 1));
    }
}
