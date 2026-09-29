//! Cloudflare Turnstile gate for the scriptable public writes (`.issues/170`).
//!
//! Covers `POST /api/waitlist` (anonymous, appends to a sheet) and
//! `POST /api/public/register` (JWT-gated, but an OAuth account is cheap to
//! script). Staff walk-in is not gated: the caller is authenticated staff, and
//! a challenge at the door is friction at the worst moment.
//!
//! The gate is off until BOTH `TURNSTILE_SITE_KEY` and `TURNSTILE_SECRET_KEY`
//! are set, so deploying this code before the widget exists changes nothing.
//! `DEV_MODE` also turns it off: dev-token e2e scripts have no browser to
//! solve a challenge. When on, it fails closed: a siteverify outage is a 502,
//! not a pass.

use axum::http::HeaderMap;
use serde::Deserialize;

use event_checkin_domain::models::error::AppError;
use event_checkin_domain::turnstile::{REJECTED, TOKEN_HEADER};

use crate::state::AppState;

const SITEVERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
/// Turnstile tokens are at most 2048 characters; anything longer is not one.
const MAX_TOKEN_LEN: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    Off,
    Enforced,
}

/// Whether the check runs for this deployment.
pub fn gate(site_key: &str, secret_key: &str, dev_mode: bool) -> Gate {
    match (
        site_key.trim().is_empty() || secret_key.trim().is_empty(),
        dev_mode,
    ) {
        (false, false) => Gate::Enforced,
        _ => Gate::Off,
    }
}

/// The token from the request header, if it is shaped like one.
pub fn token_from(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(TOKEN_HEADER)?
        .to_str()
        .ok()
        .map(str::trim)
        .filter(|token| !token.is_empty() && token.len() <= MAX_TOKEN_LEN)
}

/// The fields of the siteverify response this gate reads.
#[derive(Debug, Deserialize)]
pub struct SiteverifyResponse {
    pub success: bool,
    #[serde(default, rename = "error-codes")]
    pub error_codes: Vec<String>,
}

/// Siteverify's answer as a pass, or the error codes to log.
pub fn verdict(response: &SiteverifyResponse) -> Result<(), String> {
    match response.success {
        true => Ok(()),
        false => Err(response.error_codes.join(",")),
    }
}

fn rejected() -> AppError {
    AppError::Forbidden(REJECTED.to_string())
}

/// Reject the request unless it carries a token that siteverify accepts.
pub async fn require_human(
    state: &AppState,
    headers: &HeaderMap,
    route: &'static str,
) -> Result<(), AppError> {
    let config = &state.config;
    if gate(
        &config.turnstile_site_key,
        &config.turnstile_secret_key,
        config.dev_mode,
    ) == Gate::Off
    {
        return Ok(());
    }
    let Some(token) = token_from(headers) else {
        tracing::info!(route, reason = "missing-token", "turnstile rejected");
        return Err(rejected());
    };
    let mut form = vec![
        ("secret", config.turnstile_secret_key.as_str()),
        ("response", token),
    ];
    if let Some(ip) = headers
        .get("cf-connecting-ip")
        .and_then(|value| value.to_str().ok())
    {
        form.push(("remoteip", ip));
    }
    let response: SiteverifyResponse = crate::http::post_form(SITEVERIFY_URL, &form)
        .await
        .map_err(|error| AppError::External {
            service: "turnstile".to_string(),
            status: 502,
            body: error,
        })?;
    verdict(&response).map_err(|codes| {
        tracing::info!(route, reason = %codes, "turnstile rejected");
        rejected()
    })
}
