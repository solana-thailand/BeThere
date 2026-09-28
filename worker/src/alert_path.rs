//! The request path as it may appear in a Slack alert (.issues/159).
//!
//! Alerts leave the Worker for a third party, outside the log redaction of
//! Issue 070, so they must not carry what a raw path can hold: claim tokens
//! (`/api/claim/{token}`), attendee ids, wallet addresses, emails, event ids.
//! A segment is kept only when it looks like a route word — short, lowercase
//! ASCII letters with `-` or `_` — and every other segment becomes `{id}`.
//! That keeps `/api/escrow/mark-checked-in` readable and reduces
//! `/api/claim/3f9c…` to `/api/claim/{id}`. It over-redacts slugs that contain
//! digits, which is the safe direction.

/// Longest segment kept as a route word. Tokens and base58 wallets are longer;
/// the longest route words (`claim-forfeited`, `credit-balance`) are shorter.
const MAX_ROUTE_WORD: usize = 24;

/// `path` with every non-route-word segment replaced by `{id}`.
pub fn alert_path(path: &str) -> String {
    path.split('/')
        .map(|segment| match is_route_word(segment) {
            true => segment,
            false => "{id}",
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn is_route_word(segment: &str) -> bool {
    segment.is_empty()
        || (segment.len() <= MAX_ROUTE_WORD
            && segment
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'-' || b == b'_'))
}
