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
use serde_json::json;

use event_checkin_domain::models::auth::{Claims, GoogleUserInfo, TokenRequest};

use crate::crypto;
use crate::http;
use crate::sheets;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// OAuth helpers
// ---------------------------------------------------------------------------

/// Build the Google OAuth 2.0 authorization URL.
/// This URL redirects the user to Google's consent screen.
pub fn get_auth_url(state: &AppState) -> String {
    let config = &state.config.google_oauth;
    let params = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", &config.client_id)
        .append_pair("redirect_uri", &config.redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", "openid email profile")
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .finish();

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
    match sheets::get_staff_members(
        state,
        &state.config.sheets.sheet_id,
        &state.config.sheets.staff_sheet_name,
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
                "failed to fetch staff members from sheet, falling back to env var list: {e}"
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
            tracing::warn!("failed to list events for auth fallback: {e}");
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
            tracing::warn!("failed to list events for role check: {e}");
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
            tracing::debug!("auth middleware rejected request on {path}: {e}");
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "error": e,
                })),
            )
                .into_response();
        }
    };

    // Verify staff status:
    //   1. Global sources (env var list + Google Sheet staff tab)
    //   2. Per-event assignments (organizer_emails / staff_emails in event registry)
    if !is_staff(&claims.email, &state).await {
        tracing::warn!("non-staff user attempted access: {}", claims.email);
        return (
            axum::http::StatusCode::FORBIDDEN,
            Json(json!({
                "success": false,
                "error": "user is not in staff allowlist",
            })),
        )
            .into_response();
    }

    // Inject claims into request extensions for downstream handlers
    req.extensions_mut().insert(claims);

    next.run(req).await
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Extract JWT from Authorization header or cookie.
fn extract_token_from_request(req: &Request) -> Option<String> {
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
async fn verify_token(token: &Option<String>, state: &AppState) -> Result<Claims, String> {
    let token = token.as_ref().ok_or("missing authentication token")?;
    verify_session_jwt(token, &state.config.jwt_secret).await
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
        AppConfig, GoogleOAuthConfig, GoogleServiceAccountConfig, SheetsConfig,
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
            },
            jwt_secret: "test-jwt-secret".to_string(),
            staff_emails: vec![
                "admin@example.com".to_string(),
                "staff@example.com".to_string(),
            ],
            server_url: "http://localhost:3000".to_string(),
            claim_base_url: "http://localhost:3000/claim".to_string(),
            helius_rpc_url: "https://devnet.helius-rpc.com".to_string(),
            helius_api_key: "test-helius-key".to_string(),
            nft_collection_mint: "test-collection-mint".to_string(),
            nft_metadata_uri: "https://arweave.net/test-metadata".to_string(),
            nft_image_url: "https://arweave.net/test-image".to_string(),
            event_name: "Test Event".to_string(),
            event_tagline: "Test Tagline".to_string(),
            event_link: "https://example.com/event".to_string(),
            event_start_ms: 0,
            event_end_ms: 0,
            super_admin_emails: vec!["admin@example.com".to_string()],
            host: "0.0.0.0".to_string(),
            port: 3000,
        };

        AppState {
            config: std::sync::Arc::new(config),
            quiz_kv: None,
            events_kv: None,
        }
    }

    #[test]
    fn test_get_auth_url_contains_required_params() {
        let state = test_state();
        let url = get_auth_url(&state);

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
