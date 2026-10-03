//! The event page's attendance row: what is shown per track, and when.

use event_checkin_frontend::pages::public_event::attendance::{
    MIN_PUBLIC_UNCAPPED_COUNT, Track, TrackAttendance, checked_in_to_show, total_when_both,
};

#[test]
fn uncapped_track_hides_a_small_count() {
    assert_eq!(
        TrackAttendance::new(Track::InPerson, MIN_PUBLIC_UNCAPPED_COUNT - 1, None),
        None
    );
    assert!(TrackAttendance::new(Track::InPerson, MIN_PUBLIC_UNCAPPED_COUNT, None).is_some());
}

#[test]
fn capped_track_shows_even_when_empty() {
    let t = TrackAttendance::new(Track::Online, 0, Some(20)).expect("capped track is shown");
    assert_eq!(
        (t.left(), t.percent(), t.is_full()),
        (Some(20), Some(0), false)
    );
}

#[test]
fn capped_track_reports_room_and_fill() {
    let t = TrackAttendance::new(Track::InPerson, 3, Some(40)).unwrap();
    assert_eq!((t.left(), t.percent()), (Some(37), Some(7)));
    let full = TrackAttendance::new(Track::InPerson, 40, Some(40)).unwrap();
    assert!(full.is_full());
    assert_eq!(full.percent(), Some(100));
}

#[test]
fn overbooked_or_zero_cap_never_leaves_the_bar() {
    let over = TrackAttendance::new(Track::InPerson, 55, Some(40)).unwrap();
    assert_eq!((over.left(), over.percent()), (Some(0), Some(100)));
    let zero = TrackAttendance::new(Track::InPerson, 0, Some(0)).unwrap();
    assert_eq!((zero.is_full(), zero.percent()), (true, Some(100)));
}

#[test]
fn total_needs_two_populated_tracks() {
    let ip = TrackAttendance::new(Track::InPerson, 12, None).unwrap();
    let on = TrackAttendance::new(Track::Online, 8, Some(20)).unwrap();
    assert_eq!(total_when_both(&[ip, on]), Some(20));
    let empty_on = TrackAttendance::new(Track::Online, 0, Some(20)).unwrap();
    assert_eq!(total_when_both(&[ip, empty_on]), None);
    assert_eq!(total_when_both(&[ip]), None);
}

#[test]
fn checked_in_line_needs_a_known_non_zero_count() {
    assert_eq!(checked_in_to_show(None), None, "unknown is not zero");
    assert_eq!(
        checked_in_to_show(Some(0)),
        None,
        "a sheet-only event reads 0"
    );
    assert_eq!(checked_in_to_show(Some(12)), Some(12));
}
