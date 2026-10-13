//! Whether an event is over, for the public event page.
//!
//! The page used to read only the organizer-set status, so a past event whose
//! status was never moved to completed rendered "Happening now!" and the seat
//! form days later (RTM #6, nine days after, .issues/165). The end time now
//! decides too (a day later while the time is TBA), by the same domain rule
//! that closes in-person seats.

/// Over once the organizer marks it completed, or once its end time passes.
/// The rule itself is the domain's, which the worker uses for who may
/// register to watch the recording, so the page and the API agree.
pub fn is_over(status: &str, end_ms: i64, time_tba: bool, now_ms: i64) -> bool {
    event_checkin_domain::models::event::is_over(
        status.eq_ignore_ascii_case("completed"),
        end_ms,
        time_tba,
        now_ms,
    )
}
