//! Coverage for the shared email plausibility check.

use event_checkin_domain::validation::is_plausible_email;

#[test]
fn accepts_normal_emails() {
    assert!(is_plausible_email("a@b.co"));
    assert!(is_plausible_email("dev.user+tag@example.com"));
    assert!(is_plausible_email("  padded@example.com  "));
}

#[test]
fn rejects_malformed_emails() {
    assert!(!is_plausible_email(""));
    assert!(!is_plausible_email("no-at-sign"));
    assert!(!is_plausible_email("@example.com"));
    assert!(!is_plausible_email("user@nodot"));
    assert!(!is_plausible_email("user@.com"));
    assert!(!is_plausible_email("user@example."));
    assert!(!is_plausible_email("has space@example.com"));
}

#[test]
fn rejects_the_synthetic_wallet_identity() {
    assert!(!is_plausible_email(
        "wallet:So1111111111111111111111111111111111111111"
    ));
}

#[test]
fn rejects_an_over_long_address() {
    let long = format!("{}@example.com", "a".repeat(250));
    assert!(!is_plausible_email(&long));
}
