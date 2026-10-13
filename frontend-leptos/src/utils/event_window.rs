//! Whether an event is over, for the public event page.
//!
//! The page used to read only the organizer-set status, so a past event whose
//! status was never moved to completed rendered "Happening now!" and the seat
//! form days later (RTM #6, nine days after, .issues/165). The end time now
//! decides too. Events whose time is still TBA keep the status-only rule:
//! their end time is not a real time yet.

/// Over once the organizer marks it completed, or once its end time passes.
pub fn is_over(status: &str, end_ms: i64, time_tba: bool, now_ms: i64) -> bool {
    status.eq_ignore_ascii_case("completed") || (!time_tba && end_ms > 0 && now_ms >= end_ms)
}
