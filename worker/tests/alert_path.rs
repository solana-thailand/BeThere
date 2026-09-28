//! `.issues/159`: Slack alerts carry route words, never identifiers.

use event_checkin_worker::alert_path::alert_path;

#[test]
fn route_words_survive() {
    for path in [
        "/api/health",
        "/api/escrow/mark-checked-in",
        "/api/escrow/claim-forfeited",
        "/api/deposit/credit-balance",
        "/api/privacy/delete-request",
        "/",
    ] {
        assert_eq!(alert_path(path), path);
    }
}

#[test]
fn identifiers_become_placeholders() {
    let cases = [
        (
            "/api/claim/3f9c2a7e-5b1d-4c8e-9a0f-1b2c3d4e5f60",
            "/api/claim/{id}",
        ),
        (
            "/api/public/ticket/01a0e458-ed1a-7100-a2e3-792c6087d0e8",
            "/api/public/ticket/{id}",
        ),
        (
            "/api/wallet/7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU/deposits",
            "/api/wallet/{id}/deposits",
        ),
        ("/api/attendee/someone@example.com", "/api/attendee/{id}"),
        (
            "/api/public/event/road-to-mainnet-6-bangkok",
            "/api/public/event/{id}",
        ),
        ("/api/events/ABCdef", "/api/events/{id}"),
        (
            "/api/claim/abcdefghijklmnopqrstuvwxyzabcdefgh",
            "/api/claim/{id}",
        ),
    ];
    for (raw, want) in cases {
        assert_eq!(alert_path(raw), want, "{raw}");
    }
}

/// Source guard: both alert messages format the redacted path, and the test
/// alert names its requester by fingerprint, not by email.
#[test]
fn alert_text_never_formats_the_raw_path_or_an_email() {
    let read = |rel: &str| {
        std::fs::read_to_string(format!("{}/src/{rel}", env!("CARGO_MANIFEST_DIR")))
            .unwrap_or_else(|e| panic!("{rel}: {e}"))
    };
    let alert = read("middleware/alert.rs");
    assert!(
        !alert.contains("{method} {path}"),
        "an alert formats the raw path"
    );
    assert_eq!(
        alert.matches("{method} {shown_path}").count(),
        2,
        "both alerts use the redacted path"
    );
    let admin = read("handlers/attendee/admin.rs");
    assert!(
        !admin.contains("requested by {}\", claims.email"),
        "the test alert posts the email"
    );
    assert!(admin.contains("state.log_fingerprint(&claims.email)"));
}
