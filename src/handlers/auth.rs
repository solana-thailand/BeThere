use axum::{
    Extension,
    extract::{Query, Request, State},
    middleware::Next,
    response::{IntoResponse, Json, Redirect},
};
use serde::Deserialize;
use serde_json::json;

use crate::auth::{create_jwt, get_auth_url, handle_callback, is_staff, verify_jwt};
use crate::config::AppState;
use crate::models::auth::Claims;

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub error: Option<String>,
}

/// GET /api/auth/url
/// Returns the Google OAuth 2.0 authorization URL for staff login.
pub async fn auth_url(State(state): State<AppState>) -> Json<serde_json::Value> {
    let url = get_auth_url(&state);
    Json(json!({
        "auth_url": url,
    }))
}

/// GET /api/auth/callback?code=...
/// Handles the OAuth callback:
/// 1. Exchanges the authorization code for tokens
/// 2. Fetches user info from Google
/// 3. Verifies the user is in the staff allowlist
/// 4. Creates a JWT session token
/// 5. Redirects to the staff page with the token
pub async fn auth_callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> impl IntoResponse {
    // Check for OAuth error from Google
    if let Some(ref error) = query.error {
        tracing::warn!("oauth callback error: {error}");
        return Redirect::to(&format!("/?error=oauth_failed&message={error}")).into_response();
    }

    // Extract authorization code
    let code = match query.code {
        Some(code) => code,
        None => {
            return Redirect::to("/?error=missing_code").into_response();
        }
    };

    // Exchange code for user info via Google APIs
    let user_info = match handle_callback(&code, &state).await {
        Ok(info) => info,
        Err(ref e) => {
            tracing::error!("oauth callback failed: {e}");
            return Redirect::to(&format!(
                "/?error=auth_failed&message={}",
                urlencoding::encode(e)
            ))
            .into_response();
        }
    };

    // Verify user is in the staff allowlist
    if !is_staff(&user_info.email, &state) {
        tracing::warn!("non-staff user attempted login: {}", user_info.email);
        return Redirect::to("/?error=not_authorized").into_response();
    }

    // Create JWT session token
    let token = match create_jwt(&user_info.email, &user_info.id, &state.config.jwt_secret) {
        Ok(token) => token,
        Err(ref e) => {
            tracing::error!("jwt creation failed: {e}");
            return Redirect::to("/?error=token_failed").into_response();
        }
    };

    tracing::info!("staff login successful: {}", user_info.email);

    // Set HTTP-only cookie and redirect to staff page (no token in URL)
    let cookie =
        format!("event_checkin_token={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age=86400");
    let mut response = Redirect::to("/staff.html").into_response();
    if let Ok(cookie_value) = axum::http::HeaderValue::from_str(&cookie) {
        response
            .headers_mut()
            .insert(axum::http::header::SET_COOKIE, cookie_value);
    }
    response
}

/// GET /api/auth/me
/// Returns the current authenticated user's info from their JWT claims.
/// Requires valid JWT in the Authorization header (enforced by middleware).
pub async fn auth_me(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Json<serde_json::Value> {
    Json(json!({
        "email": claims.email,
        "sub": claims.sub,
        "is_staff": is_staff(&claims.email, &state),
    }))
}

/// GET /api/auth/logout
/// Clears the session cookie and redirects to the login page.
pub async fn auth_logout() -> impl IntoResponse {
    let cookie = "event_checkin_token=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0";
    let mut response = Redirect::to("/").into_response();
    if let Ok(cookie_value) = axum::http::HeaderValue::from_str(cookie) {
        response
            .headers_mut()
            .insert(axum::http::header::SET_COOKIE, cookie_value);
    }
    response
}

/// Auth middleware that extracts and verifies JWT from the Authorization header or cookie.
/// Injects the Claims into request extensions for downstream handlers.
/// Public routes (health, auth/url, auth/callback, auth/logout) are skipped.
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
    let claims = match verify_token(&token, &state) {
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

    // Verify staff status
    if !is_staff(&claims.email, &state) {
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

/// Extract JWT from Authorization header or cookie.
fn extract_token_from_request(req: &Request) -> Option<String> {
    // Try Authorization header first (for API clients)
    if let Some(auth_header) = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        && let Some(token) = auth_header.strip_prefix("Bearer ") {
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
fn verify_token(token: &Option<String>, state: &AppState) -> Result<Claims, String> {
    let token = token.as_ref().ok_or("missing authentication token")?;
    verify_jwt(token, &state.config.jwt_secret).map_err(|e| format!("invalid token: {e}"))
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
