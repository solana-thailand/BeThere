//! Input validation shared by every handler that accepts a user-typed value.
//!
//! Kept in the domain crate so the rules are defined once. Before this module
//! each ingress point rolled its own check — registration had a real one while
//! walk-in and waitlist only looked for an `@` — so the same address could be
//! accepted by one endpoint and rejected by another.

/// Minimal sanity check for a typed email address.
///
/// Deliberately not RFC 5322 — a complete parser accepts addresses no mail
/// provider would issue, and rejecting valid-but-unusual addresses is worse
/// than letting a typo through. This rejects only the obviously-invalid: no
/// `@`, an empty local part, a domain without a dot or with a leading/trailing
/// dot, embedded whitespace, or an implausible length.
///
/// Also rejects the synthetic `wallet:<address>` identity used for wallet-only
/// sessions, which must never be stored as a contact email.
pub fn is_plausible_email(email: &str) -> bool {
    let e = email.trim();
    if e.len() < 3 || e.len() > 254 || e.contains(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = e.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

/// `raw` if it is a same-origin path that is safe to redirect to, else `None`.
///
/// Every post-login redirect target (the OAuth `state` the worker redirects to
/// and the login page's `?next=`) is attacker-controllable: anyone can craft a
/// real Google sign-in link or a `/login?next=` link. Accepting it verbatim made
/// both an open redirect to any site, and on the frontend `location.href =
/// "javascript:…"` would run script in this origin.
///
/// Accepted: a path starting with a single `/` (`/e/rtm-6`, `/ticket/x?y=1`).
/// Rejected: absolute URLs and other schemes (`https:`, `javascript:`),
/// protocol-relative `//host`, any backslash (browsers read `/\host` as
/// `//host`), and control characters (header injection, and browsers strip tab
/// and newline, so `/\t/host` becomes `//host`).
pub fn safe_redirect_path(raw: &str) -> Option<&str> {
    let bytes = raw.as_bytes();
    match bytes {
        [b'/', b'/' | b'\\', ..] => None,
        [b'/', ..] if !raw.contains('\\') && !raw.chars().any(char::is_control) => Some(raw),
        _ => None,
    }
}
