//! A past event reads as over even when nobody set its status (.issues/165:
//! RTM #6 said "Happening now!" nine days after it ended).

use event_checkin_frontend::utils::event_window::is_over;

const END: i64 = 1_791_000_000_000;

#[test]
fn the_end_time_ends_an_event_without_a_status_change() {
    assert!(!is_over("active", END, false, END - 1));
    assert!(is_over("active", END, false, END));
    assert!(is_over("active", END, false, END + 9 * 86_400_000));
}

#[test]
fn completed_status_ends_it_early_and_tba_or_missing_times_wait_for_it() {
    assert!(is_over("Completed", END, false, END - 1));
    assert!(is_over("completed", 0, true, 0));
    // time TBA: a day after the end; no end time: only the status
    assert!(!is_over("active", END, true, END + 1));
    assert!(is_over("active", END, true, END + 86_400_000));
    assert!(!is_over("active", 0, false, END));
}
