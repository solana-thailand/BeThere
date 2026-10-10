//! "Email me when the next event opens" (.plans/045 R4.12).
//!
//! `POST /api/subscribe` — public; a bot check, then an upsert into
//! `subscribers`. It answers the same whether the address was new, already
//! there or unsubscribed before, so it cannot be used to learn who is on
//! the list.
//!
//! `POST /api/unsubscribe/{token}` — public, no sign-in: the one-click link
//! in every mail (RFC 8058 posts here) and the button on `/unsubscribe/…`.
//! The same answer for a known, unknown or already used token.

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::json;

use crate::error::ApiOk;
use crate::state::AppState;
use event_checkin_domain::models::error::AppError;

/// The mail's language: the page's, when they asked.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MailLocale {
    #[default]
    En,
    Th,
}

impl MailLocale {
    pub const fn as_str(self) -> &'static str {
        match self {
            MailLocale::En => "en",
            MailLocale::Th => "th",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub email: String,
    #[serde(default)]
    pub locale: MailLocale,
}

#[worker::send]
pub async fn subscribe(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<SubscribeRequest>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let email = body.email.trim().to_lowercase();
    if !event_checkin_domain::validation::is_plausible_email(&email) {
        return Err(AppError::Validation("Invalid email address".into()).into());
    }
    crate::turnstile::require_human(&state, &headers, "subscribe").await?;
    let Some(d1) = state.d1.as_deref() else {
        return Err(AppError::Internal("subscribe needs D1".into()).into());
    };
    let token = crate::crypto::random_hex(32).map_err(AppError::Internal)?;
    crate::subscribers::db::upsert(d1, &email, body.locale.as_str(), &token)
        .await
        .map_err(AppError::Internal)?;
    tracing::info!(subscriber_fingerprint = %state.log_fingerprint(&email), "subscribed");
    Ok(ApiOk::new(json!({ "subscribed": true })))
}

#[worker::send]
pub async fn unsubscribe(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    if crate::subscribers::db::is_token(&token)
        && let Some(d1) = state.d1.as_deref()
    {
        let changed = crate::subscribers::db::unsubscribe(d1, &token)
            .await
            .map_err(AppError::Internal)?;
        tracing::info!(changed, "unsubscribe");
    }
    Ok(ApiOk::new(json!({ "unsubscribed": true })))
}
