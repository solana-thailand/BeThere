//! Auth handlers for the Cloudflare Worker.
//!
//! Mirrors `src/handlers/auth.rs` from the Axum build but uses async JWT
//! operations (SubtleCrypto via `crate::crypto`) and `worker::Fetch` (via
//! `crate::http`) instead of sync `jsonwebtoken` + `reqwest`.

use axum::{
    Extension,
    extract::{Query, State},
    http::{HeaderValue, header},
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use serde_json::json;

use event_checkin_domain::models::auth::Claims;

use crate::auth;
use crate::error::ApiOk;
use crate::state::AppState;
use event_checkin_domain::models::api::ApiResponse;

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub error: Option<String>,
    /// OAuth state parameter — used to pass redirect URL for non-staff users.
    pub state: Option<String>,
}

/// GET /api/auth/url
/// Returns the Google OAuth 2.0 authorization URL for login.
/// Accepts optional `redirect` query param passed as OAuth state.
#[derive(Debug, Deserialize)]
pub struct AuthUrlQuery {
    pub redirect: Option<String>,
}

#[worker::send]
pub async fn auth_url(
    State(state): State<AppState>,
    Query(query): Query<AuthUrlQuery>,
) -> ApiOk<serde_json::Value> {
    let url = auth::get_auth_url(&state, query.redirect.as_deref());
    ApiOk::new(json!({
        "auth_url": url,
    }))
}

/// GET /api/auth/callback?code=...
/// Handles the OAuth callback:
/// 1. Exchanges the authorization code for tokens
/// 2. Fetches user info from Google
/// 3. Creates a JWT session token for ALL users (staff and non-staff)
/// 4. Redirects admins/organizers to `/admin`, staff to `/staff`, non-staff to the `state` param (or `/`)
/// 5. Sets HttpOnly cookie with JWT
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

    // Resolve role using the same hierarchy as auth_me:
    // super_admin → organizer → staff → attendee
    let is_super_admin = state
        .config
        .super_admin_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(&user_info.email));

    let is_staff_user = auth::is_staff(&user_info.email, &state).await;

    let role = if is_super_admin {
        "super_admin"
    } else if !is_staff_user {
        "attendee"
    } else {
        match auth::get_staff_role(&user_info.email, &state)
            .await
            .as_deref()
        {
            Some("admin" | "organizer") => "organizer",
            Some("staff") => "staff",
            None if auth::is_event_organizer_any(&user_info.email, &state).await => "organizer",
            Some(_) | None => "staff",
        }
    };

    // Log user sign-in to Google Sheet (fire-and-forget — errors don't block login)
    if let Err(e) =
        crate::handlers::user_log::upsert_user_log(&user_info.email, &user_info.id, role, &state)
            .await
    {
        tracing::warn!(error = ?e, "user log upsert failed");
    }

    // Create JWT session token for ALL users (staff and non-staff)
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

    // Determine redirect: prefer explicit state param (event page redirect),
    // fall back to role-based defaults.
    let redirect_url = if let Some(ref state_url) = query.state {
        tracing::info!(
            "login successful with redirect: {} (role={role})",
            state_url,
        );
        state_url.clone()
    } else if is_staff_user {
        let dashboard = if matches!(role, "super_admin" | "organizer") {
            "/admin"
        } else {
            "/staff"
        };
        tracing::info!(
            "staff login successful: {} (role={role}, redirect={dashboard})",
            user_info.email,
        );
        dashboard.to_string()
    } else {
        tracing::info!(
            "attendee login successful: {} (role={role})",
            user_info.email,
        );
        "/".to_string()
    };

    // Set HttpOnly cookie for browser-based auth. The frontend calls GET /api/auth/me
    // which reads the JWT from this cookie (no localStorage or URL token passing needed).
    // Cookie is scoped to /api so it's only sent on API requests.
    let http_only_cookie = format!(
        "event_checkin_token={token}; HttpOnly; Secure; SameSite=Lax; Path=/api; Max-Age=86400"
    );

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
) -> ApiOk<serde_json::Value> {
    let role = if state
        .config
        .super_admin_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(&claims.email))
    {
        "super_admin".to_string()
    } else if !auth::is_staff(&claims.email, &state).await {
        // Non-staff user (attendee who signed in via Google)
        "attendee".to_string()
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

    ApiOk::new(json!({
        "email": claims.email,
        "sub": claims.sub,
        "role": role,
    }))
}

/// GET /api/auth/logout
/// Clears the session cookie, blacklists the JWT (VULN-011), and returns JSON 200.
/// The frontend calls this via fetch(), then navigates client-side.
/// Clears cookies at both Path=/api and Path=/ to handle stale cookies
/// from earlier development iterations.
#[worker::send]
pub async fn auth_logout(State(state): State<AppState>, req: axum::extract::Request) -> Response {
    // Extract and blacklist the JWT before clearing cookies
    let token = auth::extract_token_from_request(&req);
    if let Some(ref token_str) = token
        && let Ok(claims) = auth::verify_session_jwt(token_str, &state.config.jwt_secret).await
    {
        auth::blacklist_token(token_str, &claims, &state).await;
    }

    let cookie_api = "event_checkin_token=; HttpOnly; Secure; SameSite=Lax; Path=/api; Max-Age=0";
    let cookie_root = "event_checkin_token=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0";

    let mut headers = axum::http::HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(cookie_api) {
        headers.append(header::SET_COOKIE, v);
    }
    if let Ok(v) = HeaderValue::from_str(cookie_root) {
        headers.append(header::SET_COOKIE, v);
    }

    (headers, axum::Json(ApiResponse::message("logged out"))).into_response()
}
