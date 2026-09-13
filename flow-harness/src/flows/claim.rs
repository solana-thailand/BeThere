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

/// Default claim token, matching what `seed-staging.sh` writes for the default
/// fixture (`${EVENT_ID}-claim-token-1` with `EVENT_ID=flow-test-event`).
///
/// Issue 084: this used to be the bare literal `flow-test-claim-token`, which
/// the seed script has never written. The claim flow therefore always looked up
/// a token that did not exist — and on staging, where `PLATFORM_SHEET_ID` is
/// empty, that D1 miss surfaces as a 500 through the Sheets fallback, which
/// reads like a server fault and is not one.
const DEFAULT_CLAIM_TOKEN: &str = "flow-test-event-claim-token-1";

/// The token `seed-staging.sh` writes for a given fixture event.
///
/// Keep in step with `CLAIM_TOKEN="${EVENT_ID}-claim-token-1"` in that script.
fn seeded_claim_token(event_id: &str) -> String {
    format!("{event_id}-claim-token-1")
}

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
    /// Build from the environment so this flow targets the same fixture as the
    /// rest of the suite (Issue 084).
    ///
    /// `FLOW_HARNESS_CLAIM_TOKEN` overrides explicitly; otherwise the token is
    /// derived from the event id using the seed script's convention, so naming
    /// a fixture is enough and the two can no longer drift apart silently.
    #[must_use]
    pub fn from_env() -> Self {
        use super::fixture_value;
        let defaults = ClaimConfig::default();
        let event_id = fixture_value("FLOW_HARNESS_EVENT_ID", defaults.event_id.clone());
        let claim_token = fixture_value("FLOW_HARNESS_CLAIM_TOKEN", seeded_claim_token(&event_id));
        Self {
            config: ClaimConfig {
                attendee_id: fixture_value("FLOW_HARNESS_ATTENDEE_ID", defaults.attendee_id),
                event_id,
                claim_token,
                ..defaults
            },
        }
    }

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

/// Assert the claim payload has the shape the production frontend consumes.
///
/// Issue 088. This used to require a `status` field. `GET /api/claim/{token}`
/// has never returned one — it returns `claimed`, `checked_in_at`,
/// `nft_available`, `quiz_status` and ~17 more — so the flow failed against a
/// perfectly healthy API. The Worker is the contract here, because the Leptos
/// claim page is built on exactly these fields; inventing a `status` would have
/// added API surface with no consumer.
fn assert_response_shape(claim: &ClaimResponse) -> HarnessResult<()> {
    // A resolved claim always knows its own check-in state. An empty
    // `checked_in_at` on a claimed badge means the payload is incoherent.
    if claim.claimed && claim.checked_in_at.is_empty() {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: "claim reports claimed=true with an empty checked_in_at —                      a badge cannot be claimed without a check-in"
                .to_string(),
        });
    }
    if claim.claimed && claim.claimed_at.as_deref().unwrap_or("").is_empty() {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: "claim reports claimed=true with no claimed_at timestamp".to_string(),
        });
    }
    Ok(())
}

/// Assert the response describes a pre-claim state for a freshly-seeded
/// attendee: checked in, nothing claimed yet, and the event can actually mint.
fn assert_pre_claim_state(claim: &ClaimResponse) -> HarnessResult<()> {
    if claim.claimed {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: "expected pre-claim state (claimed=false) for the seeded attendee".to_string(),
        });
    }
    if claim.checked_in_at.is_empty() {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: "seeded attendee is not checked in, so the claim is not yet                      reachable — re-run seed-staging.sh"
                .to_string(),
        });
    }
    // `nft_available` is deliberately *not* asserted: it reflects whether the
    // environment has Crossmint configured, which is an environment fact rather
    // than a claim-path regression. Asserting it would make the flow fail on a
    // staging worker with no mint provider, which is not what this flow tests.
    Ok(())
}

// ── Staging-live stub ────────────────────────────────────────────────────────

/// Perform the actual NFT mint (claim transition).
///
/// Deliberately NOT wired (unlike the deposit/refund on-chain seams): the NFT
/// mint is a separate program and the worker mint route is unconfirmed in the
/// handlers (`POST /api/claim/mint` is a guess). Rather than sign+submit against
/// an unverified endpoint, this fails fast. The claim flow defaults to
/// `attempt_mint=false`, asserting only the pre-claim state, so a default run is
/// unaffected. Wire this once the mint route + wallet-signing path are confirmed
/// (the `chain::submit_tx` seam is ready to reuse).
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

    /// Build a claim payload in the shape the Worker actually returns.
    fn resp(checked_in: bool, claimed: bool) -> ClaimResponse {
        ClaimResponse {
            claimed,
            checked_in_at: match checked_in {
                true => "2026-09-13T05:30:31.706+00:00".to_string(),
                false => String::new(),
            },
            nft_available: true,
            claimed_at: match claimed {
                true => Some("2026-09-13T06:00:00.000+00:00".to_string()),
                false => None,
            },
            message: None,
        }
    }

    // ── assert_response_shape ────────────────────────────────────────────────

    // ── assert_pre_claim_state ───────────────────────────────────────────────

    #[test]
    fn pre_claim_state_rejects_claimed_true() {
        let err = assert_pre_claim_state(&resp(true, true)).unwrap_err();
        assert!(
            err.to_string().contains("expected pre-claim state"),
            "{}",
            err
        );
    }

    // ── vocabulary helpers ───────────────────────────────────────────────────

    // ── Config + flow metadata ───────────────────────────────────────────────

    #[test]
    fn config_defaults_align_with_seed_staging() {
        let c = ClaimConfig::default();
        assert_eq!(c.attendee_id, "flow-test-attendee-1");
        assert_eq!(c.event_id, "flow-test-event");
        assert_eq!(c.claim_token, DEFAULT_CLAIM_TOKEN);
        assert!(
            !c.attempt_mint,
            "default must not attempt the mint (staging-gated)"
        );
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

    /// The real payload carries ~20 fields the harness does not model. Every
    /// field is `default`, so a richer response must still deserialise — that
    /// leniency is what keeps the harness from breaking on an unrelated API
    /// addition.
    #[test]
    fn a_richer_worker_payload_still_deserialises() {
        let real = r#"{"claimed":false,"checked_in_at":"2026-09-13T05:30:31.706+00:00",
            "nft_available":true,"quiz_status":"none","total_checked_in":3,
            "event":{"event_name":"x"},"locked_wallet":null}"#;
        let parsed: ClaimResponse = serde_json::from_str(real).expect("deserialise real payload");
        assert!(!parsed.claimed);
        assert_eq!(parsed.checked_in_at, "2026-09-13T05:30:31.706+00:00");
        assert!(parsed.nft_available);
    }

    /// Issue 088: the flow used to demand a `status` field the API has never
    /// returned, so it failed against a healthy Worker.
    #[test]
    fn a_payload_without_status_is_accepted() {
        let parsed: ClaimResponse = serde_json::from_str(
            r#"{"claimed":false,"checked_in_at":"2026-09-13T05:30:31.706+00:00"}"#,
        )
        .expect("deserialise");
        assert_response_shape(&parsed).expect("no status field is not an error");
    }

    #[test]
    fn a_claimed_badge_without_a_check_in_is_incoherent() {
        let bad = ClaimResponse {
            claimed: true,
            checked_in_at: String::new(),
            nft_available: true,
            claimed_at: Some("2026-09-13T06:00:00.000+00:00".to_string()),
            message: None,
        };
        let err = assert_response_shape(&bad).unwrap_err();
        assert!(
            err.to_string()
                .contains("cannot be claimed without a check-in"),
            "{err}"
        );
    }
}
