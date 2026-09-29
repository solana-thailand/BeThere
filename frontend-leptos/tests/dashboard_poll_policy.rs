//! `/dashboard/live` poll cadence: fast only while the event is on (P0-1).

use event_checkin_frontend::utils::poll_policy::{
    LIVE_IDLE_POLL_MS, LIVE_POLL_MS, MAX_OVERRIDE_MS, MIN_OVERRIDE_MS, OFF_WINDOW_POLL_MS,
    POLLS_BEFORE_IDLE, WINDOW_MARGIN_MS, in_live_window, next_poll_ms, parse_override,
};

const START: i64 = 1_790_000_000_000;
const END: i64 = START + 3 * 3_600_000;

#[test]
fn window_includes_the_margins_and_nothing_past_them() {
    assert!(in_live_window(START - WINDOW_MARGIN_MS, START, END));
    assert!(in_live_window(START + 60_000, START, END));
    assert!(in_live_window(END + WINDOW_MARGIN_MS, START, END));
    assert!(!in_live_window(START - WINDOW_MARGIN_MS - 1, START, END));
    assert!(!in_live_window(END + WINDOW_MARGIN_MS + 1, START, END));
}

#[test]
fn event_without_times_has_no_window() {
    assert!(!in_live_window(START, 0, 0));
    assert_eq!(next_poll_ms(START, 0, 0, 0, None), OFF_WINDOW_POLL_MS);
}

#[test]
fn missing_end_is_a_zero_length_event_at_the_start() {
    assert!(in_live_window(START + WINDOW_MARGIN_MS, START, 0));
    assert!(!in_live_window(START + WINDOW_MARGIN_MS + 1, START, 0));
}

#[test]
fn fast_only_inside_the_window_then_idle_backoff() {
    let during = START + 60_000;
    assert_eq!(next_poll_ms(during, START, END, 0, None), LIVE_POLL_MS);
    assert_eq!(
        next_poll_ms(during, START, END, POLLS_BEFORE_IDLE, None),
        LIVE_IDLE_POLL_MS
    );
    let next_day = END + 86_400_000;
    assert_eq!(
        next_poll_ms(next_day, START, END, 0, None),
        OFF_WINDOW_POLL_MS
    );
}

#[test]
fn off_window_cadence_stays_within_the_request_budget() {
    // One visible tab left open all day must stay far below the 100k/day
    // free-plan quota.
    let per_day = 86_400_000 / OFF_WINDOW_POLL_MS;
    assert!(per_day <= 6_000, "{per_day} polls/day");
}

#[test]
fn override_wins_and_is_clamped() {
    assert_eq!(parse_override(Some("10000")), Some(10_000));
    assert_eq!(parse_override(Some("10")), Some(MIN_OVERRIDE_MS));
    assert_eq!(parse_override(Some("999999")), Some(MAX_OVERRIDE_MS));
    assert_eq!(parse_override(Some("fast")), None);
    assert_eq!(parse_override(None), None);
    assert_eq!(next_poll_ms(START, START, END, 0, Some(10_000)), 10_000);
}
