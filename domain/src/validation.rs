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
