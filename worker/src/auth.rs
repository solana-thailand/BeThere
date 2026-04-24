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

/// Check if a given email is in the staff emails allowlist.
///
/// Checks both the env var `STAFF_EMAILS` list (fast, static) and the
/// Google Sheets "staff" tab (dynamic). Returns `true` if the email
/// appears in either source.
pub async fn is_staff(email: &str, state: &AppState) -> bool {
    // Fast path: check the static env var allowlist first
    if state.is_staff(email) {
        return true;
    }

    // Slow path: fetch staff emails from the Google Sheets "staff" tab
    match sheets::get_staff_emails(state).await {
        Ok(sheet_emails) => sheet_emails
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(email)),
        Err(e) => {
            tracing::warn!("failed to fetch staff emails from sheet, using env var list only: {e}");
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

    // Verify staff status (checks both env var list and staff sheet)
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
            host: "0.0.0.0".to_string(),
            port: 3000,
        };

        AppState {
            config: std::sync::Arc::new(config),
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
