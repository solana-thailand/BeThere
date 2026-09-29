//! Poll cadence for the live dashboard (`/dashboard/live`).
//!
//! Every poll is a Worker request against the 100k/day free quota: 2.5 s is
//! 1,440 requests per hour per open tab. So the fast cadence only runs while
//! the event is actually on; the rest of the time a tab left open costs a
//! tenth of that. A hidden tab makes no requests at all (the page checks
//! `document.hidden` before each fetch).

/// Cadence while the event is on.
pub const LIVE_POLL_MS: u32 = 2_500;
/// Cadence while live but the last few polls brought no change.
pub const LIVE_IDLE_POLL_MS: u32 = 5_000;
/// Unchanged polls before `LIVE_IDLE_POLL_MS` applies.
pub const POLLS_BEFORE_IDLE: u8 = 3;
/// Cadence outside the event window, or when the event has no times.
pub const OFF_WINDOW_POLL_MS: u32 = 15_000;
/// The window opens this long before the start and closes this long after
/// the end: doors open early, and check-ins trail the scheduled end.
pub const WINDOW_MARGIN_MS: i64 = 30 * 60 * 1_000;
/// Bounds for the `?poll_ms=` override.
pub const MIN_OVERRIDE_MS: u32 = 1_000;
pub const MAX_OVERRIDE_MS: u32 = 60_000;

/// Whether `now_ms` falls in the event's live window. An event without a
/// start time (0 or negative) has no window. A missing or earlier end is
/// treated as a zero-length event at the start.
pub fn in_live_window(now_ms: i64, start_ms: i64, end_ms: i64) -> bool {
    if start_ms <= 0 {
        return false;
    }
    let end_ms = end_ms.max(start_ms);
    (start_ms - WINDOW_MARGIN_MS..=end_ms + WINDOW_MARGIN_MS).contains(&now_ms)
}

/// Parse a `?poll_ms=` value. Out-of-range values are clamped, garbage is
/// ignored.
pub fn parse_override(raw: Option<&str>) -> Option<u32> {
    raw?.trim()
        .parse::<u32>()
        .ok()
        .map(|ms| ms.clamp(MIN_OVERRIDE_MS, MAX_OVERRIDE_MS))
}

/// Delay before the next poll.
pub fn next_poll_ms(
    now_ms: i64,
    start_ms: i64,
    end_ms: i64,
    unchanged_polls: u8,
    override_ms: Option<u32>,
) -> u32 {
    if let Some(ms) = override_ms {
        return ms;
    }
    match (
        in_live_window(now_ms, start_ms, end_ms),
        unchanged_polls >= POLLS_BEFORE_IDLE,
    ) {
        (false, _) => OFF_WINDOW_POLL_MS,
        (true, true) => LIVE_IDLE_POLL_MS,
        (true, false) => LIVE_POLL_MS,
    }
}
