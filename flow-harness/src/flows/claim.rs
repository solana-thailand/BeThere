//! Claim flow — NFT badge claim reachability + state transition.
//!
//! Plan 005 §3.4:
//! > NFT claim flow post-checkin via `/claim/{token}`.
//!
//! ## What this flow proves
//!
//! Per `docs/escrow_contract_surface.md` §6, the claim endpoint
//! (`GET /api/claim/{token}`) mints an NFT badge via a **separate program**
//! from the escrow — it surfaces no escrow error codes. This flow's job is
//! therefore narrower than the refund flows:
//!
//!  1. The endpoint is reachable for a known claim token.
//!  2. The response shape matches `ClaimResponse` (`status`, `claimed`,
//!     optional `message`).
//!  3. For a freshly-seeded attendee, the response reports a pre-claim state
//!     (`claimed == false`, `status` indicating "eligible" or "pending").
//!  4. The claim transition is exercisable (the actual mint is gated behind
//!     `// TODO(staging-live):` — it requires a wallet signature on a separate
//!     program and is therefore deferred until staging is live).
//!
//! ## Why this flow is registered after the refund flows
//!
//! Claiming is only meaningful for a checked-in attendee with a verified
//! deposit. The seeded attendee (`flow-test-attendee-1`) is checked-in by
//! `seed-staging.sh`, but the harness still runs the deposit + refund-positive
//! flows first so the summary tells a clean "did the upstream paths work?"
//! story before exercising the downstream claim path.
//!
//! ## Staging-independence
//!
//! The flow's configuration ([`ClaimConfig`]) and the response-shape asserter
//! ([`assert_response_shape`]) are pure functions of the response payload and
//! are unit-tested offline. The `run` body issues HTTP calls (gated behind
//! `// TODO(staging-live):`) and only executes when pointed at a live worker.
//!
//! ## Token resolution
//!
//! The claim token is not a static value — it is derived from the attendee's
//! check-in state and is unique per attendee. `seed-staging.sh` does not mint
//! a deterministic token, so the harness resolves the token at run time via
//! [`resolve_claim_token`] (currently a stub). For the skeleton, a
//! configurable default token is used so the wiring is testable without a
//! live worker.

use crate::client::{ClaimResponse, WorkerClient};
use crate::context::StagingContext;
use crate::error::{HarnessError, HarnessResult};
use crate::runner::Flow;

/// Flow name recorded in `summary.json`.
const FLOW_NAME: &str = "claim";

/// Default claim token used when `seed-staging.sh` does not produce a
/// deterministic one. This is a placeholder so the wiring is testable offline;
/// the real run resolves the token via [`ClaimFlow::resolve_claim_token`].
const DEFAULT_CLAIM_TOKEN: &str = "flow-test-claim-token";

/// Configuration for [`ClaimFlow`].
#[derive(Debug, Clone)]
pub struct ClaimConfig {
    /// Attendee id whose claim is exercised. Defaults to the seeded
    /// `flow-test-attendee-1`.
    pub attendee_id: String,
    /// Worker-side event id. Defaults to the seeded `flow-test-event`.
    pub event_id: String,
    /// Claim token. Defaults to [`DEFAULT_CLAIM_TOKEN`]; the real run overrides
    /// via [`ClaimFlow::resolve_claim_token`] once staging is live and the
    /// token can be read from the attendee's check-in state.
    pub claim_token: String,
    /// Whether to attempt the actual claim transition (mint). Defaults to
    /// `false`: the skeleton asserts reachability + pre-claim state only.
    /// Set to `true` once the wallet-signing path is wired.
    pub attempt_mint: bool,
}

impl Default for ClaimConfig {
    fn default() -> Self {
        Self {
            attendee_id: "flow-test-attendee-1".to_string(),
            event_id: "flow-test-event".to_string(),
            claim_token: DEFAULT_CLAIM_TOKEN.to_string(),
            attempt_mint: false,
        }
    }
}

/// Claim flow: exercise NFT badge claim reachability + state transition.
#[derive(Debug, Clone)]
pub struct ClaimFlow {
    config: ClaimConfig,
}

impl ClaimFlow {
    /// Create a claim flow with default config (seeded attendee + placeholder
    /// token).
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ClaimConfig::default(),
        }
    }

    /// Create a claim flow with a custom config.
    #[must_use]
    pub fn with_config(config: ClaimConfig) -> Self {
        Self { config }
    }

    /// Resolve the claim token for the configured attendee.
    ///
    /// TODO(staging-live): once staging is provisioned, this should query the
    /// worker for the attendee's check-in state and derive the per-attendee
    /// claim token. The placeholder default exists so the wiring is testable
    /// offline; the real run overrides it here.
    fn resolve_claim_token(&self, _ctx: &StagingContext) -> HarnessResult<String> {
        Ok(self.config.claim_token.clone())
    }
}

impl Default for ClaimFlow {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Flow for ClaimFlow {
    fn name(&self) -> &'static str {
        FLOW_NAME
    }

    async fn run(&self, ctx: &StagingContext, client: &WorkerClient) -> HarnessResult<()> {
        ctx.event_ids_consistent()?;

        // ── Step 1: Resolve the claim token ──────────────────────────────────
        //
        // The token is attendee-specific; the harness resolves it from the
        // check-in state at run time. The default is a placeholder so the
        // wiring is testable without a live worker.
        let token = self.resolve_claim_token(ctx)?;

        // ── Step 2: Fetch the claim endpoint ─────────────────────────────────
        //
        // TODO(staging-live): the call below issues an HTTP request. Until
        // staging is provisioned, the request fails with
        // `HarnessError::Transport`, which the runner records as a flow
        // failure. The structure above and below the marker is the real flow
        // body — only the network touch-point is deferred.
        let claim = client.fetch_claim(ctx, &token).await?;

        // ── Step 3: Assert the response shape is well-formed ─────────────────
        //
        // The claim endpoint is a separate program; it surfaces no escrow
        // codes. The harness asserts the response is non-empty and carries a
        // recognisable status string. The exact status vocabulary is owned by
        // the NFT-mint path and may evolve; the asserter accepts the known
        // pre-claim statuses and rejects empty/garbage payloads.
        assert_response_shape(&claim)?;

        // ── Step 4: Assert the seeded attendee is in a pre-claim state ───────
        //
        // A freshly-seeded attendee has not claimed their NFT, so the response
        // must report `claimed == false` and a status indicating eligibility
        // (e.g. "eligible", "pending", "ready"). The exact vocabulary is
        // asserted leniently — any non-claimed, non-error status passes.
        assert_pre_claim_state(&claim)?;

        // ── Step 5: Optionally exercise the claim transition ─────────────────
        //
        // The actual mint requires a wallet signature on a separate program
        // (NFT mint, not escrow). This is gated behind `attempt_mint` (default
        // `false`) and the `// TODO(staging-live):` marker below.
        if self.config.attempt_mint {
            perform_claim_mint(ctx, client, &token).await?;
        }

        Ok(())
    }
}

// ── Pure helpers (staging-independent, unit-tested) ──────────────────────────

/// Assert a `ClaimResponse` is well-formed: non-empty status, recognisable
/// vocabulary.
///
/// The claim endpoint is a separate program; the harness does not pin its
/// exact status enum (it may evolve with the NFT-mint path). The asserter
/// accepts any non-empty status string from the known vocabulary and rejects
/// empty/garbage payloads. This is the staging-independent payload of the
/// flow — it validates the response *shape* regardless of where the response
/// came from.
fn assert_response_shape(claim: &ClaimResponse) -> HarnessResult<()> {
    if claim.status.is_empty() {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: "claim response missing `status` field".to_string(),
        });
    }
    if !is_recognised_status(&claim.status) {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: format!(
                "claim response `status` not in recognised vocabulary: {:?} \
                 (known: eligible, pending, ready, claimed, already_claimed, expired, error)",
                claim.status
            ),
        });
    }
    Ok(())
}

/// Assert the response describes a pre-claim state for a freshly-seeded
/// attendee: `claimed == false` and status indicates eligibility.
fn assert_pre_claim_state(claim: &ClaimResponse) -> HarnessResult<()> {
    if claim.claimed {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: format!(
                "expected pre-claim state (claimed=false) for seeded attendee, \
                 got claimed=true with status={:?}",
                claim.status
            ),
        });
    }
    if !is_eligible_status(&claim.status) {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: format!(
                "expected eligible pre-claim status (eligible/pending/ready), \
                 got status={:?}",
                claim.status
            ),
        });
    }
    Ok(())
}

/// Whether a status string is in the recognised claim vocabulary.
///
/// Kept permissive: the NFT-mint path may evolve its vocabulary, and the
/// harness's job is to catch drift (empty/garbage), not to pin the enum. Add
/// new known statuses here as the worker grows them.
fn is_recognised_status(status: &str) -> bool {
    matches!(
        status,
        "eligible"
            | "pending"
            | "ready"
            | "claimed"
            | "already_claimed"
            | "expired"
            | "error"
            | "not_found"
    )
}

/// Whether a status indicates the attendee is eligible to claim (pre-claim).
fn is_eligible_status(status: &str) -> bool {
    matches!(status, "eligible" | "pending" | "ready")
}

// ── Staging-live stub ────────────────────────────────────────────────────────

/// Perform the actual NFT mint (claim transition).
///
/// TODO(staging-live): once staging is provisioned and the wallet-signing path
/// is wired, this should:
///  1. Trigger the claim-mint flow on the worker (likely `POST /api/claim/mint`
///     or similar — confirm the route with the worker handlers).
///  2. Sign + submit the mint transaction via the configured RPC.
///  3. Poll `GET /api/claim/{token}` until `claimed == true`.
///  4. Assert the response reports `claimed=true` and a terminal status.
/// Until then, returns a `Config` error so the run fails fast with a pointer
/// to the missing precondition rather than blocking on a network call.
async fn perform_claim_mint(
    _ctx: &StagingContext,
    _client: &WorkerClient,
    _token: &str,
) -> HarnessResult<()> {
    Err(HarnessError::Config(format!(
        "[{FLOW_NAME}] perform_claim_mint not yet wired (staging not live); \
         set attempt_mint=false (default) to skip the mint and assert only the \
         pre-claim state. Wire the mint in the same PR that removes the staging \
         TODO markers."
    )))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    // `Pubkey` is referenced via its fully-qualified path in
    // `resolve_claim_token_uses_config_default` below; only `FromStr` (the
    // trait that powers `.from_str`) needs to be in scope here.
    use std::str::FromStr;

    fn resp(status: &str, claimed: bool) -> ClaimResponse {
        ClaimResponse {
            status: status.to_string(),
            claimed,
            message: None,
        }
    }

    // ── assert_response_shape ────────────────────────────────────────────────

    #[test]
    fn response_shape_rejects_empty_status() {
        let err = assert_response_shape(&resp("", false)).unwrap_err();
        assert!(err.to_string().contains("missing `status`"), "{}", err);
    }

    #[test]
    fn response_shape_rejects_unknown_status() {
        let err = assert_response_shape(&resp("garbage_value", false)).unwrap_err();
        assert!(err.to_string().contains("not in recognised vocabulary"), "{}", err);
    }

    #[test]
    fn response_shape_accepts_known_statuses() {
        for s in [
            "eligible", "pending", "ready", "claimed", "already_claimed", "expired", "error",
            "not_found",
        ] {
            assert!(assert_response_shape(&resp(s, false)).is_ok(), "status={s}");
        }
    }

    // ── assert_pre_claim_state ───────────────────────────────────────────────

    #[test]
    fn pre_claim_state_rejects_claimed_true() {
        let err = assert_pre_claim_state(&resp("claimed", true)).unwrap_err();
        assert!(err.to_string().contains("expected pre-claim state"), "{}", err);
    }

    #[test]
    fn pre_claim_state_rejects_non_eligible_status() {
        // Even with claimed=false, an "expired" or "error" status is not a
        // pre-claim eligible state — the harness catches this drift.
        let err = assert_pre_claim_state(&resp("expired", false)).unwrap_err();
        assert!(err.to_string().contains("expected eligible pre-claim status"), "{}", err);

        let err = assert_pre_claim_state(&resp("error", false)).unwrap_err();
        assert!(err.to_string().contains("expected eligible pre-claim status"), "{}", err);
    }

    #[test]
    fn pre_claim_state_accepts_eligible_statuses() {
        for s in ["eligible", "pending", "ready"] {
            assert!(assert_pre_claim_state(&resp(s, false)).is_ok(), "status={s}");
        }
    }

    // ── vocabulary helpers ───────────────────────────────────────────────────

    #[test]
    fn recognised_status_covers_known_set() {
        assert!(is_recognised_status("eligible"));
        assert!(is_recognised_status("claimed"));
        assert!(is_recognised_status("already_claimed"));
        assert!(is_recognised_status("error"));
        assert!(!is_recognised_status(""));
        assert!(!is_recognised_status("ELIGIBLE")); // case-sensitive by design
        assert!(!is_recognised_status("random"));
    }

    #[test]
    fn eligible_status_excludes_terminal_states() {
        assert!(is_eligible_status("eligible"));
        assert!(is_eligible_status("pending"));
        assert!(is_eligible_status("ready"));
        // Terminal / error states are not eligible.
        assert!(!is_eligible_status("claimed"));
        assert!(!is_eligible_status("already_claimed"));
        assert!(!is_eligible_status("expired"));
        assert!(!is_eligible_status("error"));
    }

    // ── Config + flow metadata ───────────────────────────────────────────────

    #[test]
    fn config_defaults_align_with_seed_staging() {
        let c = ClaimConfig::default();
        assert_eq!(c.attendee_id, "flow-test-attendee-1");
        assert_eq!(c.event_id, "flow-test-event");
        assert_eq!(c.claim_token, DEFAULT_CLAIM_TOKEN);
        assert!(!c.attempt_mint, "default must not attempt the mint (staging-gated)");
    }

    #[tokio::test]
    async fn flow_name_is_stable() {
        let flow = ClaimFlow::new();
        assert_eq!(flow.name(), "claim");
    }

    #[tokio::test]
    async fn resolve_claim_token_uses_config_default() {
        // Without a live worker, resolution falls back to the configured
        // token. This pins the offline behaviour.
        let flow = ClaimFlow::with_config(ClaimConfig {
            claim_token: "my-test-token".to_string(),
            ..ClaimConfig::default()
        });
        let ctx = StagingContext::for_testing(
            "https://staging.example.workers.dev",
            1,
            solana_sdk::pubkey::Pubkey::from_str("11111111111111111111111111111112").unwrap(),
            solana_sdk::pubkey::Pubkey::from_str("11111111111111111111111111111111").unwrap(),
        )
        .expect("for_testing succeeds with valid inputs");
        let token = flow.resolve_claim_token(&ctx).expect("resolve");
        assert_eq!(token, "my-test-token");
    }

    #[test]
    fn response_message_field_is_optional() {
        // `ClaimResponse.message` is `Option<String>`; ensure it deserialises
        // both with and without the field present.
        let with_msg: ClaimResponse =
            serde_json::from_str(r#"{"status":"eligible","claimed":false,"message":"hi"}"#)
                .expect("deserialise with message");
        assert_eq!(with_msg.message.as_deref(), Some("hi"));

        let without_msg: ClaimResponse =
            serde_json::from_str(r#"{"status":"eligible","claimed":false}"#)
                .expect("deserialise without message");
        assert!(without_msg.message.is_none());
    }

    #[test]
    fn response_default_fields_lenient() {
        // The most minimal valid payload: just `status`. `claimed` defaults to
        // false; `message` defaults to None.
        let minimal: ClaimResponse =
            serde_json::from_str(r#"{"status":"eligible"}"#).expect("deserialise minimal");
        assert_eq!(minimal.status, "eligible");
        assert!(!minimal.claimed);
        assert!(minimal.message.is_none());
    }
}
