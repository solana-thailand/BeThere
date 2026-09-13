//! Typed application error enum for consistent error handling across handlers.
//! Replaces `Result<T, String>` with `Result<T, AppError>` for proper error
//! classification and HTTP status code mapping.

use std::{borrow::Cow, fmt};

/// Application-level error with HTTP status code mapping.
#[derive(Debug)]
pub enum AppError {
    /// Resource not found (404)
    NotFound(String),
    /// Authentication required or failed (401)
    Unauthorized(String),
    /// Authenticated but lacks permission (403)
    Forbidden(String),
    /// Input validation failed (400)
    Validation(String),
    /// Request conflicts with the current resource state (409)
    Conflict(String),
    /// Resource existed but is no longer available (410)
    Gone(String),
    /// External service error (502)
    External {
        service: String,
        status: u16,
        body: String,
    },
    /// Rate limit exceeded (429)
    RateLimited(String),
    /// Internal server error (500)
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
            Self::Unauthorized(msg) => write!(f, "unauthorized: {msg}"),
            Self::Forbidden(msg) => write!(f, "forbidden: {msg}"),
            Self::Validation(msg) => write!(f, "validation error: {msg}"),
            Self::Conflict(msg) => write!(f, "conflict: {msg}"),
            Self::Gone(msg) => write!(f, "gone: {msg}"),
            Self::External {
                service,
                status,
                body,
            } => {
                write!(
                    f,
                    "external service error: {service} returned {status}: {body}"
                )
            }
            Self::RateLimited(msg) => write!(f, "rate limited: {msg}"),
            Self::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        Self::Internal(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        Self::Internal(s.to_string())
    }
}

impl AppError {
    /// Map this error to an HTTP status code.
    pub fn status_code(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::Unauthorized(_) => 401,
            Self::Forbidden(_) => 403,
            Self::Validation(_) => 400,
            Self::Conflict(_) => 409,
            Self::Gone(_) => 410,
            Self::External { .. } => 502,
            Self::RateLimited(_) => 429,
            Self::Internal(_) => 500,
        }
    }

    /// Check if this is a not-found error.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_))
    }

    /// Check if this is an auth error (unauthorized or forbidden).
    pub fn is_auth_error(&self) -> bool {
        matches!(self, Self::Unauthorized(_) | Self::Forbidden(_))
    }

    /// The message that may be returned to the caller in an API error body.
    ///
    /// Issue 078. [`Display`](fmt::Display) renders the operator-facing detail
    /// and is what belongs on the log line; it is **not** safe as a response
    /// body, because 5xx messages are routinely built by wrapping an upstream
    /// failure (`format!("failed to look up claim: {e}")`) and therefore carry
    /// that upstream's endpoint URL, identifiers and verbatim response body.
    /// `GET /api/claim/{token}` is unauthenticated, so anyone could read them.
    ///
    /// The split is by variant, not by message text — special-casing one
    /// upstream would leave the policy wrong for the next one:
    ///
    /// - 5xx (`Internal`, `External`) are *our* failures. The caller gets a
    ///   stable, content-free string; the detail stays on the `tracing` event,
    ///   findable by the `x-correlation-id` echoed on the same response.
    /// - 4xx describe what the *caller* did wrong, so the message is part of
    ///   the API contract and is kept — but still passed through
    ///   [`redact_urls`], because a client error can also be produced by
    ///   wrapping an upstream error (an RPC rejection surfacing as
    ///   `Validation`), and that wrapping is exactly where a URL leaks in.
    pub fn public_message(&self) -> Cow<'_, str> {
        match self {
            Self::Internal(_) => Cow::Borrowed("internal error"),
            Self::External {
                service, status, ..
            } => Cow::Owned(format!(
                "external service error: {service} returned {status}"
            )),
            _ => Cow::Owned(redact_urls(&self.to_string()).into_owned()),
        }
    }
}

/// Placeholder substituted for a URL removed from a caller-facing message.
pub const REDACTED_URL: &str = "[redacted-url]";

/// Replace every `scheme://…` run in `message` with [`REDACTED_URL`].
///
/// Issue 078. An upstream endpoint is not a secret on its own, but it names
/// internal infrastructure and it is where credential-bearing identifiers hide:
/// the platform spreadsheet ID sits in the Sheets path, and the Solana RPC key
/// sits in the query string of `SolanaConfig::full_rpc_url`.
///
/// Redaction is shape-driven rather than a host allowlist so an upstream added
/// tomorrow is covered the day it is added — the same reasoning as
/// `middleware::correlation::redact_path` in the Worker.
pub fn redact_urls(message: &str) -> Cow<'_, str> {
    match message.contains("://") {
        false => Cow::Borrowed(message),
        true => Cow::Owned(rewrite_without_urls(message)),
    }
}

fn rewrite_without_urls(message: &str) -> String {
    let bytes = message.as_bytes();
    let mut redacted = String::with_capacity(message.len());
    let mut cursor = 0usize;

    while cursor < message.len() {
        match message[cursor..].find("://") {
            None => {
                redacted.push_str(&message[cursor..]);
                break;
            }
            Some(offset) => {
                let marker = cursor + offset;
                let start = scheme_start(bytes, marker, cursor);
                redacted.push_str(&message[cursor..start]);
                redacted.push_str(REDACTED_URL);
                cursor = url_end(bytes, marker + "://".len());
            }
        }
    }

    redacted
}

/// Walk back from the `://` marker over the scheme, stopping at `floor` so a
/// second URL in the same message can never re-consume text already emitted.
fn scheme_start(bytes: &[u8], marker: usize, floor: usize) -> usize {
    let mut start = marker;
    while start > floor && is_scheme_byte(bytes[start - 1]) {
        start -= 1;
    }
    start
}

fn is_scheme_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.')
}

/// Consume to the first byte that cannot continue a URL. Quotes and backslashes
/// terminate as well as whitespace: upstream bodies arrive as JSON, where the
/// URL is wrapped in `"` and the message is escaped.
fn url_end(bytes: &[u8], from: usize) -> usize {
    let mut end = from;
    while end < bytes.len() && !is_url_terminator(bytes[end]) {
        end += 1;
    }
    end
}

fn is_url_terminator(byte: u8) -> bool {
    byte.is_ascii_whitespace() || matches!(byte, b'"' | b'\'' | b'`' | b'<' | b'>' | b'\\')
}

/// Helper trait to convert `Result<T, String>` to `Result<T, AppError>`.
pub trait IntoAppError<T> {
    fn map_to_app(self) -> Result<T, AppError>;
    fn not_found(self) -> Result<T, AppError>;
    fn unauthorized(self) -> Result<T, AppError>;
    fn forbidden(self) -> Result<T, AppError>;
    fn validation(self) -> Result<T, AppError>;
}

impl<T> IntoAppError<T> for Result<T, String> {
    fn map_to_app(self) -> Result<T, AppError> {
        self.map_err(AppError::Internal)
    }
    fn not_found(self) -> Result<T, AppError> {
        self.map_err(AppError::NotFound)
    }
    fn unauthorized(self) -> Result<T, AppError> {
        self.map_err(AppError::Unauthorized)
    }
    fn forbidden(self) -> Result<T, AppError> {
        self.map_err(AppError::Forbidden)
    }
    fn validation(self) -> Result<T, AppError> {
        self.map_err(AppError::Validation)
    }
}
