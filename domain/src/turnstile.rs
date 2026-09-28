//! Wire contract for the Cloudflare Turnstile bot check (`.issues/170`).
//!
//! Shared so the worker and the frontend cannot drift on the header name or
//! on the rejection text the frontend recognises to show localised copy.

/// Request header that carries the widget's single-use token.
pub const TOKEN_HEADER: &str = "x-turnstile-token";

/// Caller-facing reason when the check fails. The worker returns it inside a
/// 403 (`forbidden: …`); keep `://` out of it (the error redactor).
pub const REJECTED: &str = "human check failed or expired, please try again";

/// Whether an API error message is a Turnstile rejection.
pub fn is_rejection(public_error: &str) -> bool {
    public_error.contains(REJECTED)
}
