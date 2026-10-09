//! Walk-ins belong on the In-Person roster tab.
//!
//! The worker stores walk-ins as `participation_type = 'walkin'`. The admin
//! roster used to route tabs by `is_in_person` alone, which is false for the
//! sentinel, so every walk-in showed up under Online and the In-Person tab's
//! Walk-in pill was always empty.

use event_checkin_frontend::utils::{is_in_person, is_on_site_roster, is_walkin};

#[test]
fn walkin_sentinel_is_on_site() {
    for raw in ["walkin", "Walkin", "WALKIN", " walkin "] {
        assert!(is_walkin(raw), "{raw:?}");
        assert!(is_on_site_roster(raw), "{raw:?}");
    }
}

#[test]
fn walkin_stays_out_of_is_in_person() {
    // Deposit badges and the participation toggle depend on this staying
    // false, even though the domain now parses it as the in-person `WalkIn`.
    assert!(!is_in_person("walkin"));
}

#[test]
fn online_and_other_tracks_stay_off_site() {
    for raw in [
        "Online",
        "online",
        "Virtual",
        "retrospective",
        "test",
        "walk-in-ish",
    ] {
        assert!(!is_on_site_roster(raw), "{raw:?}");
    }
}

#[test]
fn in_person_values_stay_on_site() {
    for raw in ["In-Person", "in_person", "In Person", ""] {
        assert!(is_on_site_roster(raw), "{raw:?}");
    }
}
