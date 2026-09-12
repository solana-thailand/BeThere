//! An API error body must never carry an upstream response back to the caller.
//!
//! Issue 078. `AppError`'s `Display` renders the operator-facing detail, and
//! that string used to be serialized straight into the public JSON body. On
//! `GET /api/claim/{token}` — unauthenticated by design, because the claim link
//! is what an attendee opens — a D1 miss fell through to Google Sheets, and a
//! Sheets failure handed the caller the platform spreadsheet ID, the exact API
//! and range being queried, and the upstream error text verbatim.
//!
//! These tests pin the split: `Display` keeps the detail for the log,
//! `public_message` is what may leave the Worker.

use event_checkin_domain::models::error::{AppError, REDACTED_URL, redact_urls};

/// The exact production shape that Issue 078 was filed from, with a real
/// spreadsheet ID standing in for staging's empty path segment.
const SHEET_ID: &str = "1AbCdEfGhIjKlMnOpQrStUvWxYz0123456789_ABCD";

fn observed_sheets_failure() -> AppError {
    AppError::Internal(format!(
        "failed to look up claim: HTTP 404 from \
         https://sheets.googleapis.com/v4/spreadsheets/{SHEET_ID}/values/Attendees%21A2%3AAG: \
         {{\n  \"error\": {{\n    \"code\": 404,\n    \"message\": \"Requested entity was not found.\"\n  }}\n}}"
    ))
}

#[test]
fn internal_body_discloses_nothing_about_the_upstream() {
    let error = observed_sheets_failure();
    let public = error.public_message();

    assert_eq!(public, "internal error");
    assert!(!public.contains(SHEET_ID));
    assert!(!public.contains("sheets.googleapis.com"));
    assert!(!public.contains("://"));
}

#[test]
fn internal_detail_survives_for_the_operator() {
    let error = observed_sheets_failure();
    let detail = error.to_string();

    assert!(
        detail.contains(SHEET_ID) && detail.contains("Requested entity was not found."),
        "the log line must keep what the body drops, or the fix has traded \
         one problem for another: {detail}"
    );
}

#[test]
fn external_body_keeps_service_and_status_but_not_the_response() {
    let error = AppError::External {
        service: "crossmint".to_string(),
        status: 502,
        body: "{\"error\":\"unauthorized\",\"endpoint\":\"https://staging.crossmint.com/api/2022-06-09/collections/abc123/nfts\"}".to_string(),
    };

    let public = error.public_message();
    assert_eq!(public, "external service error: crossmint returned 502");
    assert!(!public.contains("crossmint.com"));
    assert!(error.to_string().contains("crossmint.com"));
}

/// Every variant, so a variant added later cannot quietly opt out.
#[test]
fn no_variant_puts_a_url_in_the_body() {
    let leak = "https://mainnet.helius-rpc.com/?api-key=0198f2c4-5b6a-7c8d-9e0f-112233445566";
    let variants = [
        AppError::NotFound(format!("event not found: {leak}")),
        AppError::Unauthorized(format!("token rejected by {leak}")),
        AppError::Forbidden(format!("denied by {leak}")),
        AppError::Validation(format!("escrow not found on-chain: {leak}")),
        AppError::Conflict(format!("already settled per {leak}")),
        AppError::Gone(format!("expired per {leak}")),
        AppError::RateLimited(format!("quota tripped at {leak}")),
        AppError::Internal(format!("failed: {leak}")),
        AppError::External {
            service: "helius-das".to_string(),
            status: 502,
            body: leak.to_string(),
        },
    ];

    for error in &variants {
        let public = error.public_message();
        assert!(
            !public.contains("://") && !public.contains("api-key"),
            "{} leaked an endpoint: {public}",
            error.status_code()
        );
    }
}

/// A 4xx message is part of the API contract — the caller needs to know what
/// they got wrong. Only the URL is removed.
#[test]
fn client_errors_keep_their_explanation() {
    let error = AppError::Validation("invalid organizer_wallet: not base58".to_string());
    assert_eq!(
        error.public_message(),
        "validation error: invalid organizer_wallet: not base58"
    );

    let error = AppError::NotFound("event 'solana-bangkok' not found".to_string());
    assert_eq!(
        error.public_message(),
        "not found: event 'solana-bangkok' not found"
    );
}

#[test]
fn redaction_leaves_url_free_text_untouched() {
    let message = "not found: event 'solana-bangkok' not found";
    assert!(matches!(
        redact_urls(message),
        std::borrow::Cow::Borrowed(_)
    ));
    assert_eq!(redact_urls(message), message);
}

#[test]
fn redaction_removes_the_scheme_too() {
    let redacted = redact_urls("fetch failed: https://sheets.googleapis.com/v4 timed out");
    assert_eq!(redacted, format!("fetch failed: {REDACTED_URL} timed out"));
}

/// The second URL must not swallow the text emitted between the two — the
/// backwards scheme walk is floored at the previous cursor for this reason.
#[test]
fn redaction_handles_several_urls_in_one_message() {
    let redacted = redact_urls("from https://a.example/x fell back to https://b.example/y");
    assert_eq!(
        redacted,
        format!("from {REDACTED_URL} fell back to {REDACTED_URL}")
    );
}

/// Upstream bodies arrive as JSON, where the URL is quoted rather than
/// whitespace-delimited.
#[test]
fn redaction_terminates_a_quoted_url() {
    let redacted = redact_urls("{\"endpoint\":\"https://sheets.googleapis.com/v4\",\"code\":404}");
    assert_eq!(
        redacted,
        format!("{{\"endpoint\":\"{REDACTED_URL}\",\"code\":404}}")
    );
}

#[test]
fn redaction_covers_non_https_schemes() {
    for url in [
        "http://10.0.0.1:8787/api/internal",
        "ws://bethere.internal/socket",
        "postgres://user@db.internal:5432/bethere",
    ] {
        let message = format!("dial failed: {url}");
        let redacted = redact_urls(&message);
        assert!(
            !redacted.contains("://"),
            "{url} survived redaction: {redacted}"
        );
    }
}

/// Multi-byte input must not panic the byte-indexed scanner.
#[test]
fn redaction_is_utf8_safe() {
    let redacted = redact_urls("ไม่พบข้อมูล: https://sheets.googleapis.com/v4 — ลองใหม่");
    assert_eq!(redacted, format!("ไม่พบข้อมูล: {REDACTED_URL} — ลองใหม่"));
}
