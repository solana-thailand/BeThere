//! Authentication module for the Cloudflare Worker.
//!
//! Provides OAuth URL generation, callback handling, JWT session management,
//! and auth middleware — all using SubtleCrypto (via `crate::crypto`) and
//! `worker::Fetch` (via `crate::http`) instead of `jsonwebtoken` + `reqwest`.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Json},
};

use event_checkin_domain::models::api::ApiResponse;
use event_checkin_domain::models::auth::{Claims, GoogleUserInfo, TokenRequest};

use crate::crypto;
use crate::http;
use crate::sheets;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// JWT blacklist (VULN-011)
// ---------------------------------------------------------------------------

/// KV key prefix for blacklisted JWTs.
const JWT_BLACKLIST_PREFIX: &str = "jwt_blacklist:";

/// Minimum TTL for KV entries (Cloudflare KV requires >= 60s).
const KV_MIN_TTL: u64 = 60;

/// Add a JWT to the blacklist.
/// D1-first: writes to `jwt_blacklist` table. Falls back to KV with TTL.
pub async fn blacklist_token(token: &str, claims: &Claims, state: &AppState) {
    let hash = sha256_hex(token.as_bytes()).await;
    let now = chrono::Utc::now().timestamp() as u64;
    let expires_at = claims.exp;
    let ttl = (claims.exp.saturating_sub(now)).max(KV_MIN_TTL);

    // D1-first
    if let Some(ref db) = state.d1
        && let Err(e) = crate::db::jwt_blacklist::insert(db, &hash, expires_at).await
    {
        tracing::warn!(hash = %hash, error = %e, "failed to blacklist JWT in D1");
    }

    // KV fallback (also write to KV for redundancy during migration)
    if let Some(ref kv) = state.events_kv {
        let key = format!("{JWT_BLACKLIST_PREFIX}{hash}");
        match kv.put(&key, "1") {
            Ok(builder) => {
                if let Err(e) = builder.expiration_ttl(ttl).execute().await {
                    tracing::warn!(key = %key, error = ?e, "failed to blacklist JWT in KV");
                }
            }
            Err(e) => {
                tracing::warn!(key = %key, error = ?e, "failed to build JWT blacklist KV put");
            }
        }
    }

    if state.d1.is_none() && state.events_kv.is_none() {
        tracing::warn!("no D1 or KV available — JWT blacklist skipped");
    }
}

/// Check if a JWT has been blacklisted.
/// D1-first: checks `jwt_blacklist` table. Falls back to KV.
async fn is_token_blacklisted(token: &str, state: &AppState) -> bool {
    let hash = sha256_hex(token.as_bytes()).await;

    // D1-first
    if let Some(ref db) = state.d1 {
        return crate::db::jwt_blacklist::exists(db, &hash)
            .await
            .unwrap_or(false);
    }

    // KV fallback
    if let Some(ref kv) = state.events_kv {
        let key = format!("{JWT_BLACKLIST_PREFIX}{hash}");
        return matches!(kv.get(&key).text().await, Ok(Some(_)));
    }

    false
}

/// SHA-256 hash (hex-encoded) for JWT blacklist keys.
/// Uses SubtleCrypto in WASM, pure Rust SHA-256 in native tests.
/// Replaces the previous FNV-1a approach (VULN-007).
async fn sha256_hex(data: &[u8]) -> String {
    match crate::solana_escrow::crypto::sha256(data).await {
        Ok(hash) => hash.iter().map(|b| format!("{b:02x}")).collect(),
        Err(e) => {
            // FNV-1a fallback — should never happen but prevents auth breakage
            tracing::warn!(error = %e, "SHA-256 failed for JWT blacklist, using FNV-1a fallback");
            let mut h: u64 = 0xcbf29ce484222325;
            for &b in data {
                h ^= b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
            format!("{h:016x}")
        }
    }
}

// ---------------------------------------------------------------------------
// OAuth helpers
// ---------------------------------------------------------------------------

/// Build the Google OAuth 2.0 authorization URL.
/// This URL redirects the user to Google's consent screen.
pub fn get_auth_url(state: &AppState, redirect: Option<&str>) -> String {
    let config = &state.config.google_oauth;
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer
        .append_pair("client_id", &config.client_id)
        .append_pair("redirect_uri", &config.redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", "openid email profile")
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");

    if let Some(redirect) = redirect {
        serializer.append_pair("state", redirect);
    }

    let params = serializer.finish();
    format!("https://accounts.google.com/o/oauth2/v2/auth?{params}")
}

/// Handle the OAuth callback: exchange the authorization code for tokens,
/// fetch user info from Google, and return the user info.
///
/// Uses `worker::Fetch` via `crate::http` instead of `reqwest`.
pub async fn handle_callback(code: &str, state: &AppState) -> Result<GoogleUserInfo, String> {
    let config = &state.config.google_oauth;

    // Exchange code for tokens
    let token_request = TokenRequest::new(
        code.to_string(),
        config.client_id.clone(),
        config.client_secret.clone(),
        config.redirect_uri.clone(),
    );

    let token_response = http::exchange_oauth_code(&token_request).await?;

    // Fetch user info using the access token
    let user_info = http::fetch_user_info(&token_response.access_token).await?;

    Ok(user_info)
}

/// Check if a given email is authorized to access the platform.
///
/// Checks in order:
/// 1. Global `STAFF_EMAILS` env var list (fast, static)
/// 2. Google Sheets "staff" tab (dynamic, with role column)
/// 3. Per-event `organizer_emails` / `staff_emails` in event registry KV
///
/// Returns `true` if the email appears in any source.
pub async fn is_staff(email: &str, state: &AppState) -> bool {
    // Fast path: global sources (env var + Google Sheet)
    if get_staff_role(email, state).await.is_some() {
        return true;
    }

    // Fallback: check per-event assignments in KV
    is_event_assigned(email, state).await
}

/// Get the role for a staff member by email.
///
/// Checks the Google Sheets "staff" tab first (dynamic, supports roles),
/// then falls back to the env var `STAFF_EMAILS` list with default "staff" role.
///
/// Returns `Some(role)` if the email is authorized, `None` otherwise.
/// Role values: "admin" (scanner + admin dashboard) or "staff" (scanner only).
pub async fn get_staff_role(email: &str, state: &AppState) -> Option<String> {
    // Check the Google Sheets "staff" tab first (supports role column B)
    let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());
    match sheets::get_staff_members(
        state,
        &state.config.sheets.sheet_id,
        &state.config.sheets.staff_sheet_name,
        kv,
    )
    .await
    {
        Ok(members) => {
            if let Some(member) = members.iter().find(|m| m.email.eq_ignore_ascii_case(email)) {
                return Some(member.role.clone());
            }
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "failed to fetch staff members from sheet, falling back to env var list"
            );
        }
    }

    // Fallback: check the static env var allowlist (default role: "staff")
    if state.is_staff(email) {
        return Some("staff".to_string());
    }

    None
}

/// Check if a user is assigned as organizer or staff in **any** event config.
///
/// This is the fallback path in `is_staff()` for users not in global sources.
/// Two-pass check:
/// 1. Fast path: `EventMeta.organizer_emails` (no extra KV read)
/// 2. Slow path: load full `EventConfig` to check `staff_emails`
///
/// Returns `true` if the email appears in any event's organizer or staff list.
pub async fn is_event_assigned(email: &str, state: &AppState) -> bool {
    let kv = match state.events_kv {
        Some(ref kv) => kv,
        None => return false,
    };

    let all_events = match crate::event_store::list_events(kv).await {
        Ok(events) => events,
        Err(e) => {
            tracing::warn!(error = %e, "failed to list events for auth fallback");
            return false;
        }
    };

    // Fast path: check organizer_emails in EventMeta (already loaded)
    for meta in &all_events {
        if meta
            .organizer_emails
            .iter()
            .any(|e| e.eq_ignore_ascii_case(email))
        {
            return true;
        }
    }

    // Slow path: load full configs to check staff_emails
    for meta in &all_events {
        if let Ok(Some(config)) = crate::event_store::get_event_config(kv, &meta.id).await
            && crate::event_store::is_event_staff(&config, email)
        {
            return true;
        }
    }

    false
}

/// Check if a user is an **organizer** in any event (fast path only).
///
/// Only checks `EventMeta.organizer_emails` — no full config loading needed.
/// Used by `auth_me` to report the correct role without expensive KV reads.
pub async fn is_event_organizer_any(email: &str, state: &AppState) -> bool {
    let kv = match state.events_kv {
        Some(ref kv) => kv,
        None => return false,
    };

    match crate::event_store::list_events(kv).await {
        Ok(events) => events.iter().any(|meta| {
            meta.organizer_emails
                .iter()
                .any(|e| e.eq_ignore_ascii_case(email))
        }),
        Err(e) => {
            tracing::warn!(error = %e, "failed to list events for role check");
            false
        }
    }
}

// ---------------------------------------------------------------------------
// JWT session management
// ---------------------------------------------------------------------------

/// Create a JWT session token for an authenticated staff member.
///
/// Delegates to `crypto::create_jwt` which uses HMAC-SHA256 via SubtleCrypto.
pub async fn create_session_jwt(email: &str, sub: &str, secret: &str) -> Result<String, String> {
    crypto::create_jwt(email, sub, secret).await
}

/// Verify and decode a JWT session token.
///
/// Delegates to `crypto::verify_jwt` which uses HMAC-SHA256 via SubtleCrypto.
pub async fn verify_session_jwt(token: &str, secret: &str) -> Result<Claims, String> {
    crypto::verify_jwt(token, secret).await
}

// ---------------------------------------------------------------------------
// Auth middleware
// ---------------------------------------------------------------------------

/// Auth middleware that extracts and verifies JWT from the Authorization header or cookie.
/// Injects the Claims into request extensions for downstream handlers.
/// Public routes (health, auth/url, auth/callback, auth/logout) are skipped.
#[worker::send]
pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> axum::response::Response {
    let path = req.uri().path();

    // Skip auth for public routes
    if is_public_route(path) {
        return next.run(req).await;
    }

    // Extract JWT from Authorization header or cookie
    let token = extract_token_from_request(&req);

    // Verify JWT and extract claims
    let claims = match verify_token(&token, &state).await {
        Ok(claims) => claims,
        Err(e) => {
            tracing::debug!(path = %path, error = %e, "auth middleware rejected request");
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some(e),
                    correlation_id: None,
                }),
            )
                .into_response();
        }
    };

    // Verify staff status:
    //   1. Global sources (env var list + Google Sheet staff tab)
    //   2. Per-event assignments (organizer_emails / staff_emails in event registry)
    if !is_staff(&claims.email, &state).await {
        tracing::warn!(email = %claims.email, "non-staff user attempted access");
        return (
            axum::http::StatusCode::FORBIDDEN,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some("user is not in staff allowlist".to_string()),
                correlation_id: None,
            }),
        )
            .into_response();
    }

    // Inject claims into request extensions for downstream handlers
    req.extensions_mut().insert(claims);

    next.run(req).await
}

/// Identity-only middleware that extracts and verifies JWT without staff checks.
///
/// Same as `require_auth` but does NOT verify staff status. Used for attendee-facing
/// routes where we only need a verified email (e.g. registration, my-registration).
/// Injects `Claims` into request extensions for downstream handlers.
#[worker::send]
pub async fn require_identity(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> axum::response::Response {
    let path = req.uri().path();

    // Skip auth for public routes
    if is_public_route(path) {
        return next.run(req).await;
    }

    // Extract JWT from Authorization header or cookie
    let token = extract_token_from_request(&req);

    // Verify JWT and extract claims (no staff check)
    let claims = match verify_token(&token, &state).await {
        Ok(claims) => claims,
        Err(e) => {
            tracing::debug!(path = %path, error = %e, "identity middleware rejected request");
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some(e),
                    correlation_id: None,
                }),
            )
                .into_response();
        }
    };

    // Inject claims into request extensions for downstream handlers
    req.extensions_mut().insert(claims);

    next.run(req).await
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Extract JWT from Authorization header or cookie.
pub fn extract_token_from_request(req: &Request) -> Option<String> {
    // Try Authorization header first (for API clients)
    if let Some(auth_header) = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        && let Some(token) = auth_header.strip_prefix("Bearer ")
    {
        return Some(token.to_string());
    }

    // Try cookie (for browser sessions)
    for cookie_header in req.headers().get_all("cookie").iter() {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for cookie in cookie_str.split(';') {
                let cookie = cookie.trim();
                if let Some(token) = cookie.strip_prefix("event_checkin_token=") {
                    return Some(token.to_string());
                }
            }
        }
    }

    None
}

/// Verify a JWT token and return claims.
///
/// In dev mode (`DEV_MODE=1`), accepts the literal string `dev-token`
/// and returns synthetic Claims with the configured `dev_email`.
pub(crate) async fn verify_token(
    token: &Option<String>,
    state: &AppState,
) -> Result<Claims, String> {
    // Dev mode bypass — accept "dev-token" without JWT verification
    if state.config.dev_mode {
        let token = token.as_ref().ok_or("missing authentication token")?;
        if token == "dev-token" {
            return Ok(Claims::new(
                state.config.dev_email.clone(),
                "dev-subject".to_string(),
            ));
        }
        // If not "dev-token", still try normal JWT verification (e.g. real OAuth sessions)
    }

    let token = token.as_ref().ok_or("missing authentication token")?;
    let claims = verify_session_jwt(token, &state.config.jwt_secret).await?;

    // VULN-011: Check JWT blacklist (logged-out tokens)
    if is_token_blacklisted(token, state).await {
        tracing::debug!(email = %claims.email, "rejected blacklisted JWT");
        return Err("token has been revoked".to_string());
    }

    Ok(claims)
}

/// Check if a route should bypass authentication.
/// Supports both `/api/...` (full path) and `/...` (stripped prefix inside nested router).
fn is_public_route(path: &str) -> bool {
    matches!(
        path,
        "/api/health"
            | "/health"
            | "/api/auth/url"
            | "/auth/url"
            | "/api/auth/callback"
            | "/auth/callback"
            | "/api/auth/logout"
            | "/auth/logout"
    )
}

// ---------------------------------------------------------------------------
// Role-based access control
// ---------------------------------------------------------------------------

/// User role levels for access control.
/// Ordered by privilege: Staff < Organizer < SuperAdmin.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum UserRole {
    /// Scanner only — can check in attendees.
    Staff,
    /// Event management — can CRUD events they organize.
    Organizer,
    /// Global admin — can create/manage all events.
    SuperAdmin,
}

/// Resolve the highest role for a user across global config and event config.
///
/// Checks in order: super_admin → event organizer → Google Sheet organizer → event staff → Google Sheet staff.
/// Uses the existing `is_event_organizer` / `is_event_staff` helpers from event_store
/// to avoid duplicating email-matching logic.
pub async fn resolve_user_role(
    email: &str,
    state: &AppState,
    event_config: Option<&event_checkin_domain::models::event::EventConfig>,
) -> UserRole {
    // 1. Super admin (global config)
    if state
        .config
        .super_admin_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(email))
    {
        return UserRole::SuperAdmin;
    }

    // 2. Per-event organizer (event config)
    if let Some(ec) = event_config
        && crate::event_store::is_event_organizer(ec, email)
    {
        return UserRole::Organizer;
    }

    // 3. Google Sheet role (fallback — sheet is source of truth for global roles)
    if let Some(role) = get_staff_role(email, state).await.as_deref()
        && matches!(role, "admin" | "organizer")
    {
        return UserRole::Organizer;
    }
    // Sheet says "staff" — continue to check event staff

    // 4. Per-event staff (event config)
    if let Some(ec) = event_config
        && crate::event_store::is_event_staff(ec, email)
    {
        return UserRole::Staff;
    }

    // 5. Global staff (they already passed require_auth, so at least Staff)
    UserRole::Staff
}

/// Check if a user has access to operate on a specific event.
///
/// Returns `Ok(())` if access is granted, `Err(reason)` if denied.
/// Handlers should return the error string in a JSON response.
///
/// Access hierarchy:
/// - **SuperAdmin** → always allowed (global admin)
/// - **Organizer** in event config → allowed
/// - **Organizer** in Google Sheet staff tab → allowed (fallback)
/// - **Staff** in event config → allowed (scanner only)
/// - **Staff** in Google Sheet staff tab → allowed (scanner only, fallback)
/// - Any other authenticated staff → denied (not assigned to this event)
pub async fn check_event_access(
    email: &str,
    state: &AppState,
    event_config: &event_checkin_domain::models::event::EventConfig,
) -> Result<(), String> {
    // 1. SuperAdmin → always allowed
    if state
        .config
        .super_admin_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(email))
    {
        return Ok(());
    }

    // 2. Per-event organizer or staff (event config)
    if crate::event_store::has_event_access(event_config, email) {
        return Ok(());
    }

    // 3. Google Sheet role (fallback — sheet is source of truth)
    if let Some(role) = get_staff_role(email, state).await.as_deref()
        && matches!(role, "admin" | "organizer" | "staff")
    {
        return Ok(());
    }

    Err(format!(
        "you are not assigned to event '{}' — contact the event organizer",
        event_config.name
    ))
}

// ---------------------------------------------------------------------------
// Tests (pure logic only — no SubtleCrypto available in unit tests)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use event_checkin_domain::config::{
        AppConfig, EventDefaults, GoogleOAuthConfig, GoogleServiceAccountConfig, NftConfig,
        ServerConfig, SheetsConfig, SolanaConfig,
    };

    fn test_state() -> AppState {
        let config = AppConfig {
            google_oauth: GoogleOAuthConfig {
                client_id: "test-client-id".to_string(),
                client_secret: "test-secret".to_string(),
                redirect_uri: "http://localhost:3000/api/auth/callback".to_string(),
            },
            service_account: GoogleServiceAccountConfig {
                client_email: "test@test.iam.gservicemain.com".to_string(),
                private_key: "test-key".to_string(),
                token_uri: "https://oauth2.googleapis.com/token".to_string(),
            },
            sheets: SheetsConfig {
                sheet_id: "test-sheet-id".to_string(),
                sheet_name: "Sheet1".to_string(),
                staff_sheet_name: "staff".to_string(),
                contacts_sheet_id: String::new(),
                contacts_sheet_name: "Contacts".to_string(),
                events_sheet_name: "Events".to_string(),
                platform_sheet_id: String::new(),
            },
            jwt_secret: "test-jwt-secret".to_string(),
            staff_emails: [
                "admin@example.com".to_string(),
                "staff@example.com".to_string(),
            ]
            .into_iter()
            .collect(),
            super_admin_emails: ["admin@example.com".to_string()].into_iter().collect(),
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 3000,
                url: "http://localhost:3000".to_string(),
                claim_base_url: "http://localhost:3000/claim".to_string(),
            },
            solana: SolanaConfig {
                rpc_url: "https://devnet.helius-rpc.com".to_string(),
                api_key: "test-helius-key".to_string(),
            },
            nft: NftConfig {
                collection_mint: "test-collection-mint".to_string(),
                metadata_uri: "https://arweave.net/test-metadata".to_string(),
                image_url: "https://arweave.net/test-image".to_string(),
            },
            event_defaults: EventDefaults {
                name: "Test Event".to_string(),
                tagline: "Test Tagline".to_string(),
                link: "https://example.com/event".to_string(),
                start_ms: 0,
                end_ms: 0,
                deposit_enabled: false,
                deposit_amount_usdc: 0,
                deposit_amount_thb: 0,
                promptpay_id: String::new(),
            },
            dev_mode: false,
            dev_email: "dev@localhost".to_string(),
        };

        AppState {
            config: std::sync::Arc::new(config),
            quiz_kv: None,
            events_kv: None,
            d1: None,
            r2: None,
            event_do: None,
            webhook_secret: String::new(),
            worker_ctx: None,
        }
    }

    #[test]
    fn test_get_auth_url_contains_required_params() {
        let state = test_state();
        let url = get_auth_url(&state, None);

        assert!(url.contains("accounts.google.com/o/oauth2/v2/auth"));
        assert!(url.contains("client_id=test-client-id"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A3000"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=openid+email+profile"));
    }

    /// Test the static (env var) staff check via AppState::is_staff.
    /// The async is_staff() also queries the Google Sheets "staff" tab,
    /// which is not available in unit tests, so we test the fast path only.
    #[test]
    fn test_is_staff_allowed() {
        let state = test_state();
        assert!(state.is_staff("admin@example.com"));
        assert!(state.is_staff("staff@example.com"));
        assert!(state.is_staff("Admin@Example.COM")); // case insensitive
    }

    #[test]
    fn test_is_staff_not_allowed() {
        let state = test_state();
        assert!(!state.is_staff("random@example.com"));
        assert!(!state.is_staff("unknown@gmail.com"));
    }

    #[test]
    fn test_is_public_route() {
        assert!(is_public_route("/api/health"));
        assert!(is_public_route("/health"));
        assert!(is_public_route("/api/auth/url"));
        assert!(is_public_route("/auth/url"));
        assert!(is_public_route("/api/auth/callback"));
        assert!(is_public_route("/auth/callback"));
        assert!(is_public_route("/api/auth/logout"));
        assert!(is_public_route("/auth/logout"));
    }

    #[test]
    fn test_is_not_public_route() {
        assert!(!is_public_route("/api/attendees"));
        assert!(!is_public_route("/attendees"));
        assert!(!is_public_route("/api/checkin/abc123"));
        assert!(!is_public_route("/api/auth/me"));
        assert!(!is_public_route("/auth/me"));
        assert!(!is_public_route("/staff"));
        assert!(!is_public_route("/admin"));
    }
}
