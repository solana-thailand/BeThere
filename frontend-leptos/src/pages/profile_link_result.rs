//! The one-shot banner `/profile` shows after a link callback redirects back
//! with `?email_link=`, `?linked=` or `?error=`.
//!
//! The query is parsed once, on mount, but translated at render time: the
//! saved language is restored in an Effect that runs after component bodies,
//! so text picked in the body would come out in the browser's language.

use crate::i18n::{Locale, td_string};
use crate::locale::fill;

/// A link callback's outcome, as the redirect reported it.
#[derive(Clone, Debug, PartialEq)]
pub enum LinkResult {
    /// `?email_link=<result>` from the Google email-link callback.
    Email(String),
    /// `?linked=<provider>` after a social account was linked.
    Linked(String),
    /// `?error=<code>` from a failed social link.
    Error(String),
}

impl LinkResult {
    /// The first link parameter present, in `email_link`, `linked`, `error`
    /// order. `get` reads one query parameter.
    pub fn from_query(get: impl Fn(&str) -> Option<String>) -> Option<Self> {
        get("email_link")
            .map(Self::Email)
            .or_else(|| get("linked").map(Self::Linked))
            .or_else(|| get("error").map(Self::Error))
    }

    /// `(is_success, message)` in `locale`.
    pub fn message(&self, locale: Locale) -> (bool, String) {
        match self {
            Self::Email(result) => email_message(result, locale),
            Self::Linked(provider) => (true, linked_message(provider, locale)),
            Self::Error(code) => (false, error_message(code, locale)),
        }
    }
}

fn email_message(result: &str, l: Locale) -> (bool, String) {
    let (ok, text) = match result {
        "linked" => (true, td_string!(l, profile.email_linked)),
        "already" => (true, td_string!(l, profile.email_already)),
        "same" => (false, td_string!(l, profile.email_same)),
        "conflict" => (false, td_string!(l, profile.email_conflict)),
        "session_mismatch" => (false, td_string!(l, profile.email_session_mismatch)),
        "needs_google" => (false, td_string!(l, profile.email_needs_google)),
        "expired" => (false, td_string!(l, profile.email_expired)),
        "cancelled" => (false, td_string!(l, profile.email_cancelled)),
        _ => (false, td_string!(l, profile.email_failed)),
    };
    (ok, text.to_string())
}

fn linked_message(provider: &str, l: Locale) -> String {
    match provider {
        "github" => td_string!(l, profile.linked_github).to_string(),
        "telegram" => td_string!(l, profile.linked_telegram).to_string(),
        other => fill(td_string!(l, profile.linked_other), &[("provider", other)]),
    }
}

fn error_message(code: &str, l: Locale) -> String {
    let text = match code {
        "github_denied" => td_string!(l, profile.err_github_denied),
        "github_state_expired" => td_string!(l, profile.err_github_expired),
        "github_no_code" | "github_no_state" | "github_invalid_state" => {
            td_string!(l, profile.err_github_invalid)
        }
        "github_token_failed" => td_string!(l, profile.err_github_token),
        "github_user_failed" => td_string!(l, profile.err_github_user),
        "github_save_failed" | "db_unavailable" => td_string!(l, profile.err_github_save),
        "telegram_bad_signature" | "telegram_invalid" => {
            td_string!(l, profile.err_telegram_verify)
        }
        "telegram_expired" => td_string!(l, profile.err_telegram_expired),
        "telegram_save_failed" | "telegram_unconfigured" => {
            td_string!(l, profile.err_telegram_save)
        }
        other => return fill(td_string!(l, profile.err_link_other), &[("code", other)]),
    };
    text.to_string()
}
