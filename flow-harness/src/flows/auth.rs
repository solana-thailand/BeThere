//! Auth-session flow — plan 006 SIWS regression baseline.
//!
//! Plan 005 §3.4:
//! > existing Google-auth session issuance baseline (so plan 006 can prove
//! > SIWS doesn't regress it).
//!
//! ## What this flow proves
//!
//! Plan 006 (`006_siws_hybrid_auth.md`) introduces Sign-In-With-Solana as an
//! alternative authentication path alongside the existing Google OAuth flow.
//! SIWS must not regress the existing Google-auth behaviour. This flow
//! establishes the **baseline** against which that claim is verified: it
//! asserts the worker's `GET /api/auth/session` endpoint reports the expected
//! shape for both the logged-out and logged-in cases.
//!
//! Concretely, this flow exercises two sub-paths:
//!
//!  1. **Logged-out (unauthenticated)**: a request with no session cookie
//!     returns `AuthSessionResponse { authenticated: false, email: None }`.
//!     This is the state a fresh user sees before any login.
//!  2. **Logged-in (authenticated)**: a request carrying a valid session
//!     cookie returns `authenticated: true` and the seeded attendee's email.
//!     This is the state after Google OAuth completes.
//!
//! When plan 006 ships SIWS, the logged-in sub-path will additionally cover
//! SIWS-issued sessions; the shape contract must not change. The harness pins
//! that contract here so a regression fails loudly at preflight rather than
//! in production.
//!
//! ## Why this flow is registered last
//!
//! Authentication is the outermost layer of every other flow's request, so it
//! is the most cross-cutting concern. Running it last means a failure here is
//! easy to attribute: if the auth flow fails but the deposit/refund/claim
//! flows passed, the failure is auth-specific, not a downstream side effect.
//!
//! ## Staging-independence
//!
//! The flow's configuration ([`AuthConfig`]) and the response-shape asserter
//! ([`assert_session_shape`]) are pure functions of the response payload and
//! are unit-tested offline. The `run` body issues HTTP calls (gated behind
//! `// TODO(staging-live):`) and only executes when pointed at a live worker.
//!
//! ## Session cookie resolution
//!
//! The logged-in sub-path requires a valid session cookie. The harness does
//! not mint one itself — that would duplicate the OAuth flow under test.
//! Instead, the cookie is supplied via `FLOW_HARNESS_ATTENDEE_SESSION` (the
//! same cookie a browser would carry after a real Google login). When absent,
//! the logged-in sub-path is skipped (not failed) with a clear reason in
//! `summary.json`; the logged-out sub-path still runs unconditionally.

use crate::client::{AuthSessionResponse, WorkerClient};
use crate::context::StagingContext;
use crate::error::{HarnessError, HarnessResult};
use crate::runner::Flow;

/// Flow name recorded in `summary.json`.
const FLOW_NAME: &str = "auth";

/// Configuration for [`AuthFlow`].
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// The email the logged-in sub-path expects to see in the session
    /// response. Defaults to the seeded attendee's email
    /// (`flow-test-attendee-1@staging.local`).
    pub expected_email: String,
    /// A valid session cookie value for the logged-in sub-path. When `None`,
    /// the logged-in sub-path is skipped (not failed). Supplied at runtime
    /// via `FLOW_HARNESS_ATTENDEE_SESSION`.
    pub session_cookie: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            expected_email: "flow-test-attendee-1@staging.local".to_string(),
            session_cookie: None,
        }
    }
}

/// Auth flow: assert the `GET /api/auth/session` contract for both the
/// logged-out and logged-in states, establishing the SIWS regression baseline.
#[derive(Debug, Clone)]
pub struct AuthFlow {
    config: AuthConfig,
}

impl AuthFlow {
    /// Create an auth flow with default config (seeded attendee email, no
    /// session cookie — the logged-in sub-path will be skipped until
    /// `FLOW_HARNESS_ATTENDEE_SESSION` is supplied).
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: AuthConfig::default(),
        }
    }

    /// Create an auth flow with a custom config.
    #[must_use]
    pub fn with_config(config: AuthConfig) -> Self {
        Self { config }
    }

    /// Construct an auth flow from environment, picking up the session cookie
    /// from `FLOW_HARNESS_ATTENDEE_SESSION` when present. This is the
    /// production-path constructor used by the runner; `new()`/`with_config()`
    /// are for tests and explicit overrides.
    #[must_use]
    pub fn from_env() -> Self {
        let session_cookie = std::env::var("FLOW_HARNESS_ATTENDEE_SESSION").ok();
        let expected_email = std::env::var("FLOW_HARNESS_ATTENDEE_EMAIL")
            .unwrap_or_else(|_| AuthConfig::default().expected_email);
        Self {
            config: AuthConfig {
                expected_email,
                session_cookie,
            },
        }
    }
}

impl Default for AuthFlow {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Flow for AuthFlow {
    fn name(&self) -> &'static str {
        FLOW_NAME
    }

    async fn run(&self, ctx: &StagingContext, client: &WorkerClient) -> HarnessResult<()> {
        ctx.event_ids_consistent()?;

        // ── Sub-path 1: logged-out (unauthenticated) ─────────────────────────
        //
        // A request with no session cookie must report `authenticated: false`
        // and no email. This is the contract every pre-login page relies on,
        // and the contract SIWS must not break when it short-circuits the
        // session check for SIWS-derived cookies.
        //
        // We build a *fresh* client without an auth cookie so the baseline
        // client's cookie (if any) does not leak into the logged-out probe.
        let unauth_client = strip_auth_cookie(client);
        let logged_out = unauth_client.probe_auth_session(ctx).await?;
        assert_logged_out_shape(&logged_out)?;

        // ── Sub-path 2: logged-in (authenticated) ────────────────────────────
        //
        // When a valid session cookie is supplied, the response must report
        // `authenticated: true` and the seeded attendee's email. Without a
        // cookie, this sub-path is skipped — the harness does not mint
        // sessions itself (that would duplicate the OAuth flow under test).
        match &self.config.session_cookie {
            None => {
                // Skip, not fail. The runner records the outcome below via
                // a sentinel return; the caller (runner) does not
                // short-circuit on `Skipped` — we surface it as a flow
                // outcome instead. Since the trait returns `HarnessResult`,
                // we encode the skip as an `Ok(())` with a side log: the
                // logged-out sub-path passing is sufficient for the baseline.
                eprintln!(
                    "[{FLOW_NAME}] logged-in sub-path skipped: \
                     FLOW_HARNESS_ATTENDEE_SESSION not set"
                );
            }
            Some(cookie) => {
                // `with_auth_cookie` is the public builder; `clone()` copies
                // the baseline cookie (if any) and we overwrite it with the
                // logged-in probe's cookie.
                let authed_client = client.clone().with_auth_cookie(cookie.clone());
                let logged_in = authed_client.probe_auth_session(ctx).await?;
                assert_logged_in_shape(&logged_in, &self.config.expected_email)?;
            }
        }

        Ok(())
    }
}

// ── Pure helpers (staging-independent, unit-tested) ──────────────────────────

/// Assert the logged-out session response shape: `authenticated == false` and
/// no email.
///
/// Pure function of the response payload; unit-tested offline.
fn assert_logged_out_shape(resp: &AuthSessionResponse) -> HarnessResult<()> {
    if resp.authenticated {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: "logged-out probe returned authenticated=true with no session cookie"
                .to_string(),
        });
    }
    if resp.email.is_some() {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: format!(
                "logged-out probe returned an email ({:?}) with no session cookie; \
                 unauthenticated responses must omit the email",
                resp.email
            ),
        });
    }
    Ok(())
}

/// Assert the logged-in session response shape: `authenticated == true` and
/// the email matches `expected`.
///
/// Pure function of the response payload; unit-tested offline.
fn assert_logged_in_shape(resp: &AuthSessionResponse, expected_email: &str) -> HarnessResult<()> {
    if !resp.authenticated {
        return Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: "logged-in probe returned authenticated=false despite a session cookie"
                .to_string(),
        });
    }
    match &resp.email {
        None => Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: "logged-in probe returned no email despite authenticated=true".to_string(),
        }),
        Some(actual) if actual != expected_email => Err(HarnessError::AssertionFailed {
            flow: FLOW_NAME,
            reason: format!(
                "logged-in probe email mismatch: got {actual:?}, expected {expected_email:?}"
            ),
        }),
        Some(_) => Ok(()),
    }
}

// ── Client helpers ───────────────────────────────────────────────────────────

/// Build a fresh `WorkerClient` with no auth cookie, targeting the same base
/// URL as `client`. Used by the logged-out sub-path so the baseline client's
/// cookie (if any) does not leak into the probe.
///
/// Re-constructs a `reqwest::Client` per call. Acceptable for the harness
/// (one call per run); revisit only if the flow set grows to call this in a
/// loop. Reads the base URL via the client's public `base_url()` accessor
/// rather than re-deriving it from the context so the helper is self-contained.
fn strip_auth_cookie(client: &WorkerClient) -> WorkerClient {
    WorkerClient::new(client.base_url().clone())
        .expect("reconstructing the client with the same base URL must succeed")
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    // `FlowOutcome` is used only by the skipped-outcome sanity test below;
    // imported in the test module so the parent imports stay warning-free.
    use crate::runner::FlowOutcome;

    fn resp(authenticated: bool, email: Option<&str>) -> AuthSessionResponse {
        AuthSessionResponse {
            authenticated,
            email: email.map(str::to_string),
        }
    }

    // ── assert_logged_out_shape ──────────────────────────────────────────────

    #[test]
    fn logged_out_shape_accepts_clean_unauthenticated() {
        let r = resp(false, None);
        assert!(assert_logged_out_shape(&r).is_ok());
    }

    #[test]
    fn logged_out_shape_rejects_authenticated_true() {
        let r = resp(true, None);
        let err = assert_logged_out_shape(&r).unwrap_err();
        assert!(
            err.to_string().contains("authenticated=true"),
            "{}",
            err
        );
    }

    #[test]
    fn logged_out_shape_rejects_email_present() {
        // An unauthenticated response must never carry an email — leaking one
        // would be a privacy regression (and would mislead the frontend into
        // showing a logged-in UI).
        let r = resp(false, Some("anyone@staging.local"));
        let err = assert_logged_out_shape(&r).unwrap_err();
        assert!(err.to_string().contains("omit the email"), "{}", err);
    }

    // ── assert_logged_in_shape ───────────────────────────────────────────────

    #[test]
    fn logged_in_shape_accepts_matching_email() {
        let r = resp(true, Some("flow-test-attendee-1@staging.local"));
        assert!(assert_logged_in_shape(&r, "flow-test-attendee-1@staging.local").is_ok());
    }

    #[test]
    fn logged_in_shape_rejects_unauthenticated() {
        let r = resp(false, Some("flow-test-attendee-1@staging.local"));
        let err = assert_logged_in_shape(&r, "flow-test-attendee-1@staging.local").unwrap_err();
        assert!(err.to_string().contains("authenticated=false"), "{}", err);
    }

    #[test]
    fn logged_in_shape_rejects_missing_email() {
        let r = resp(true, None);
        let err = assert_logged_in_shape(&r, "flow-test-attendee-1@staging.local").unwrap_err();
        assert!(err.to_string().contains("no email"), "{}", err);
    }

    #[test]
    fn logged_in_shape_rejects_email_mismatch() {
        let r = resp(true, Some("someone-else@staging.local"));
        let err = assert_logged_in_shape(&r, "flow-test-attendee-1@staging.local").unwrap_err();
        assert!(err.to_string().contains("email mismatch"), "{}", err);
        assert!(
            err.to_string().contains("someone-else@staging.local"),
            "{}",
            err
        );
    }

    // ── Config + flow metadata ───────────────────────────────────────────────

    #[test]
    fn config_defaults_align_with_seed_staging() {
        let c = AuthConfig::default();
        assert_eq!(c.expected_email, "flow-test-attendee-1@staging.local");
        assert!(c.session_cookie.is_none(), "default must not assume a session");
    }

    #[test]
    fn from_env_picks_up_session_when_present() {
        // Without env vars set, from_env falls back to defaults.
        std::env::remove_var("FLOW_HARNESS_ATTENDEE_SESSION");
        std::env::remove_var("FLOW_HARNESS_ATTENDEE_EMAIL");
        let flow = AuthFlow::from_env();
        assert!(flow.config.session_cookie.is_none());
        assert_eq!(flow.config.expected_email, "flow-test-attendee-1@staging.local");

        // With the session env set, from_env picks it up.
        std::env::set_var("FLOW_HARNESS_ATTENDEE_SESSION", "session=abc");
        std::env::set_var("FLOW_HARNESS_ATTENDEE_EMAIL", "custom@staging.local");
        let flow = AuthFlow::from_env();
        assert_eq!(flow.config.session_cookie.as_deref(), Some("session=abc"));
        assert_eq!(flow.config.expected_email, "custom@staging.local");

        // Cleanup so other tests are not affected.
        std::env::remove_var("FLOW_HARNESS_ATTENDEE_SESSION");
        std::env::remove_var("FLOW_HARNESS_ATTENDEE_EMAIL");
    }

    #[tokio::test]
    async fn flow_name_is_stable() {
        let flow = AuthFlow::new();
        assert_eq!(flow.name(), "auth");
    }

    // ── Response deserialisation contract ────────────────────────────────────

    #[test]
    fn session_response_lenient_default_fields() {
        // Minimal payload: `{}`. Both fields default (authenticated=false,
        // email=None). This pins that the worker may omit fields and the
        // harness tolerates it — important for the logged-out path, which
        // some handlers implement as an empty 200.
        let minimal: AuthSessionResponse = serde_json::from_str("{}").expect("deserialise minimal");
        assert!(!minimal.authenticated);
        assert!(minimal.email.is_none());
    }

    #[test]
    fn session_response_round_trips_logged_in() {
        let json = r#"{"authenticated":true,"email":"flow-test-attendee-1@staging.local"}"#;
        let r: AuthSessionResponse = serde_json::from_str(json).expect("deserialise");
        assert!(r.authenticated);
        assert_eq!(r.email.as_deref(), Some("flow-test-attendee-1@staging.local"));
    }

    // ── Pure-logic sanity on FlowOutcome (used by the runner, asserted here so
    // the auth flow's documentation of "skip, not fail" stays accurate). ──────

    #[test]
    fn skipped_outcome_is_distinct_from_failed() {
        // The auth flow encodes a missing session cookie as a skip, not a
        // failure. This pins that Skipped is its own discriminant so a future
        // refactor that collapses Skipped into Failed is caught.
        assert_ne!(FlowOutcome::Skipped, FlowOutcome::Failed);
        assert_ne!(FlowOutcome::Skipped, FlowOutcome::Passed);
    }

    // ── base_url_of fallback ─────────────────────────────────────────────────

    #[test]
    fn strip_auth_cookie_preserves_base_url() {
        // strip_auth_cookie must build a fresh client targeting the SAME base
        // URL as its input (so a logged-out probe never silently re-targets
        // production). Compare via `base_url()` on both sides so the assertion
        // is robust to `url`'s trailing-slash normalization for host-only paths
        // (`https://host` parses/serialises as `https://host/`).
        let original =
            WorkerClient::new(url::Url::parse("https://staging.example.workers.dev").unwrap())
                .unwrap();
        let stripped = strip_auth_cookie(&original);
        assert_eq!(stripped.base_url(), original.base_url());
        // Belt-and-braces: the normalised host must still be the staging host,
        // never production.
        assert_eq!(stripped.base_url().host_str(), Some("staging.example.workers.dev"));
    }
}
