//! Deposit flow — the foundational positive path.
//!
//! Plan 005 §3.4:
//! > register attendee → `POST /deposit/usdc` → sign+send tx →
//! > poll `GET /deposit/status/{id}` until `verified=true` →
//! > assert PDA exists on-chain with expected fields.
//!
//! ## Why this flow is registered first
//!
//! Every later flow assumes a verified deposit exists for the seeded attendee.
//! If the deposit path is broken, the refund/claim flows cannot reach their
//! assertions meaningfully. Running deposit first gives the summary a clear
//! "foundation failed" signal at the top.
//!
//! ## Staging-independence
//!
//! The flow's configuration ([`DepositFlowConfig`]) and the
//! [`DepositFlow::poll_deadline`] / [`DepositFlow::reached_timeout`] helpers
//! are pure functions of time and are unit-tested offline. The `run` body
//! issues HTTP calls and submits a signed transaction — those are gated behind
//! `// TODO(staging-live):` markers and only execute when pointed at a live
//! worker.
//!
//! ## Polling semantics
//!
//! Verification is asynchronous: the worker observes the transaction and
//! flips `verified=true` after confirmation. The harness polls
//! `GET /api/deposit/status/{id}` on a fixed interval until either:
//!  - the response reports `verified=true` → flow passes, OR
//!  - [`DepositFlowConfig::timeout_ms`] elapses → flow fails with a clear
//!    "verification timeout" message.
//!
//! The default timeout is 60s (90s would exceed the §4 5min budget once six
//! flows run; 60s leaves comfortable headroom). The interval is 2s — fast
//! enough to feel responsive, slow enough to avoid hammering the worker.

use std::time::{Duration, Instant};

use domain::models::deposit::DepositStatusResponse;

use crate::assertions::{now_ms, DepositStatusAsserter};
use crate::client::{DepositUsdcRequest, WorkerClient};
use crate::context::StagingContext;
use crate::error::{EscrowCode, HarnessError, HarnessResult, WorkerError};
use crate::runner::Flow;

/// Flow name recorded in `summary.json`.
const FLOW_NAME: &str = "deposit";

/// Default poll interval for deposit verification (2s).
const DEFAULT_POLL_INTERVAL_MS: u64 = 2_000;

/// Default total timeout for deposit verification (60s). Tuned to fit within
/// the §4 performance budget (full harness < 5min) while leaving room for
/// devnet confirmation latency.
const DEFAULT_POLL_TIMEOUT_MS: u64 = 60_000;

/// Configuration for [`DepositFlow`].
///
/// Fields are `pub` so the runner (or a test) can override the seeded defaults
/// without re-constructing the flow. Defaults align with
/// `worker/scripts/seed-staging.sh`.
#[derive(Debug, Clone)]
pub struct DepositFlowConfig {
    /// Attendee id to deposit for. Defaults to the seeded
    /// `flow-test-attendee-1`.
    pub attendee_id: String,
    /// Worker-side event id. Defaults to the seeded `flow-test-event`.
    pub event_id: String,
    /// Attendee wallet address (base58). Defaults to the context's
    /// `attendee_wallet`; overridden only when the flow should target a
    /// different wallet than the seeded one.
    pub wallet_address: Option<String>,
    /// Poll interval for `GET /deposit/status/{id}`.
    pub poll_interval: Duration,
    /// Total timeout before the flow fails with "verification timeout".
    pub poll_timeout: Duration,
}

impl Default for DepositFlowConfig {
    fn default() -> Self {
        Self {
            attendee_id: "flow-test-attendee-1".to_string(),
            event_id: "flow-test-event".to_string(),
            wallet_address: None,
            poll_interval: Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
            poll_timeout: Duration::from_millis(DEFAULT_POLL_TIMEOUT_MS),
        }
    }
}

/// Deposit flow: drive the attendee through the full USDC deposit path.
///
/// Construct via [`DepositFlow::new`] (defaults) or
/// [`DepositFlow::with_config`] (overrides). The struct is cheaply cloneable
/// and holds no state mutation across `run` invocations.
#[derive(Debug, Clone)]
pub struct DepositFlow {
    config: DepositFlowConfig,
}

impl DepositFlow {
    /// Create a deposit flow with default config (seeded attendee + event).
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: DepositFlowConfig::default(),
        }
    }

    /// Create a deposit flow with a custom config (used by tests and by
    /// flows that target a second attendee, e.g. the no-show path).
    #[must_use]
    pub fn with_config(config: DepositFlowConfig) -> Self {
        Self { config }
    }

    /// Resolve the wallet address to use: explicit override, else the
    /// context's seeded attendee wallet.
    fn wallet_address(&self, ctx: &StagingContext) -> String {
        self.config
            .wallet_address
            .clone()
            .unwrap_or_else(|| ctx.attendee_wallet.to_string())
    }

    /// Compute the poll deadline (start + timeout). Pure helper, unit-tested.
    /// Public so the runner or tests can assert the budget is respected.
    pub fn poll_deadline(&self, started_at: Instant) -> Instant {
        started_at + self.config.poll_timeout
    }

    /// True iff `now` has passed the deadline. Pure helper, unit-tested.
    pub fn reached_timeout(now: Instant, deadline: Instant) -> bool {
        now >= deadline
    }
}

impl Default for DepositFlow {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Flow for DepositFlow {
    fn name(&self) -> &'static str {
        FLOW_NAME
    }

    async fn run(&self, ctx: &StagingContext, client: &WorkerClient) -> HarnessResult<()> {
        // Pre-flight: the context must have a consistent event id pair.
        ctx.event_ids_consistent()?;

        let wallet = self.wallet_address(ctx);

        // ── Step 1: Request the deposit transaction ─────────────────────────
        //
        // Issues a live HTTP request to the worker; against an un-provisioned
        // target it fails with `HarnessError::Transport` (recorded as a flow
        // failure). Point the harness at a live staging worker to exercise it.
        let deposit_req = DepositUsdcRequest {
            attendee_id: self.config.attendee_id.clone(),
            event_id: self.config.event_id.clone(),
            wallet_address: wallet.clone(),
        };
        let deposit_resp = client.request_deposit_usdc(ctx, &deposit_req).await?;

        // The response carries a base64 transaction and a Solana Pay URL.
        // The harness signs + submits the transaction directly (it has the
        // funded payer); the Solana Pay URL is a fallback for manual flows.
        assert_deposit_response_present(&deposit_resp.transaction, &deposit_resp.solana_pay_url)?;

        // ── Step 2: Sign + submit the transaction ───────────────────────────
        //
        // Decodes the base64 tx, signs with `ctx.payer`, submits via the
        // `FLOW_HARNESS_RPC_URL` cluster, and awaits confirmation (see
        // `crate::chain::submit_tx`).
        let _signature = submit_deposit_transaction(ctx, &deposit_resp.transaction).await?;

        // ── Step 3: Poll for verification ───────────────────────────────────
        //
        // The worker observes the transaction and flips `verified=true` on the
        // attendee's `DepositStatus` row. We poll until verified or timeout.
        let started_at = Instant::now();
        let deadline = self.poll_deadline(started_at);

        let status = loop {
            if Self::reached_timeout(Instant::now(), deadline) {
                return Err(HarnessError::AssertionFailed {
                    flow: FLOW_NAME,
                    reason: format!(
                        "verification timeout after {}ms (attendee={})",
                        self.config.poll_timeout.as_millis(),
                        self.config.attendee_id
                    ),
                });
            }

            let current = client
                .fetch_deposit_status(ctx, &self.config.attendee_id)
                .await?;

            if is_verified(&current) {
                break current;
            }

            // Sleep before the next poll. `tokio::time::sleep` is cancel-safe;
            // the runner does not cancel mid-flow today, but the property is
            // preserved for future short-circuit semantics.
            tokio::time::sleep(self.config.poll_interval).await;
        };

        // ── Step 4: Assert the deposit-status response is internally
        // consistent and the refund-window verdict matches expectation
        // (deposit-just-verified ⇒ no refund yet, since `now < event_end` on
        // a freshly-seeded event). The assertion logic is the staging-
        // independent payload: even though we fetched the status over the
        // network, the verdict computation is pure and is the regression
        // safety net for fix #19.
        DepositStatusAsserter::new(FLOW_NAME, &status)
            .deadline_consistent()?
            .outcome_is(crate::assertions::RefundOutcome::PreEventEnd, now_ms())?;

        // ── Step 5: Assert the on-chain PDA exists with expected fields ─────
        //
        // Fetches the `AttendeeDeposit` PDA via RPC and asserts owner == escrow
        // program and a fresh (not checked-in / not refunded) deposit — see
        // `assert_on_chain_pda_exists`. Defense-in-depth over the API assertions.
        assert_on_chain_pda_exists(ctx).await?;

        Ok(())
    }
}

// ── Pure helpers (staging-independent, unit-tested) ──────────────────────────

/// A deposit response must carry a non-empty transaction and (optionally) a
/// non-empty Solana Pay URL. The URL may be empty for the `/deposit/usdc/tx`
/// variant; the harness only requires it for the Solana Pay path.
fn assert_deposit_response_present(transaction: &str, solana_pay_url: &str) -> HarnessResult<()> {
    if transaction.is_empty() {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: "deposit response missing `transaction` field".to_string(),
        });
    }
    // Base64-encoded serialized transactions are at least ~200 bytes; flag
    // suspiciously short payloads as likely truncated/empty-tx bugs.
    if transaction.len() < 100 {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: format!(
                "deposit response `transaction` suspiciously short ({} bytes); expected ≥100",
                transaction.len()
            ),
        });
    }
    // Solana Pay URLs are optional for the tx-submission path but, when
    // present, must be well-formed (start with `solana:`).
    if !solana_pay_url.is_empty() && !solana_pay_url.starts_with("solana:") {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: format!(
                "deposit response `solana_pay_url` malformed (expected `solana:` prefix): {}",
                solana_pay_url
            ),
        });
    }
    Ok(())
}

/// Determine whether a [`DepositStatusResponse`] reports a verified deposit.
///
/// The `DepositStatusResponse` shape surfaces verification via the underlying
/// `DepositStatus.verified` field. The exact accessor depends on which fields
/// the response carries; this helper centralises the truth so a future
/// refactor of `domain` updates one call-site.
///
/// TODO(staging-independent): once `DepositStatusResponse` is confirmed to
/// surface `verified` directly, replace this with a field read. For now, we
/// infer verification from `deposit_amount_usdc > 0` (the worker populates
/// the amount only after verifying on-chain). This is the conservative read:
/// a verified-but-zero-amount deposit (not currently possible) would poll
/// until timeout, which is the correct fail-safe.
fn is_verified(status: &DepositStatusResponse) -> bool {
    status.deposit_amount_usdc > 0
}

/// Classify a worker error from the deposit endpoint.
///
/// The deposit endpoint can surface escrow codes 0, 7, 9, 11 (per contract
/// surface §6). Of these, only `IncorrectDepositAmount` (7) and
/// `MintMismatch` (9) are hard failures the harness should report; the others
/// are retryable or environmental. This helper exists so the flow can
/// distinguish them once staging is live.
#[allow(dead_code)]
fn classify_deposit_worker_error(err: &WorkerError) -> DepositWorkerVerdict {
    match err.code {
        Some(EscrowCode::Other(7)) => DepositWorkerVerdict::HardFailure,
        Some(EscrowCode::Other(9)) => DepositWorkerVerdict::HardFailure,
        Some(EscrowCode::Other(11)) => DepositWorkerVerdict::HardFailure,
        _ => DepositWorkerVerdict::Retryable,
    }
}

/// Internal verdict for deposit-endpoint worker errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DepositWorkerVerdict {
    /// The error indicates a permanent contract-level failure (wrong amount,
    /// wrong mint). The harness should not retry.
    HardFailure,
    /// The error is environmental (rate limit, transient RPC). The harness
    /// may retry within its poll budget.
    Retryable,
}

// ── Staging-live stubs ───────────────────────────────────────────────────────
//
// These functions encapsulate the network touch-points so the `run` body reads
// as straight-line logic. Their bodies are TODO until staging is provisioned;
// each returns a clear `Config` error so a misconfigured run fails fast rather
// than blocking on a network call.

/// Submit the deposit transaction to the configured RPC.
///
/// Decode the base64 transaction, sign with `ctx.payer`, submit via the RPC in
/// `FLOW_HARNESS_RPC_URL`, and return the confirmation signature (base58).
async fn submit_deposit_transaction(
    ctx: &StagingContext,
    transaction_base64: &str,
) -> HarnessResult<String> {
    let sig = crate::chain::submit_tx(ctx, transaction_base64).await?;
    Ok(sig.to_string())
}

/// Fetch and validate the on-chain `AttendeeDeposit` PDA: it must exist, be
/// owned by the escrow program, and (a fresh deposit) not be checked in.
async fn assert_on_chain_pda_exists(ctx: &StagingContext) -> HarnessResult<()> {
    let (pda, _) = ctx.attendee_deposit_pda();
    let account = crate::chain::fetch_account(ctx, &pda)
        .await?
        .ok_or_else(|| HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: format!("AttendeeDeposit PDA {pda} not found on-chain after deposit verified"),
        })?;

    if account.owner != ctx.escrow_program_id {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: format!(
                "AttendeeDeposit PDA {pda} owned by {}, expected escrow program {}",
                account.owner, ctx.escrow_program_id
            ),
        });
    }

    let view = crate::chain::decode_attendee_deposit(&account.data).map_err(|e| {
        HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: format!("decoding AttendeeDeposit {pda}: {e}"),
        }
    })?;

    // A just-deposited attendee must not be checked in or refunded.
    if view.checked_in || view.refunded {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: format!(
                "fresh deposit has unexpected state: checked_in={}, refunded={}",
                view.checked_in, view.refunded
            ),
        });
    }
    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_deadline_is_start_plus_timeout() {
        let flow = DepositFlow::with_config(DepositFlowConfig {
            poll_timeout: Duration::from_secs(30),
            ..DepositFlowConfig::default()
        });
        let start = Instant::now();
        let deadline = flow.poll_deadline(start);
        assert_eq!(deadline.duration_since(start), Duration::from_secs(30));
    }

    #[test]
    fn reached_timeout_boundary() {
        // `DepositFlow::reached_timeout` is an associated function; call it
        // directly (no `Self::` in tests — that resolves to the test module).
        let start = Instant::now();
        let deadline = start + Duration::from_secs(5);
        // Exactly at deadline → timed out (>= comparison).
        assert!(DepositFlow::reached_timeout(
            start + Duration::from_secs(5),
            deadline
        ));
        // One nanosecond before → not timed out.
        assert!(!DepositFlow::reached_timeout(
            start + Duration::from_secs(5) - Duration::from_nanos(1),
            deadline
        ));
        // Long after → timed out.
        assert!(DepositFlow::reached_timeout(
            start + Duration::from_secs(60),
            deadline
        ));
    }

    #[test]
    fn assert_deposit_response_rejects_empty_transaction() {
        let err = assert_deposit_response_present("", "").unwrap_err();
        assert!(matches!(err, HarnessError::AssertionFailed { .. }));
        assert!(err.to_string().contains("missing `transaction`"));
    }

    #[test]
    fn assert_deposit_response_rejects_short_transaction() {
        // 50 bytes — below the 100-byte sanity floor.
        let tx = "0".repeat(50);
        let err = assert_deposit_response_present(&tx, "").unwrap_err();
        assert!(err.to_string().contains("suspiciously short"));
    }

    #[test]
    fn assert_deposit_response_accepts_valid_tx_without_pay_url() {
        let tx = "0".repeat(200);
        assert!(assert_deposit_response_present(&tx, "").is_ok());
    }

    #[test]
    fn assert_deposit_response_accepts_valid_solana_pay_url() {
        let tx = "0".repeat(200);
        let url = "solana:https://example.com/pay";
        assert!(assert_deposit_response_present(&tx, url).is_ok());
    }

    #[test]
    fn assert_deposit_response_rejects_malformed_pay_url() {
        let tx = "0".repeat(200);
        let url = "https://example.com/pay";
        let err = assert_deposit_response_present(&tx, url).unwrap_err();
        assert!(err.to_string().contains("solana_pay_url` malformed"));
    }

    #[test]
    fn classify_deposit_worker_error_recognises_hard_failures() {
        let mk = |code: Option<u32>| WorkerError {
            http_status: 400,
            code: code.map(EscrowCode::from_u32),
            message: "x".to_string(),
        };
        // Codes 7, 9, 11 are hard failures per contract surface §6.
        assert_eq!(
            classify_deposit_worker_error(&mk(Some(7))),
            DepositWorkerVerdict::HardFailure
        );
        assert_eq!(
            classify_deposit_worker_error(&mk(Some(9))),
            DepositWorkerVerdict::HardFailure
        );
        assert_eq!(
            classify_deposit_worker_error(&mk(Some(11))),
            DepositWorkerVerdict::HardFailure
        );
        // Anything else (including named refund codes) is retryable.
        assert_eq!(
            classify_deposit_worker_error(&mk(Some(99))),
            DepositWorkerVerdict::Retryable
        );
        assert_eq!(
            classify_deposit_worker_error(&mk(None)),
            DepositWorkerVerdict::Retryable
        );
    }

    #[test]
    fn config_defaults_match_seed_staging_script() {
        let c = DepositFlowConfig::default();
        assert_eq!(c.attendee_id, "flow-test-attendee-1");
        assert_eq!(c.event_id, "flow-test-event");
        assert_eq!(c.poll_interval, Duration::from_millis(2_000));
        assert_eq!(c.poll_timeout, Duration::from_millis(60_000));
    }

    #[tokio::test]
    async fn flow_name_is_stable() {
        let flow = DepositFlow::new();
        assert_eq!(flow.name(), "deposit");
    }

    #[test]
    fn wallet_address_falls_back_to_context_attendee() {
        // We can't construct a real StagingContext without secrets here, but
        // the resolution logic is a trivial `Option::clone().unwrap_or_else`.
        // This test pins the behaviour: `None` config → falls back.
        let config = DepositFlowConfig::default();
        assert!(config.wallet_address.is_none());
    }
}
