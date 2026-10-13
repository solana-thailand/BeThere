//! When an event is over and who may register to watch its recording
//! (.issues/165, owner 2026-10-13).

use event_checkin_domain::models::event::{https_public_url, is_over};

const END: i64 = 1_791_000_000_000;

#[test]
fn the_end_time_ends_an_event_without_a_status_change() {
    assert!(!is_over(false, END, false, END - 1));
    assert!(is_over(false, END, false, END));
    assert!(is_over(false, END, false, END + 9 * 86_400_000));
    assert!(is_over(true, END, false, END - 1));
    // time TBA: the date is real, the hour is not, so a day later
    assert!(!is_over(false, END, true, END + 1));
    assert!(is_over(false, END, true, END + 86_400_000));
    // no end time: only the status can end it
    assert!(!is_over(false, 0, false, END));
}

#[test]
fn only_https_links_count_as_a_public_recording() {
    assert_eq!(
        https_public_url(" https://youtu.be/abc "),
        Some("https://youtu.be/abc".to_string())
    );
    assert_eq!(https_public_url("http://youtu.be/abc"), None);
    assert_eq!(https_public_url("https://"), None);
    assert_eq!(https_public_url("javascript:alert(1)"), None);
    assert_eq!(https_public_url(""), None);
}
