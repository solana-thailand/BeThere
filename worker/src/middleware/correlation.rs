//! Correlation ID middleware — request-scoped tracing identifier.

use axum::{extract::Request, http::HeaderName, middleware::Next, response::Response};
use std::{borrow::Cow, ops::Deref};
use uuid::Uuid;

/// Correlation ID extracted from `x-correlation-id` header or generated as UUID v7.
///
/// Inserted into request extensions by [`correlation_id_layer`] so downstream
/// handlers / layers can read it. Also added to every response header.
#[derive(Clone, Debug)]
pub struct CorrelationId(pub String);

impl Deref for CorrelationId {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Axum middleware that assigns a correlation ID to every request/response cycle.
///
/// 1. Reads `x-correlation-id` from the incoming request header.
/// 2. Falls back to a new UUID v7 if the header is missing.
/// 3. Inserts a [`CorrelationId`] into request extensions.
/// 4. Logs request start and completion with the correlation ID.
/// 5. Adds `x-correlation-id` to the response headers.
pub async fn correlation_id_layer(mut req: Request, next: Next) -> Response {
    // 1. Check for incoming x-correlation-id header
    let correlation_id = req
        .headers()
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::now_v7().to_string());

    // 2. Insert into request extensions
    req.extensions_mut()
        .insert(CorrelationId(correlation_id.clone()));

    // 3. Log request entry (capture method/path before `req` is consumed).
    //    The path is redacted first — see `redact_path`.
    let method = req.method().clone();
    let path = redact_path(req.uri().path()).into_owned();
    tracing::info!(
        correlation_id = %correlation_id,
        method = %method,
        path = %path,
        "request started"
    );

    // 4. Run handler
    let mut response = next.run(req).await;

    // 5. Add correlation_id to response headers
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("x-correlation-id"),
        correlation_id.parse().unwrap(),
    );

    // 6. Log response — elevate 5xx to error level (with method+path) so server
    //    failures are filterable/alertable in the log stream instead of hiding
    //    among INFO "request completed" lines.
    let code = response.status().as_u16();
    if code >= 500 {
        tracing::error!(
            correlation_id = %correlation_id,
            method = %method,
            path = %path,
            status = code,
            "request completed with server error"
        );
    } else {
        tracing::info!(
            correlation_id = %correlation_id,
            status = code,
            "request completed"
        );
    }

    response
}

/// Placeholder substituted for a redacted path segment.
const REDACTED_SEGMENT: &str = "{id}";

/// Base58 alphabet — no `0`, `O`, `I` or `l`.
const BASE58_ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Drop opaque identifiers out of a request path before it is logged.
///
/// Issue 070. Request paths carry capability tokens (`/api/claim/{token}`,
/// `/api/quiz/{token}/submit`) and durable identifiers (`/api/wallet/{address}/nfts`),
/// so logging `uri().path()` verbatim puts both back into the general log
/// stream that the rest of the issue cleared. The route shape is what makes
/// this line useful; the value is not needed here, because the handler behind
/// the route already emits a keyed fingerprint of it under the same
/// `correlation_id`.
///
/// Redaction is shape-driven rather than a per-route table so a new route that
/// takes an identifier segment is covered the day it is added. Slugs, numeric
/// ids and literal route segments stay readable.
fn redact_path(path: &str) -> Cow<'_, str> {
    if !path.split('/').any(is_opaque_identifier) {
        return Cow::Borrowed(path);
    }

    let mut redacted = String::with_capacity(path.len());
    for (index, segment) in path.split('/').enumerate() {
        match index {
            0 => {}
            _ => redacted.push('/'),
        }
        match is_opaque_identifier(segment) {
            true => redacted.push_str(REDACTED_SEGMENT),
            false => redacted.push_str(segment),
        }
    }
    Cow::Owned(redacted)
}

/// Whether a path segment is a high-entropy identifier rather than a readable
/// route or slug: a UUID (claim/quiz/adventure tokens, attendee ids) or a
/// base58-encoded 32-byte key (wallet, mint, escrow address).
fn is_opaque_identifier(segment: &str) -> bool {
    is_uuid_shaped(segment) || is_base58_key_shaped(segment)
}

fn is_uuid_shaped(segment: &str) -> bool {
    segment.len() == 36
        && segment
            .as_bytes()
            .iter()
            .enumerate()
            .all(|(index, byte)| match index {
                8 | 13 | 18 | 23 => *byte == b'-',
                _ => byte.is_ascii_hexdigit(),
            })
}

/// The uppercase requirement is what keeps human-readable slugs out of the
/// redaction set: slugs are generated lowercase and hyphenated, and `-` is not
/// in the base58 alphabet.
fn is_base58_key_shaped(segment: &str) -> bool {
    (32..=44).contains(&segment.len())
        && segment.bytes().all(|byte| BASE58_ALPHABET.contains(&byte))
        && segment.bytes().any(|byte| byte.is_ascii_uppercase())
}

#[cfg(test)]
mod redact_path_tests {
    use super::redact_path;

    #[test]
    fn readable_routes_are_left_alone() {
        for path in [
            "/api/health",
            "/api/events/readiness",
            "/api/public/event/solana-bangkok-deep-dive",
            "/api/wallet/leaderboard",
            "/api/adventure/config",
            "/api/metadata/pii-probe-event",
            "/api/deposit/credit-balance",
        ] {
            assert_eq!(redact_path(path), path, "{path} must stay readable");
        }
    }

    #[test]
    fn capability_tokens_are_dropped() {
        assert_eq!(
            redact_path("/api/claim/0198f2c4-5b6a-7c8d-9e0f-112233445566"),
            "/api/claim/{id}"
        );
        assert_eq!(
            redact_path("/api/quiz/0198f2c4-5b6a-7c8d-9e0f-112233445566/submit"),
            "/api/quiz/{id}/submit"
        );
        assert_eq!(
            redact_path("/api/adventure/0198f2c4-5b6a-7c8d-9e0f-112233445566/save"),
            "/api/adventure/{id}/save"
        );
    }

    #[test]
    fn wallet_addresses_are_dropped() {
        assert_eq!(
            redact_path("/api/wallet/HN7cABqLq46Es1jh92dQQisAq662SmxELLLsHHe4YWrH/nfts"),
            "/api/wallet/{id}/nfts"
        );
    }

    #[test]
    fn a_segment_is_never_partially_kept() {
        let redacted = redact_path("/api/claim/0198f2c4-5b6a-7c8d-9e0f-112233445566");
        assert!(
            !redacted.contains("0198"),
            "no fragment of the token may survive: {redacted}"
        );
    }
}
