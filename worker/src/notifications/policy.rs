//! Pure retry and scheduling decisions, independent of D1 and transport.
pub(super) fn failure_state(code: &str, attempts: i32) -> &'static str {
    match code {
        "E_RATE_LIMIT_EXCEEDED" | "E_DAILY_LIMIT_EXCEEDED" => {
            if attempts < 5 {
                "pending"
            } else {
                "failed"
            }
        }
        "E_VALIDATION_ERROR"
        | "E_FIELD_MISSING"
        | "E_TOO_MANY_RECIPIENTS"
        | "E_SENDER_NOT_VERIFIED"
        | "E_RECIPIENT_NOT_ALLOWED"
        | "E_RECIPIENT_SUPPRESSED"
        | "E_SENDER_DOMAIN_NOT_AVAILABLE"
        | "E_CONTENT_TOO_LARGE" => "failed",
        _ => "uncertain",
    }
}
pub(super) fn retry_delay(attempts: i32) -> i32 {
    300 * 2_i32.pow(attempts.clamp(1, 5) as u32 - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uncertain_delivery_is_never_replayed() {
        for code in [
            "E_INTERNAL_SERVER_ERROR",
            "E_DELIVERY_FAILED",
            "UNKNOWN",
            "INTERRUPTED_SEND",
        ] {
            assert_eq!(failure_state(code, 1), "uncertain");
        }
        assert_eq!(failure_state("E_RATE_LIMIT_EXCEEDED", 1), "pending");
        assert_eq!(failure_state("E_RATE_LIMIT_EXCEEDED", 5), "failed");
        assert_eq!(failure_state("E_RECIPIENT_SUPPRESSED", 1), "failed");
    }
    #[test]
    fn bounded_backoff() {
        assert_eq!(retry_delay(1), 300);
        assert_eq!(retry_delay(5), 4800);
        assert_eq!(retry_delay(99), 4800);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ReminderTiming {
    Send,
    Cancel,
    Defer(i64),
}

/// Re-check timing after claim: an organizer may change dates while the job is in flight.
pub(super) fn reminder_timing(time_tba: bool, start_ms: i64, now_ms: i64) -> ReminderTiming {
    if time_tba {
        return ReminderTiming::Defer(now_ms.div_euclid(1000).saturating_add(3600));
    }
    if start_ms <= now_ms {
        return ReminderTiming::Cancel;
    }
    if start_ms.saturating_sub(now_ms) > 86_400_000 {
        // Round UP so a fractional-second boundary cannot create a hot re-claim loop.
        let start_seconds = start_ms.div_euclid(1000) + i64::from(start_ms.rem_euclid(1000) != 0);
        return ReminderTiming::Defer(start_seconds - 86400);
    }
    ReminderTiming::Send
}

#[cfg(test)]
mod schedule_tests {
    use super::*;
    #[test]
    fn moved_event_defers_without_losing_reminder() {
        assert_eq!(
            reminder_timing(false, 172_800_000, 0),
            ReminderTiming::Defer(86400)
        );
        assert_eq!(
            reminder_timing(false, 86_400_001, 0),
            ReminderTiming::Defer(1)
        );
    }
    #[test]
    fn tba_does_not_permanently_cancel_a_claimed_reminder() {
        assert_eq!(reminder_timing(true, 0, 1000), ReminderTiming::Defer(3601));
    }
    #[test]
    fn started_events_cancel_and_twenty_four_hour_boundary_sends() {
        assert_eq!(reminder_timing(false, 1000, 1000), ReminderTiming::Cancel);
        assert_eq!(reminder_timing(false, 999, 1000), ReminderTiming::Cancel);
        assert_eq!(
            reminder_timing(false, 86_401_000, 1000),
            ReminderTiming::Send
        );
    }
}
