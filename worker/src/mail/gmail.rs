//! The Gmail API transport: a refresh token (scope `gmail.send` only) turned
//! into an access token per run, then one `users.messages.send` per mail.
//! Secrets: `GMAIL_CLIENT_ID`, `GMAIL_CLIENT_SECRET`, `GMAIL_REFRESH_TOKEN`;
//! var `MAIL_FROM` (the account the token belongs to). Setup:
//! `docs/gmail_sender_setup.md`.

use serde::Deserialize;
use worker::{Env, Fetch, Headers, Method, Request, RequestInit};

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const SEND_URL: &str = "https://gmail.googleapis.com/gmail/v1/users/me/messages/send";

/// How a send ended. Only a refusal is safe to try again: after a network
/// error the mail may have gone, so it is never replayed automatically.
#[derive(Debug, PartialEq, Eq)]
pub enum SendError {
    Refused(u16),
    Ambiguous(String),
}

pub struct Gmail {
    pub from: String,
    access_token: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

impl Gmail {
    /// `None` when the secrets are not set (the announcer then sends nothing
    /// and says so in the log).
    pub async fn connect(env: &Env) -> Result<Option<Self>, String> {
        let secret = |name: &str| {
            env.secret(name)
                .ok()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
        };
        let (Some(client_id), Some(client_secret), Some(refresh)) = (
            secret("GMAIL_CLIENT_ID"),
            secret("GMAIL_CLIENT_SECRET"),
            secret("GMAIL_REFRESH_TOKEN"),
        ) else {
            return Ok(None);
        };
        let from = env
            .var("MAIL_FROM")
            .map(|v| v.to_string())
            .map_err(|_| "MAIL_FROM missing".to_string())?;
        if !event_checkin_domain::validation::is_plausible_email(&from) {
            return Err("MAIL_FROM invalid".into());
        }
        let token: TokenResponse = crate::http::post_form(
            TOKEN_URL,
            &[
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("refresh_token", refresh.as_str()),
                ("grant_type", "refresh_token"),
            ],
        )
        .await
        // The provider's error body can echo the client id; keep it out of logs.
        .map_err(|_| "gmail token refresh failed".to_string())?;
        Ok(Some(Self {
            from,
            access_token: token.access_token,
        }))
    }

    /// Send one message (`raw` from [`super::message::gmail_raw`]).
    pub async fn send(&self, raw: &str) -> Result<(), SendError> {
        let headers = Headers::new();
        let auth = format!("Bearer {}", self.access_token);
        headers
            .set("Authorization", &auth)
            .and_then(|_| headers.set("Content-Type", "application/json"))
            .map_err(|e| SendError::Ambiguous(format!("headers: {e}")))?;
        let body = serde_json::json!({ "raw": raw }).to_string();
        let mut init = RequestInit::new();
        init.with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(body.into()));
        let request = Request::new_with_init(SEND_URL, &init)
            .map_err(|e| SendError::Ambiguous(format!("request: {e}")))?;
        let response = Fetch::Request(request)
            .send()
            .await
            .map_err(|e| SendError::Ambiguous(format!("fetch: {e}")))?;
        match response.status_code() {
            200..=299 => Ok(()),
            // A 5xx may still have queued it; only a 4xx is a clear refusal.
            code @ 400..=499 => Err(SendError::Refused(code)),
            code => Err(SendError::Ambiguous(format!("status {code}"))),
        }
    }
}
