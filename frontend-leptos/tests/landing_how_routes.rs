//! How it works (.plans/043 L4): at least one route stays on, `?track=`
//! opens one route, and timings read as minutes or hours.

use event_checkin_frontend::i18n::Locale;
use event_checkin_frontend::pages::landing::how::{Route, toggle_route};
use event_checkin_frontend::pages::landing::stats::duration_label;

#[test]
fn the_last_route_on_cannot_be_turned_off() {
    assert_eq!(
        toggle_route([true, false, false], Route::Thb),
        [true, false, false]
    );
    assert_eq!(
        toggle_route([true, true, false], Route::Thb),
        [false, true, false]
    );
    assert_eq!(
        toggle_route([false, true, false], Route::Ai),
        [false, true, true]
    );
}

#[test]
fn track_opens_sol_or_ai_only() {
    assert_eq!(Route::from_track("sol"), Some(Route::Sol));
    assert_eq!(Route::from_track("ai"), Some(Route::Ai));
    assert_eq!(Route::from_track("thb"), None);
    assert_eq!(Route::from_track(""), None);
}

#[test]
fn durations_read_in_minutes_then_hours() {
    assert_eq!(duration_label(6, Locale::en), "6 min");
    assert_eq!(duration_label(89, Locale::th), "89 นาที");
    // 4015 min = 66.9 h → 67 h (prod refund median, 2026-10-06).
    assert_eq!(duration_label(4015, Locale::en), "67 h");
    assert_eq!(duration_label(90, Locale::th), "2 ชม.");
}
