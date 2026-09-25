//! Add another email to the signed-in person (plan 025, issue #122).
//!
//! Proof of both emails, in one browser:
//!
//! 1. `GET /api/auth/email-link` (authed) needs a **Google-verified** session
//!    for email A. It signs `A` into an OAuth state and sends the browser to
//!    Google's account chooser.
//! 2. Google redirects to the normal `/api/auth/callback` with a verified email
//!    B. The callback sees the `email-link:` state and calls [`finish`] instead
//!    of logging in, so the session stays A.
//! 3. [`finish`] checks the HMAC state **and** that the callback request carries
//!    A's own session cookie. The cookie check is what stops link-CSRF: without
//!    it, someone could sign a state for their own A and send the URL to a
//!    victim, whose Google sign-in would join the victim's email — and credit —
//!    to the sender. (`SameSite=Lax` sends the cookie on Google's top-level GET
//!    redirect.)
//!
//! Linking only merges credit balances (owner choice 2026-09-18). It never
//! changes a session, a role, or any other email-keyed record.

use axum::{
    Extension,
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
};
use serde_json::json;

use event_checkin_domain::models::auth::Claims;

use crate::db::person::{self, LinkOutcome, LinkProof};
use crate::error::ApiOk;
use crate::handlers::social_link::{GITHUB_STATE_TTL_SECS, sign_link_state, verify_link_state};
use crate::state::AppState;

/// Prefix that marks a Google OAuth `state` as an email link, not a login.
pub const STATE_PREFIX: &str = "email-link:";
/// HMAC domain separation from the GitHub/Telegram link states.
const STATE_TAG: &str = "email-link";

fn to_profile(result: &str) -> Response {
    Redirect::to(&format!("/profile?email_link={result}")).into_response()
}

/// GET /api/auth/email-link — start adding a second email.
#[worker::send]
pub async fn start(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    // A wallet session proves a wallet, not an email; linking needs Google proof.
    if !claims.email_verified || claims.email.starts_with("wallet:") {
        return to_profile("needs_google");
    }
    let expires = (js_sys::Date::now() / 1000.0) as i64 + GITHUB_STATE_TTL_SECS;
    let signed = match sign_link_state(
        STATE_TAG,
        &claims.email.to_lowercase(),
        expires,
        &state.config.jwt_secret,
    )
    .await
    {
        Ok(signed) => signed,
        Err(e) => {
            tracing::error!(error = %e, "email-link state signing failed");
            return to_profile("error");
        }
    };
    tracing::info!(
        identity_fingerprint = %state.log_fingerprint(&claims.email),
        "email link started"
    );
    let url = crate::auth::get_account_chooser_auth_url(&state, &format!("{STATE_PREFIX}{signed}"));
    Redirect::to(&url).into_response()
}

/// Finish a link from the OAuth callback. `signed_state` is the state with
/// [`STATE_PREFIX`] removed; `google_email` is the verified email Google
/// returned. Never sets a cookie.
pub async fn finish(
    state: &AppState,
    headers: &HeaderMap,
    signed_state: &str,
    google_email: &str,
) -> Response {
    let state_email =
        match verify_link_state(STATE_TAG, signed_state, &state.config.jwt_secret).await {
            Ok(email) => email,
            Err("expired") => return to_profile("expired"),
            Err(_) => return to_profile("invalid"),
        };

    let token = crate::auth::extract_token_from_headers(headers);
    let session_ok = match crate::auth::verify_token(&token, state).await {
        Ok(claims) => claims.email_verified && claims.email.eq_ignore_ascii_case(&state_email),
        Err(_) => false,
    };
    if !session_ok {
        tracing::warn!(
            identity_fingerprint = %state.log_fingerprint(&state_email),
            "email link refused: callback session does not match the link state"
        );
        return to_profile("session_mismatch");
    }

    let Some(db) = state.d1.as_deref() else {
        return to_profile("error");
    };
    let outcome = match person::link(db, &state_email, google_email, LinkProof::Google).await {
        Ok(outcome) => outcome,
        Err(e) => {
            tracing::error!(error = %e, "email link write failed");
            return to_profile("error");
        }
    };
    tracing::info!(
        identity_fingerprint = %state.log_fingerprint(&state_email),
        added_fingerprint = %state.log_fingerprint(google_email),
        outcome = ?outcome,
        "email link finished"
    );
    to_profile(match outcome {
        LinkOutcome::Linked => "linked",
        LinkOutcome::AlreadyLinked => "already",
        LinkOutcome::SameEmail => "same",
        LinkOutcome::Conflict => "conflict",
    })
}

/// GET /api/auth/linked-emails — every email of the signed-in person.
#[worker::send]
pub async fn linked_emails(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> ApiOk<serde_json::Value> {
    let emails = match (state.d1.as_deref(), claims.email.starts_with("wallet:")) {
        (Some(db), false) => person::emails_of(db, &claims.email)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "linked emails read failed — showing session email only");
                vec![claims.email.to_lowercase()]
            }),
        (_, true) => Vec::new(),
        (None, false) => vec![claims.email.to_lowercase()],
    };
    ApiOk::new(json!({ "emails": emails }))
}
