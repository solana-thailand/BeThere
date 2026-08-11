//! Social account linking handlers.
//!
//! Allows authenticated attendees to link verified social accounts to their
//! developer profile. Supported platforms: GitHub (OAuth), Telegram (Login Widget).
//!
//! GitHub flow:
//!   GET  /api/auth/github           — redirect to GitHub OAuth (auth-guarded)
//!   GET  /api/auth/github/callback  — exchange code, save handle, redirect to /profile
//!
//! Telegram flow:
//!   POST /api/auth/telegram/verify  — verify Login Widget HMAC, save handle (auth-guarded)
//!
//! Unlink:
//!   POST /api/auth/social/unlink    — remove a verified social link (auth-guarded)

use axum::{
    Extension,
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use serde_json::json;

use crate::error::{ApiOk, WorkerError};
use crate::state::AppState;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

// ---------------------------------------------------------------------------
// GitHub OAuth — Link GitHub account
// ---------------------------------------------------------------------------

/// GET /api/auth/github
/// Redirects authenticated user to GitHub OAuth authorization URL.
#[worker::send]
pub async fn github_link_start(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let client_id = &state.config.github_client_id;
    if client_id.is_empty() {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(json!({"error": "GitHub OAuth not configured — set GITHUB_CLIENT_ID secret"})),
        )
            .into_response();
    }

    let raw_redirect_uri = if !state.config.github_redirect_uri.is_empty() {
        state.config.github_redirect_uri.clone()
    } else {
        format!("{}/api/auth/github/callback", state.config.server.url)
    };

    // Encode the user's email in a signed OAuth state param so we know who to
    // update on callback — the HMAC stops third parties from linking their
    // GitHub account to an arbitrary victim email.
    let expires = (js_sys::Date::now() / 1000.0) as i64 + GITHUB_STATE_TTL_SECS;
    let signed_state = match sign_github_state(&claims.email, expires, &state.config.jwt_secret).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("GitHub state signing failed: {e}");
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(json!({"error": "failed to sign OAuth state"})),
            )
                .into_response();
        }
    };
    let encoded_state = urlencoding::encode(&signed_state).to_string();
    let redirect_uri = urlencoding::encode(&raw_redirect_uri).to_string();

    let github_auth_url = format!(
        "https://github.com/login/oauth/authorize\
         ?client_id={client_id}\
         &redirect_uri={redirect_uri}\
         &scope=read:user\
         &state={encoded_state}"
    );

    tracing::info!(email = %claims.email, "GitHub OAuth link started");
    Redirect::to(&github_auth_url).into_response()
}

/// How long a social-link state param stays valid.
const GITHUB_STATE_TTL_SECS: i64 = 600;

/// Build a signed state param `{email}|{expires_unix}|{hmac_hex}`, domain-
/// separated by `tag`. The HMAC (keyed with the JWT secret) covers email +
/// expiry, so a callback can trust the email WITHOUT a session cookie — which
/// matters for third-party redirect flows (GitHub OAuth, Telegram widget)
/// where SameSite cookies may not survive the cross-site hop.
async fn sign_link_state(
    tag: &str,
    email: &str,
    expires: i64,
    secret: &str,
) -> Result<String, String> {
    let payload = format!("{email}|{expires}");
    let sig = crate::crypto::hmac_sha256(
        secret.as_bytes(),
        format!("{tag}|{payload}").as_bytes(),
    )
    .await?;
    let sig_hex: String = sig.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("{payload}|{sig_hex}"))
}

/// Verify a signed state param for `tag` and return the embedded email.
async fn verify_link_state(tag: &str, state: &str, secret: &str) -> Result<String, &'static str> {
    // rsplitn: sig and expiry cannot contain '|', the email theoretically could
    let mut parts = state.rsplitn(3, '|');
    let _sig_hex = parts.next().ok_or("malformed")?;
    let expires_str = parts.next().ok_or("malformed")?;
    let email = parts.next().ok_or("malformed")?;

    let expires: i64 = expires_str.parse().map_err(|_| "malformed")?;
    let now = (js_sys::Date::now() / 1000.0) as i64;
    if now > expires {
        return Err("expired");
    }

    let expected = sign_link_state(tag, email, expires, secret)
        .await
        .map_err(|_| "hmac_failed")?;
    // Constant-time-ish comparison: compare full signed strings byte-wise
    let matches = expected.len() == state.len()
        && expected
            .bytes()
            .zip(state.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0;
    if !matches {
        return Err("bad_signature");
    }
    Ok(email.to_string())
}

async fn sign_github_state(email: &str, expires: i64, secret: &str) -> Result<String, String> {
    sign_link_state("github-link", email, expires, secret).await
}

async fn verify_github_state(state: &str, secret: &str) -> Result<String, &'static str> {
    verify_link_state("github-link", state, secret).await
}

#[derive(Debug, Deserialize)]
pub struct GithubCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>, // signed: email|expires|hmac
    pub error: Option<String>,
}

/// GitHub user info response from GET /user.
#[derive(Deserialize)]
struct GithubUserInfo {
    login: String,
}

/// Dedicated GitHub OAuth token exchange with explicit Accept: application/json and User-Agent headers.
async fn exchange_github_code(
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<String, String> {
    let url = "https://github.com/login/oauth/access_token";
    let token_body = serde_json::json!({
        "client_id": client_id,
        "client_secret": client_secret,
        "code": code,
        "redirect_uri": redirect_uri,
    });
    let json_body = serde_json::to_string(&token_body)
        .map_err(|e| format!("failed to serialize GitHub token request: {e}"))?;

    let headers = worker::Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("failed to set content-type: {e:?}"))?;
    headers
        .set("Accept", "application/json")
        .map_err(|e| format!("failed to set accept: {e:?}"))?;
    headers
        .set("User-Agent", "BeThere-App/1.0")
        .map_err(|e| format!("failed to set user-agent: {e:?}"))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = worker::Request::new_with_init(url, &init)
        .map_err(|e| format!("failed to create request to {url}: {e:?}"))?;

    let mut response = worker::Fetch::Request(request)
        .send()
        .await
        .map_err(|e| format!("POST {url} failed: {e:?}"))?;

    let text = response
        .text()
        .await
        .map_err(|e| format!("failed to read response text from {url}: {e:?}"))?;

    #[derive(Deserialize)]
    struct GithubTokenRes {
        access_token: Option<String>,
        error: Option<String>,
        error_description: Option<String>,
    }

    let parsed: GithubTokenRes = serde_json::from_str(&text)
        .map_err(|e| format!("failed to parse GitHub JSON response '{text}': {e}"))?;

    if let Some(err) = parsed.error {
        let desc = parsed.error_description.unwrap_or_default();
        return Err(format!("GitHub OAuth returned error: {err} ({desc})"));
    }

    parsed
        .access_token
        .filter(|t| !t.is_empty())
        .ok_or_else(|| format!("no access token in GitHub response '{text}'"))
}

/// GET /api/auth/github/callback?code=...&state=<encoded_email>
/// Exchanges the code for an access token, fetches the GitHub username,
/// and saves it (with verified=1) to the developer profile.
///
/// Note: This endpoint is PUBLIC (no auth middleware) because GitHub redirects
/// back here from an external domain. The email is recovered from the OAuth state param.
#[worker::send]
pub async fn github_link_callback(
    State(state): State<AppState>,
    Query(query): Query<GithubCallbackQuery>,
) -> Response {
    if let Some(ref err) = query.error {
        tracing::warn!("GitHub OAuth callback error: {err}");
        return Redirect::to("/profile?error=github_denied").into_response();
    }

    let code = match query.code {
        Some(c) => c,
        None => return Redirect::to("/profile?error=github_no_code").into_response(),
    };

    // Recover and verify the signed state param
    let raw_state = match query.state {
        Some(ref s) => urlencoding::decode(s)
            .map(|c| c.into_owned())
            .unwrap_or_default(),
        None => return Redirect::to("/profile?error=github_no_state").into_response(),
    };

    let email = match verify_github_state(&raw_state, &state.config.jwt_secret).await {
        Ok(email) => email,
        Err("expired") => {
            tracing::warn!("GitHub OAuth state expired");
            return Redirect::to("/profile?error=github_state_expired").into_response();
        }
        Err(reason) => {
            tracing::warn!("GitHub OAuth state rejected: {reason}");
            return Redirect::to("/profile?error=github_invalid_state").into_response();
        }
    };

    if email.is_empty() {
        return Redirect::to("/profile?error=github_invalid_state").into_response();
    }

    let raw_redirect_uri = if !state.config.github_redirect_uri.is_empty() {
        state.config.github_redirect_uri.clone()
    } else {
        format!("{}/api/auth/github/callback", state.config.server.url)
    };

    // Exchange code for access token using dedicated token exchange
    let access_token = match exchange_github_code(
        &state.config.github_client_id,
        &state.config.github_client_secret,
        &code,
        &raw_redirect_uri,
    )
    .await
    {
        Ok(token) => token,
        Err(e) => {
            tracing::error!("GitHub token exchange failed: {e}");
            return Redirect::to("/profile?error=github_token_failed").into_response();
        }
    };

    // Fetch GitHub user info — requires User-Agent header per GitHub API policy
    let user_info: GithubUserInfo = match github_get_user(&access_token).await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("GitHub user fetch failed: {e}");
            return Redirect::to("/profile?error=github_user_failed").into_response();
        }
    };

    // Save verified GitHub handle to developer profile
    let d1 = match state.d1.as_ref() {
        Some(d) => d,
        None => return Redirect::to("/profile?error=db_unavailable").into_response(),
    };

    let handle = user_info.login.replace('\'', "''");
    let email_escaped = email.replace('\'', "''");
    let sql = format!(
        "INSERT INTO developer_profiles \
         (email, github_handle, github_verified, github_verified_at, \
          first_seen_at, last_active_at, total_events, updated_at) \
         VALUES ('{email_escaped}', '{handle}', 1, datetime('now'), \
                 datetime('now'), datetime('now'), 0, datetime('now')) \
         ON CONFLICT (email) DO UPDATE SET \
          github_handle = '{handle}', \
          github_verified = 1, \
          github_verified_at = datetime('now'), \
          updated_at = datetime('now')"
    );

    if let Err(e) = worker::D1Database::prepare(d1, &sql).run().await {
        tracing::error!("GitHub handle save failed: {e:?}");
        return Redirect::to("/profile?error=github_save_failed").into_response();
    }

    tracing::info!(email = %email, github = %user_info.login, "GitHub account linked successfully");
    Redirect::to("/profile?linked=github").into_response()
}

/// GET https://api.github.com/user with Bearer token.
///
/// GitHub API requires `User-Agent` header; use `worker::Fetch` directly.
async fn github_get_user(access_token: &str) -> Result<GithubUserInfo, String> {
    use worker::{Fetch, Headers, Method, Request, RequestInit};

    let headers = Headers::new();
    headers
        .set("Authorization", &format!("Bearer {access_token}"))
        .map_err(|e| format!("header error: {e:?}"))?;
    headers
        .set("Accept", "application/vnd.github+json")
        .map_err(|e| format!("header error: {e:?}"))?;
    headers
        .set("User-Agent", "BeThere-Protocol/1.0")
        .map_err(|e| format!("header error: {e:?}"))?;

    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);

    let request = Request::new_with_init("https://api.github.com/user", &init)
        .map_err(|e| format!("request error: {e:?}"))?;

    let mut response = Fetch::Request(request)
        .send()
        .await
        .map_err(|e| format!("fetch error: {e:?}"))?;

    if response.status_code() != 200 {
        return Err(format!("GitHub API returned status {}", response.status_code()));
    }

    response
        .json()
        .await
        .map_err(|e| format!("parse error: {e:?}"))
}

// ---------------------------------------------------------------------------
// Telegram Login Widget — Link Telegram account
// ---------------------------------------------------------------------------

/// Telegram Login Widget data sent from the browser.
#[derive(Debug, Deserialize)]
pub struct TelegramVerifyRequest {
    pub id: i64,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub photo_url: Option<String>,
    pub auth_date: i64,
    pub hash: String,
}

/// GET /api/auth/telegram/config
/// Public. Tells the frontend whether the Telegram Login Widget can be shown
/// and which bot to render it for. Never exposes the bot token.
#[worker::send]
pub async fn telegram_config(State(state): State<AppState>) -> Response {
    let username = state.config.telegram_bot_username.trim();
    // Require BOTH the username (for the widget) and the token (for verification)
    // before advertising the widget as usable.
    let configured = !username.is_empty() && !state.config.telegram_bot_token.is_empty();
    (
        axum::http::StatusCode::OK,
        axum::Json(json!({
            "configured": configured,
            "bot_username": if configured { username } else { "" },
        })),
    )
        .into_response()
}

/// GET /api/auth/telegram/state
/// Auth-guarded. Returns a signed state token binding the current user's email,
/// which the frontend embeds in the widget's `data-auth-url`. The redirect
/// callback recovers the email from this token, so it needs no session cookie.
#[worker::send]
pub async fn telegram_state(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let expires = (js_sys::Date::now() / 1000.0) as i64 + GITHUB_STATE_TTL_SECS;
    match sign_link_state("telegram-link", &claims.email, expires, &state.config.jwt_secret).await {
        Ok(signed) => (
            axum::http::StatusCode::OK,
            axum::Json(json!({ "state": signed })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("telegram state signing failed: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(json!({"error": "failed to sign state"})),
            )
                .into_response()
        }
    }
}

/// POST /api/auth/telegram/verify
/// Receives Telegram Login Widget data from the browser.
/// Verifies the HMAC-SHA256 signature using the bot token, then saves
/// the Telegram handle and ID to the developer profile.
#[worker::send]
pub async fn telegram_verify(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    axum::Json(body): axum::Json<TelegramVerifyRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let bot_token = &state.config.telegram_bot_token;
    if bot_token.is_empty() {
        return Err(WorkerError(AppError::Validation(
            "Telegram not configured — set TELEGRAM_BOT_TOKEN secret".to_string(),
        )));
    }

    // Verify HMAC-SHA256 signature per Telegram Login Widget spec using SubtleCrypto
    let is_valid = verify_telegram_hash_subtle(&body, bot_token).await;
    if !is_valid {
        tracing::warn!(email = %claims.email, "Telegram HMAC verification failed");
        return Err(WorkerError(AppError::Validation(
            "Invalid Telegram signature".to_string(),
        )));
    }

    // Replay attack prevention: auth_date must be within 24 hours
    let now = (js_sys::Date::now() / 1000.0) as i64;
    if now - body.auth_date > 86_400 {
        return Err(WorkerError(AppError::Validation(
            "Telegram auth_date too old — please re-authenticate".to_string(),
        )));
    }

    let d1 = state
        .d1
        .as_ref()
        .ok_or_else(|| WorkerError(AppError::Internal("D1 not available".to_string())))?;

    let telegram_handle = body.username.clone().unwrap_or_else(|| body.first_name.clone());
    let telegram_id = body.id.to_string();
    let email_escaped = claims.email.replace('\'', "''");
    let handle_escaped = telegram_handle.replace('\'', "''");
    let id_escaped = telegram_id.replace('\'', "''");

    let sql = format!(
        "INSERT INTO developer_profiles \
         (email, telegram_handle, telegram_id, telegram_verified, telegram_verified_at, \
          first_seen_at, last_active_at, total_events, updated_at) \
         VALUES ('{email_escaped}', '{handle_escaped}', '{id_escaped}', 1, datetime('now'), \
                 datetime('now'), datetime('now'), 0, datetime('now')) \
         ON CONFLICT (email) DO UPDATE SET \
          telegram_handle = '{handle_escaped}', \
          telegram_id = '{id_escaped}', \
          telegram_verified = 1, \
          telegram_verified_at = datetime('now'), \
          updated_at = datetime('now')"
    );

    worker::D1Database::prepare(d1, &sql)
        .run()
        .await
        .map_err(|e| WorkerError(AppError::Internal(format!("Telegram save failed: {e:?}"))))?;

    tracing::info!(
        email = %claims.email,
        telegram_id = %body.id,
        username = ?body.username,
        "Telegram account linked successfully"
    );

    Ok(ApiOk::new(json!({
        "status": "linked",
        "telegram_handle": telegram_handle,
        "telegram_id": body.id,
    })))
}

/// Save a verified Telegram link to the developer profile (shared by the POST
/// verify path and the GET redirect-callback path).
async fn save_telegram_link(
    d1: &worker::D1Database,
    email: &str,
    handle: &str,
    telegram_id: &str,
) -> Result<(), String> {
    let email_escaped = email.replace('\'', "''");
    let handle_escaped = handle.replace('\'', "''");
    let id_escaped = telegram_id.replace('\'', "''");
    let sql = format!(
        "INSERT INTO developer_profiles \
         (email, telegram_handle, telegram_id, telegram_verified, telegram_verified_at, \
          first_seen_at, last_active_at, total_events, updated_at) \
         VALUES ('{email_escaped}', '{handle_escaped}', '{id_escaped}', 1, datetime('now'), \
                 datetime('now'), datetime('now'), 0, datetime('now')) \
         ON CONFLICT (email) DO UPDATE SET \
          telegram_handle = '{handle_escaped}', \
          telegram_id = '{id_escaped}', \
          telegram_verified = 1, \
          telegram_verified_at = datetime('now'), \
          updated_at = datetime('now')"
    );
    worker::D1Database::prepare(d1, &sql)
        .run()
        .await
        .map(|_| ())
        .map_err(|e| format!("Telegram save failed: {e:?}"))
}

/// Query params Telegram appends to the Login Widget `data-auth-url` callback,
/// plus our own signed `state` (carried through by Telegram since it preserves
/// existing query params on the auth URL).
#[derive(Debug, Deserialize)]
pub struct TelegramCallbackQuery {
    pub state: Option<String>, // our signed email token
    pub id: Option<i64>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub photo_url: Option<String>,
    pub auth_date: Option<i64>,
    pub hash: Option<String>,
}

/// GET /api/auth/telegram/callback
/// PUBLIC redirect-flow endpoint for the Telegram Login Widget (`data-auth-url`).
/// Used instead of the JS `data-onauth` callback because that path requires
/// `eval()`, which our CSP forbids. Identity comes from our signed `state`
/// param (not a cookie), so it survives the cross-site redirect even when the
/// browser drops SameSite cookies on the hop back.
#[worker::send]
pub async fn telegram_callback(
    State(state): State<AppState>,
    Query(q): Query<TelegramCallbackQuery>,
) -> Response {
    let bot_token = &state.config.telegram_bot_token;
    if bot_token.is_empty() {
        return Redirect::to("/profile?error=telegram_unconfigured").into_response();
    }

    // Recover the linking user's email from our signed state.
    let raw_state = q.state.clone().unwrap_or_default();
    let email = match verify_link_state("telegram-link", &raw_state, &state.config.jwt_secret).await {
        Ok(email) => email,
        Err("expired") => return Redirect::to("/profile?error=telegram_expired").into_response(),
        Err(_) => return Redirect::to("/profile?error=telegram_invalid").into_response(),
    };

    let (id, first_name, auth_date, hash) =
        match (q.id, q.first_name.clone(), q.auth_date, q.hash.clone()) {
            (Some(i), Some(f), Some(a), Some(h)) => (i, f, a, h),
            _ => return Redirect::to("/profile?error=telegram_invalid").into_response(),
        };

    let data = TelegramVerifyRequest {
        id,
        first_name,
        last_name: q.last_name.clone(),
        username: q.username.clone(),
        photo_url: q.photo_url.clone(),
        auth_date,
        hash,
    };

    if !verify_telegram_hash_subtle(&data, bot_token).await {
        tracing::warn!(email = %email, "Telegram callback HMAC verification failed");
        return Redirect::to("/profile?error=telegram_bad_signature").into_response();
    }

    let now = (js_sys::Date::now() / 1000.0) as i64;
    if now - data.auth_date > 86_400 {
        return Redirect::to("/profile?error=telegram_expired").into_response();
    }

    let d1 = match state.d1.as_ref() {
        Some(d) => d,
        None => return Redirect::to("/profile?error=db_unavailable").into_response(),
    };

    let handle = data.username.clone().unwrap_or_else(|| data.first_name.clone());
    if let Err(e) = save_telegram_link(d1, &email, &handle, &data.id.to_string()).await {
        tracing::error!("Telegram callback save failed: {e}");
        return Redirect::to("/profile?error=telegram_save_failed").into_response();
    }

    tracing::info!(email = %email, telegram_id = %data.id, "Telegram linked via redirect callback");
    Redirect::to("/profile?linked=telegram").into_response()
}

// ---------------------------------------------------------------------------
// Unlink
// ---------------------------------------------------------------------------

/// POST /api/auth/social/unlink
/// Removes a verified social account link from the developer profile.
#[derive(Debug, Deserialize)]
pub struct UnlinkRequest {
    pub platform: String, // "github" | "telegram" | "discord"
}

#[worker::send]
pub async fn social_unlink(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    axum::Json(body): axum::Json<UnlinkRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = state
        .d1
        .as_ref()
        .ok_or_else(|| WorkerError(AppError::Internal("D1 not available".to_string())))?;

    let email_escaped = claims.email.replace('\'', "''");

    let sql = match body.platform.as_str() {
        "github" => format!(
            "UPDATE developer_profiles SET \
             github_handle = NULL, github_verified = 0, github_verified_at = NULL, \
             updated_at = datetime('now') \
             WHERE email = '{email_escaped}'"
        ),
        "telegram" => format!(
            "UPDATE developer_profiles SET \
             telegram_handle = NULL, telegram_id = NULL, telegram_verified = 0, \
             telegram_verified_at = NULL, updated_at = datetime('now') \
             WHERE email = '{email_escaped}'"
        ),
        "discord" => format!(
            "UPDATE developer_profiles SET \
             discord_handle = NULL, discord_verified = 0, discord_verified_at = NULL, \
             updated_at = datetime('now') \
             WHERE email = '{email_escaped}'"
        ),
        other => {
            return Err(WorkerError(AppError::Validation(format!(
                "Unknown platform: {other}"
            ))))
        }
    };

    worker::D1Database::prepare(d1, &sql)
        .run()
        .await
        .map_err(|e| WorkerError(AppError::Internal(format!("Unlink failed: {e:?}"))))?;

    tracing::info!(email = %claims.email, platform = %body.platform, "Social account unlinked");

    Ok(ApiOk::new(json!({
        "status": "unlinked",
        "platform": body.platform,
    })))
}

// ---------------------------------------------------------------------------
// Helpers — Telegram HMAC verification via SubtleCrypto (Web Crypto API)
// ---------------------------------------------------------------------------

/// Verify the Telegram Login Widget HMAC-SHA256 signature using SubtleCrypto.
///
/// Per Telegram docs:
/// 1. Build data-check-string: sorted `key=value` pairs (all fields except `hash`), joined by `\n`
/// 2. Secret key = HMAC-SHA256("WebAppData", bot_token) — but for Login Widget it's SHA256(bot_token)
/// 3. Verify: HMAC-SHA256(secret_key, data_check_string) == hash (hex-encoded)
///
/// Note: Uses WebCrypto SubtleCrypto because pure-Rust HMAC crates add WASM binary size.
async fn verify_telegram_hash_subtle(data: &TelegramVerifyRequest, bot_token: &str) -> bool {
    // Build sorted data-check-string
    let mut pairs: Vec<String> = vec![
        format!("auth_date={}", data.auth_date),
        format!("first_name={}", data.first_name),
        format!("id={}", data.id),
    ];
    if let Some(ref ln) = data.last_name {
        pairs.push(format!("last_name={ln}"));
    }
    if let Some(ref ph) = data.photo_url {
        pairs.push(format!("photo_url={ph}"));
    }
    if let Some(ref un) = data.username {
        pairs.push(format!("username={un}"));
    }
    pairs.sort();
    let check_string = pairs.join("\n");

    // Use SubtleCrypto for HMAC: key = SHA256(bot_token), then HMAC-SHA256(key, check_string)
    match compute_telegram_hmac(bot_token, &check_string).await {
        Ok(computed_hex) => computed_hex == data.hash,
        Err(e) => {
            tracing::error!("Telegram HMAC computation failed: {e}");
            false
        }
    }
}

/// Compute HMAC-SHA256 of `check_string` using SHA256(bot_token) as the key.
async fn compute_telegram_hmac(bot_token: &str, check_string: &str) -> Result<String, String> {
    // Step 1: secret_key = SHA-256(bot_token)
    let secret_key = crate::crypto::sha256_digest(bot_token.as_bytes()).await?;

    // Step 2: signature = HMAC-SHA256(secret_key, check_string)
    let sig_bytes = crate::crypto::hmac_sha256(&secret_key, check_string.as_bytes()).await?;

    // Step 3: Hex-encode
    Ok(sig_bytes.iter().map(|b| format!("{b:02x}")).collect())
}
