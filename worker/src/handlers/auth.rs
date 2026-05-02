//! Auth handlers for the Cloudflare Worker.
//!
//! Mirrors `src/handlers/auth.rs` from the Axum build but uses async JWT
//! operations (SubtleCrypto via `crate::crypto`) and `worker::Fetch` (via
//! `crate::http`) instead of sync `jsonwebtoken` + `reqwest`.

use axum::{
    Extension,
    extract::{Query, State},
    http::{HeaderValue, header},
    response::{IntoResponse, Json, Redirect, Response},
};
use serde::Deserialize;
use serde_json::json;

use event_checkin_domain::models::auth::Claims;

use crate::auth;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub error: Option<String>,
}

/// GET /api/auth/url
/// Returns the Google OAuth 2.0 authorization URL for staff login.
#[worker::send]
pub async fn auth_url(State(state): State<AppState>) -> Json<serde_json::Value> {
    let url = auth::get_auth_url(&state);
    Json(json!({
        "auth_url": url,
    }))
}

/// GET /api/auth/callback?code=...
/// Handles the OAuth callback:
/// 1. Exchanges the authorization code for tokens
/// 2. Fetches user info from Google
/// 3. Verifies the user is in the staff allowlist
/// 4. Creates a JWT session token (async via SubtleCrypto)
/// 5. Redirects to the staff page with the token in an HttpOnly cookie
#[worker::send]
pub async fn auth_callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> Response {
    // Check for OAuth error from Google
    if let Some(ref error) = query.error {
        tracing::warn!("oauth callback error: {error}");
        return Redirect::to("/?error=oauth_failed").into_response();
    }

    // Extract authorization code
    let Some(code) = query.code else {
        return Redirect::to("/login?error=missing_code").into_response();
    };

    // Exchange code for user info via Google APIs (uses worker::Fetch)
    let user_info = match auth::handle_callback(&code, &state).await {
        Ok(info) => info,
        Err(ref e) => {
            tracing::error!("oauth callback failed: {e}");
            return Redirect::to("/login?error=auth_failed").into_response();
        }
    };

    // Verify user is in the staff allowlist
    if !auth::is_staff(&user_info.email, &state).await {
        tracing::warn!("non-staff user attempted login: {}", user_info.email);
        return Redirect::to("/login?error=not_authorized").into_response();
    }

    // Create JWT session token (async via SubtleCrypto HMAC-SHA256)
    let token =
        match auth::create_session_jwt(&user_info.email, &user_info.id, &state.config.jwt_secret)
            .await
        {
            Ok(token) => token,
            Err(ref e) => {
                tracing::error!("jwt creation failed: {e}");
                return Redirect::to("/login?error=token_failed").into_response();
            }
        };

    tracing::info!("staff login successful: {}", user_info.email);

    // Set HttpOnly cookie for browser-based auth. The frontend calls GET /api/auth/me
    // which reads the JWT from this cookie (no localStorage or URL token passing needed).
    // Cookie is scoped to /api so it's only sent on API requests.
    let http_only_cookie = format!(
        "event_checkin_token={token}; HttpOnly; Secure; SameSite=Lax; Path=/api; Max-Age=86400"
    );

    let redirect_url = "/staff".to_string();
    let mut response = Redirect::to(&redirect_url).into_response();

    if let Ok(cookie_value) = HeaderValue::from_str(&http_only_cookie) {
        response
            .headers_mut()
            .insert(header::SET_COOKIE, cookie_value);
    }

    response
}

/// GET /api/auth/me
/// Returns the current authenticated user's info from their JWT claims.
/// Requires valid JWT in the Authorization header or cookie (enforced by middleware).
///
/// Returns the user's role using the Phase 4 hierarchy:
/// - "super_admin" — global admin (from SUPER_ADMIN_EMAILS env var)
/// - "organizer"  — event manager (from Google Sheets staff tab role "admin")
/// - "staff"      — scanner only (from Google Sheets staff tab or STAFF_EMAILS env var)
#[worker::send]
pub async fn auth_me(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Json<serde_json::Value> {
    let role = if state
        .config
        .super_admin_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(&claims.email))
    {
        "super_admin".to_string()
    } else {
        match auth::get_staff_role(&claims.email, &state).await.as_deref() {
            Some("admin" | "organizer") => "organizer".to_string(),
            Some("staff") => "staff".to_string(),
            // Per-event organizer (not in global sources) → report as organizer
            None if auth::is_event_organizer_any(&claims.email, &state).await => {
                "organizer".to_string()
            }
            // Any other role (including per-event staff) → staff
            Some(_) | None => "staff".to_string(),
        }
    };

    Json(json!({
        "email": claims.email,
        "sub": claims.sub,
        "role": role,
    }))
}

/// GET /api/auth/logout
/// Clears the session cookie and returns JSON 200 (no redirect).
/// The frontend calls this via fetch(), then navigates client-side.
/// Clears cookies at both Path=/api and Path=/ to handle stale cookies
/// from earlier development iterations.
#[worker::send]
pub async fn auth_logout() -> Response {
    let cookie_api = "event_checkin_token=; HttpOnly; Secure; SameSite=Lax; Path=/api; Max-Age=0";
    let cookie_root = "event_checkin_token=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0";

    let mut headers = axum::http::HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(cookie_api) {
        headers.append(header::SET_COOKIE, v);
    }
    if let Ok(v) = HeaderValue::from_str(cookie_root) {
        headers.append(header::SET_COOKIE, v);
    }

    (
        headers,
        Json(json!({
            "success": true,
            "message": "logged out",
        })),
    )
        .into_response()
}
