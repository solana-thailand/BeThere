//! Turnstile bot check (`.issues/170`): when it runs, what counts as a token,
//! and how siteverify's answer is read.

use axum::http::{HeaderMap, HeaderValue};
use event_checkin_domain::turnstile::{REJECTED, TOKEN_HEADER, is_rejection};
use event_checkin_worker::turnstile::{Gate, SiteverifyResponse, gate, token_from, verdict};

#[test]
fn gate_is_on_only_with_both_keys_and_no_dev_mode() {
    assert_eq!(gate("site", "secret", false), Gate::Enforced);
    assert_eq!(gate("site", "secret", true), Gate::Off, "DEV_MODE bypasses");
    assert_eq!(gate("", "secret", false), Gate::Off);
    assert_eq!(gate("site", "", false), Gate::Off);
    assert_eq!(gate("  ", " ", false), Gate::Off, "whitespace is not a key");
}

fn headers_with(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(TOKEN_HEADER, HeaderValue::from_str(token).unwrap());
    headers
}

#[test]
fn token_must_be_present_non_empty_and_bounded() {
    assert_eq!(token_from(&HeaderMap::new()), None);
    assert_eq!(token_from(&headers_with("   ")), None);
    assert_eq!(
        token_from(&headers_with(" XXXX.DUMMY.TOKEN.XXXX ")),
        Some("XXXX.DUMMY.TOKEN.XXXX")
    );
    assert_eq!(
        token_from(&headers_with(&"a".repeat(2048))).map(str::len),
        Some(2048)
    );
    assert_eq!(token_from(&headers_with(&"a".repeat(2049))), None);
}

#[test]
fn siteverify_success_passes_and_failure_carries_the_codes() {
    let pass: SiteverifyResponse = serde_json::from_str(
        r#"{"success":true,"challenge_ts":"2026-09-29T00:00:00.000Z","hostname":"example.com","error-codes":[],"action":"","cdata":""}"#,
    )
    .unwrap();
    assert_eq!(verdict(&pass), Ok(()));

    let fail: SiteverifyResponse = serde_json::from_str(
        r#"{"success":false,"error-codes":["invalid-input-response","timeout-or-duplicate"]}"#,
    )
    .unwrap();
    assert_eq!(
        verdict(&fail),
        Err("invalid-input-response,timeout-or-duplicate".to_string())
    );

    let bare: SiteverifyResponse = serde_json::from_str(r#"{"success":false}"#).unwrap();
    assert_eq!(verdict(&bare), Err(String::new()));
}

#[test]
fn rejection_text_survives_the_error_envelope_and_the_redactor() {
    let public = event_checkin_domain::models::error::AppError::Forbidden(REJECTED.to_string())
        .public_message()
        .into_owned();
    assert!(is_rejection(&public), "{public}");
    assert!(!REJECTED.contains("://"));
    assert!(!is_rejection("forbidden: not an organizer of this event"));
}
