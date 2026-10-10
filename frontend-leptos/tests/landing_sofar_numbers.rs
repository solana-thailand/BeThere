//! "So far" (.plans/043 L5): numbers grouped the way they are read, the
//! count-up ends on the exact value, and the payers line keeps rule 4's
//! caveat in both languages.

use event_checkin_frontend::pages::landing::stats::{count_at, ease_out, group_thousands};

#[test]
fn thousands_are_grouped() {
    assert_eq!(group_thousands(0), "0");
    assert_eq!(group_thousands(999), "999");
    assert_eq!(group_thousands(34_000), "34,000");
    assert_eq!(group_thousands(1_234_567), "1,234,567");
}

#[test]
fn count_up_starts_at_zero_and_ends_exact() {
    assert_eq!(count_at(141, 0.0), 0);
    assert_eq!(count_at(141, 1.0), 141);
    assert_eq!(count_at(141, 7.5), 141);
    let mid = count_at(141, 0.5);
    assert!(mid > 70 && mid < 141, "{mid}");
    assert!(ease_out(-1.0) == 0.0 && ease_out(2.0) == 1.0);
}

#[test]
fn payers_line_carries_the_caveat() {
    let root = env!("CARGO_MANIFEST_DIR");
    for (lang, caveat) in [
        ("en", "may have meant to come anyway"),
        ("th", "อาจตั้งใจมาอยู่แล้ว"),
    ] {
        let text = std::fs::read_to_string(format!("{root}/locales/{lang}/landing.json")).unwrap();
        let catalog: serde_json::Value = serde_json::from_str(&text).unwrap();
        let note = catalog["sofar"]["payers_note"].as_str().unwrap_or_default();
        assert!(note.contains(caveat), "{lang}: {note}");
        let all = catalog["sofar"].to_string().to_lowercase();
        assert!(
            !all.contains("forfeit") && !all.contains("credit"),
            "{lang}"
        );
    }
}

#[test]
fn payers_line_hidden_when_nobody_paid() {
    use event_checkin_domain::models::public_stats::PublicStats;
    use event_checkin_frontend::pages::landing::stats::payers_line_shown;
    let stats = |paid, came| PublicStats {
        measured_at: "2026-10-06T00:00:00Z".to_string(),
        events_held: 18,
        onsite_registrations: 27,
        online_registrations: 0,
        door_scans: 21,
        deposits_handled_count: 0,
        deposits_handled_thb: 0,
        deposit_payers: paid,
        deposit_payers_came: came,
        slip_check: None,
        refund_after_end: None,
        payers_by_event: Vec::new(),
    };
    assert!(!payers_line_shown(&stats(0, 0)), "0 of 0 must not be drawn");
    assert!(payers_line_shown(&stats(54, 50)));
    assert!(payers_line_shown(&stats(1, 0)), "0 of 1 is a real count");
}
