//! Typed HTTP client for the staging worker.
//!
//! Wraps [`reqwest::Client`] and exposes one typed method per worker endpoint
//! the harness drives, mirroring the route table in
//! `docs/escrow_contract_surface.md` §6. Each method:
//!  - builds the path from [`StagingContext`] URL helpers (single SSOT for
//!    path shape),
//!  - attaches the auth cookie when present (attendee flows need a session),
//!  - deserialises success bodies into `domain` types (or local mirrors where
//!    the worker shape is not in `domain`),
//!  - maps non-2xx responses into [`WorkerError`] with a parsed
//!    [`EscrowCode`] so negative-test flows can match on the exact revert.
//!
//! ## Staging-independence
//!
//! The client compiles and constructs without any network — the reqwest
//! `Client` is lazy. Only *invoking* the typed methods requires staging-live,
//! and the flow bodies gate those calls behind `// TODO(staging-live):`
//! markers. The error-parsing logic is unit-tested offline (see `tests`).
//!
//! ## Error envelope
//!
//! The worker's exact error envelope shape is not pinned in a shared type, so
//! [`parse_worker_error`] tolerates the three forms observed in the handlers:
//!  - `{"error":{"code":N,"message":"..."}}`  (escrow-originated)
//!  - `{"error":"..."}`                        (worker validation)
//!  - `{"code":N,"message":"..."}`             (flat)
//! Unknown shapes fall back to the raw body as the message with no code.

use reqwest::header::{COOKIE, HeaderMap, HeaderValue};
use reqwest::{Client, Method, Response, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use url::Url;

use domain::models::deposit::{DepositStatusResponse, UsdcDepositResponse};

use crate::context::StagingContext;
use crate::error::{EscrowCode, HarnessError, HarnessResult, WorkerError};

// ── Request/Response types not in `domain` ───────────────────────────────────
//
// These mirror the worker handler I/O shapes. They live here (not in `domain`)
// because they are harness-side concerns: the worker serialises escrow TXs as
// base64 strings regardless of the underlying program framework, and the
// harness only needs to forward them to a wallet/RPC. If a shape later moves
// into `domain`, prefer that and delete the local mirror.

/// `POST /api/deposit/usdc` body.
#[derive(Debug, Clone, Serialize)]
pub struct DepositUsdcRequest {
    pub attendee_id: String,
    pub event_id: String,
    pub wallet_address: String,
}

/// `POST /api/escrow/refund` body.
///
/// The worker pairs `refund + close_deposit` (per the contract surface §6 /
/// §8), so the harness only triggers a single endpoint. The returned TX is the
/// combined instruction sequence the attendee signs.
#[derive(Debug, Clone, Serialize)]
pub struct RefundRequest {
    pub attendee_id: String,
    pub event_id: String,
    pub wallet_address: String,
}

/// Common envelope for any endpoint that returns a base64 transaction the
/// attendee must sign and submit. Mirrors `UsdcDepositResponse` for the
/// refund path (the worker reuses the same shape).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TxResponse {
    /// Base64-encoded serialized transaction.
    pub transaction: String,
    /// Optional Solana Pay URL (deposit path only; refund path omits it).
    #[serde(default)]
    pub solana_pay_url: String,
}

/// `GET /api/auth/session` response — plan 006 baseline.
///
/// The harness asserts a logged-in session reports the seeded attendee email,
/// and a logged-out session reports `authenticated=false`. SIWS (006) must not
/// regress either branch.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuthSessionResponse {
    #[serde(default)]
    pub authenticated: bool,
    #[serde(default)]
    pub email: Option<String>,
}

/// `GET /api/claim/{token}` response.
///
/// Minimal shape — the claim flow primarily asserts reachability + state
/// transitions, not the full NFT mint envelope (which is a separate program
/// per the contract surface §6). Fields are lenient (`default`) so a richer
/// worker response deserialises without breaking the harness.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ClaimResponse {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub claimed: bool,
    #[serde(default)]
    pub message: Option<String>,
}

// ── Client ───────────────────────────────────────────────────────────────────

/// Typed HTTP client bound to a staging worker.
///
/// Construct via [`WorkerClient::new`] (no auth) or
/// [`WorkerClient::with_auth_cookie`] (attendee flows). Held by the runner for
/// the lifetime of a harness run; cheaply cloneable if a flow needs its own
/// copy (the underlying [`reqwest::Client`] is `Arc`-internally).
#[derive(Clone)]
pub struct WorkerClient {
    http: Client,
    base_url: Url,
    auth_cookie: Option<String>,
}

impl std::fmt::Debug for WorkerClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerClient")
            .field("base_url", &self.base_url)
            .field("has_auth_cookie", &self.auth_cookie.is_some())
            .finish_non_exhaustive()
    }
}

impl WorkerClient {
    /// Create a client targeting `base_url` with sensible defaults (30s
    /// timeout, JSON accept, redirect-follow on for the OAuth callback path).
    pub fn new(base_url: Url) -> HarnessResult<Self> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .user_agent("bethere-flow-harness/0.1")
            .build()
            .map_err(|e| HarnessError::Transport(format!("reqwest build: {e}")))?;
        Ok(Self {
            http,
            base_url,
            auth_cookie: None,
        })
    }

    /// Attach a session cookie (the value of the `Cookie` header, e.g.
    /// `session=eyJ...`). Required for attendee-scoped endpoints.
    #[must_use]
    pub fn with_auth_cookie(mut self, cookie: String) -> Self {
        self.auth_cookie = Some(cookie);
        self
    }

    /// The base URL this client targets. Used by flows that need to spawn a
    /// sibling client with different headers (e.g. the auth flow builds an
    /// unauthenticated client for the logged-out probe without re-deriving
    /// the URL from the context).
    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    // ── Typed endpoint methods (mirror contract surface §6) ─────────────────

    /// `GET /api/deposit/status/{attendee_id}` — the primary read path.
    /// Every refund/claim flow fetches status through here to assert the
    /// horizon fields and predicted refund outcome.
    pub async fn fetch_deposit_status(
        &self,
        ctx: &StagingContext,
        attendee_id: &str,
    ) -> HarnessResult<DepositStatusResponse> {
        let url = ctx.deposit_status_url(attendee_id)?;
        self.get_json(url).await
    }

    /// `POST /api/deposit/usdc` — request a Solana Pay deposit TX.
    pub async fn request_deposit_usdc(
        &self,
        ctx: &StagingContext,
        req: &DepositUsdcRequest,
    ) -> HarnessResult<UsdcDepositResponse> {
        let url = ctx.deposit_usdc_url()?;
        self.post_json(url, req).await
    }

    /// `POST /api/escrow/refund` — request the paired `refund + close_deposit`
    /// TX. Negative-test flows expect this to return a non-2xx with the
    /// relevant [`EscrowCode`]; positive flows sign+submit the returned TX.
    pub async fn request_refund(
        &self,
        ctx: &StagingContext,
        req: &RefundRequest,
    ) -> HarnessResult<TxResponse> {
        let url = ctx.refund_url()?;
        self.post_json(url, req).await
    }

    /// `GET /api/claim/{token}` — NFT claim probe.
    pub async fn fetch_claim(
        &self,
        ctx: &StagingContext,
        token: &str,
    ) -> HarnessResult<ClaimResponse> {
        let url = ctx.claim_url(token)?;
        self.get_json(url).await
    }

    /// `GET /api/auth/session` — plan 006 SIWS regression baseline.
    pub async fn probe_auth_session(&self, ctx: &StagingContext) -> HarnessResult<AuthSessionResponse> {
        let url = ctx.auth_session_url()?;
        self.get_json(url).await
    }

    // ── Low-level helpers ────────────────────────────────────────────────────

    /// GET a JSON body, deserialising into `T` on 2xx, mapping non-2xx to
    /// [`WorkerError`].
    async fn get_json<T: DeserializeOwned>(&self, url: Url) -> HarnessResult<T> {
        let resp = self
            .send_request(Method::GET, url, None)
            .await?;
        self.decode_or_error(resp).await
    }

    /// POST a JSON body, deserialising the response into `T` on 2xx.
    async fn post_json<T: DeserializeOwned, B: Serialize>(
        &self,
        url: Url,
        body: &B,
    ) -> HarnessResult<T> {
        let serialized = serde_json::to_string(body)?;
        let resp = self
            .send_request(Method::POST, url, Some(serialized))
            .await?;
        self.decode_or_error(resp).await
    }

    async fn send_request(
        &self,
        method: Method,
        url: Url,
        body: Option<String>,
    ) -> HarnessResult<Response> {
        let mut req = self.http.request(method, url.clone());
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::ACCEPT,
            HeaderValue::from_static("application/json"),
        );
        if let Some(cookie) = &self.auth_cookie {
            headers.insert(
                COOKIE,
                HeaderValue::from_str(cookie)
                    .map_err(|e| HarnessError::Config(format!("invalid cookie value: {e}")))?,
            );
        }
        req = req.headers(headers);
        if let Some(body) = body {
            req = req
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body);
        }
        req.send().await.map_err(HarnessError::from)
    }

    /// Deserialise a 2xx response, or turn a non-2xx into a [`WorkerError`].
    async fn decode_or_error<T: DeserializeOwned>(&self, resp: Response) -> HarnessResult<T> {
        let status = resp.status();
        if status.is_success() {
            return resp.json::<T>().await.map_err(HarnessError::from);
        }
        let body_text = resp.text().await.unwrap_or_default();
        Err(HarnessError::Worker(parse_worker_error(status, &body_text)))
    }
}

// ── Error parsing ────────────────────────────────────────────────────────────

/// Parse a non-2xx response body into a [`WorkerError`], extracting the escrow
/// code when the worker surfaced one.
///
/// Tolerates the three observed envelope shapes (see module docs). The body is
/// parsed leniently: a malformed JSON body becomes the message verbatim with
/// no code rather than a decode error, because the harness cares about the
/// HTTP-level failure first and the code second.
pub fn parse_worker_error(status: StatusCode, body: &str) -> WorkerError {
    let trimmed = body.trim();
    let (code, message) = match serde_json::from_str::<Value>(trimmed) {
        Ok(v) => extract_code_and_message(&v),
        Err(_) => (None, trimmed.to_string()),
    };
    let message = if message.is_empty() {
        status.canonical_reason().unwrap_or("unknown error").to_string()
    } else {
        message
    };
    WorkerError {
        http_status: status.as_u16(),
        code,
        message,
    }
}

/// Walk a parsed JSON body looking for an escrow code + message, tolerating
/// the three known envelope shapes.
fn extract_code_and_message(v: &Value) -> (Option<EscrowCode>, String) {
    // Shape 1: {"error": {"code": N, "message": "..."}}
    if let Some(err_obj) = v.get("error").filter(|v| v.is_object()) {
        let code = err_obj
            .get("code")
            .and_then(Value::as_u64)
            .map(|n| EscrowCode::from_u32(n as u32));
        let message = err_obj
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| err_obj.get("error").and_then(Value::as_str))
            .unwrap_or("")
            .to_string();
        return (code, message);
    }

    // Shape 2: {"error": "..."}
    if let Some(msg) = v.get("error").and_then(Value::as_str) {
        let code = v
            .get("code")
            .and_then(Value::as_u64)
            .map(|n| EscrowCode::from_u32(n as u32));
        return (code, msg.to_string());
    }

    // Shape 3: {"code": N, "message": "..."}
    let code = v
        .get("code")
        .and_then(Value::as_u64)
        .map(|n| EscrowCode::from_u32(n as u32));
    let message = v
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    (code, message)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    #[test]
    fn parses_nested_error_with_escrow_code() {
        let body = r#"{"error":{"code":19,"message":"refund deadline passed"}}"#;
        let e = parse_worker_error(StatusCode::BAD_REQUEST, body);
        assert_eq!(e.http_status, 400);
        assert_eq!(e.code, Some(EscrowCode::RefundDeadlinePassed));
        assert_eq!(e.message, "refund deadline passed");
    }

    #[test]
    fn parses_string_error_without_code() {
        let body = r#"{"error":"wallet address required"}"#;
        let e = parse_worker_error(StatusCode::BAD_REQUEST, body);
        assert_eq!(e.code, None);
        assert_eq!(e.message, "wallet address required");
    }

    #[test]
    fn parses_flat_code_message_shape() {
        let body = r#"{"code":1,"message":"refund not yet allowed"}"#;
        let e = parse_worker_error(StatusCode::FORBIDDEN, body);
        assert_eq!(e.http_status, 403);
        assert_eq!(e.code, Some(EscrowCode::RefundNotYetAllowed));
    }

    #[test]
    fn unknown_code_round_trips_via_other() {
        let body = r#"{"error":{"code":77,"message":"?"}}"#;
        let e = parse_worker_error(StatusCode::INTERNAL_SERVER_ERROR, body);
        match e.code {
            Some(EscrowCode::Other(77)) => {}
            other => panic!("expected Other(77), got {other:?}"),
        }
    }

    #[test]
    fn malformed_body_falls_back_to_status_reason() {
        let e = parse_worker_error(StatusCode::NOT_FOUND, "<html>not found</html>");
        assert_eq!(e.code, None);
        // Body is preserved verbatim when non-empty.
        assert_eq!(e.message, "<html>not found</html>");
    }

    #[test]
    fn empty_body_uses_canonical_status_reason() {
        let e = parse_worker_error(StatusCode::NOT_FOUND, "");
        assert_eq!(e.code, None);
        assert_eq!(e.message, "Not Found");
    }

    #[test]
    fn client_debug_does_not_print_auth_cookie_value() {
        let url = Url::parse("https://staging.example.workers.dev").unwrap();
        let client = WorkerClient::new(url)
            .unwrap()
            .with_auth_cookie("session=secret-value".to_string());
        let s = format!("{client:?}");
        assert!(s.contains("has_auth_cookie: true"));
        assert!(!s.contains("secret-value"));
    }

    #[test]
    fn tx_response_lenient_deserialises_without_solana_pay_url() {
        // Refund path omits solana_pay_url; the field is `#[serde(default)]`.
        let json = r#"{"transaction":"base64data"}"#;
        let r: TxResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.transaction, "base64data");
        assert_eq!(r.solana_pay_url, "");
    }
}
