//! The admin deposit tabs show "View Slip" only for a real serving path.
//! Credit-covered and staff-comp deposits store a sentinel in `slip_url`;
//! linking it would send the organizer to a 404. The check used to be copied
//! into four tabs and now lives in one place.

use event_checkin_frontend::pages::admin_deposit_slip_link::is_viewable_slip_url;

#[test]
fn serving_paths_are_viewable() {
    assert!(is_viewable_slip_url(Some("/api/deposit/thb/slip/evt/att")));
    assert!(is_viewable_slip_url(Some("https://r2.example/slip.png")));
}

#[test]
fn sentinels_and_missing_are_not_viewable() {
    for sentinel in ["ROLLING_CREDIT_AUTO_APPLIED", "STAFF_COMP_WAIVED", ""] {
        assert!(!is_viewable_slip_url(Some(sentinel)), "{sentinel:?}");
    }
    assert!(!is_viewable_slip_url(None));
}
