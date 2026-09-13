//! Pure retry, scheduling and delivery-window decisions, independent of D1 and transport.
use event_checkin_domain::models::event::EventStatus;
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

/// Every message the outbox can carry. `notification_outbox.kind` CHECKs the
/// same set (migration 0035), and `sql/cancel.sql` partitions it by window —
/// `cancel_sql_covers_every_pre_event_kind` holds the two in step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NotificationKind {
    Registration,
    Reminder,
    DepositConfirmed,
    DepositRejected,
    Survey,
}

/// Which side of the event's end a kind is allowed to be delivered on.
///
/// This used to be a pipeline invariant rather than a per-kind property: every
/// kind was pre-event, so `prepare` simply cancelled anything queued for a
/// finished event. That made a whole class of message unreachable
/// (`.issues/080`) — the survey has to wait for the event to be over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeliveryWindow {
    BeforeEventEnds,
    AfterEventEnds,
}

impl NotificationKind {
    #[cfg(test)]
    pub(super) const ALL: [Self; 5] = [
        Self::Registration,
        Self::Reminder,
        Self::DepositConfirmed,
        Self::DepositRejected,
        Self::Survey,
    ];

    pub(super) fn parse(kind: &str) -> Option<Self> {
        match kind {
            "registration" => Some(Self::Registration),
            "reminder" => Some(Self::Reminder),
            "deposit_confirmed" => Some(Self::DepositConfirmed),
            "deposit_rejected" => Some(Self::DepositRejected),
            "survey" => Some(Self::Survey),
            _ => None,
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Registration => "registration",
            Self::Reminder => "reminder",
            Self::DepositConfirmed => "deposit_confirmed",
            Self::DepositRejected => "deposit_rejected",
            Self::Survey => "survey",
        }
    }

    pub(super) fn delivery_window(self) -> DeliveryWindow {
        match self {
            Self::Registration
            | Self::Reminder
            | Self::DepositConfirmed
            | Self::DepositRejected => DeliveryWindow::BeforeEventEnds,
            Self::Survey => DeliveryWindow::AfterEventEnds,
        }
    }

    /// Whether the kind may only be delivered to someone who actually attended.
    /// Asking a no-show how the event went is worse than not asking.
    pub(super) fn requires_check_in(self) -> bool {
        matches!(self, Self::Survey)
    }
}

/// Is the event in the right part of its life for this window?
///
/// `Completed` counts as ended whatever `event_end_ms` says: the organizer has
/// declared it over, and post-event registration (the survey's carrier) can
/// only be opened on a Completed event. `event_end_ms == 0` means no end was
/// ever recorded, so such an event never demonstrably finishes and an
/// after-the-event message never becomes deliverable.
pub(super) fn in_delivery_window(
    window: DeliveryWindow,
    status: &EventStatus,
    event_end_ms: i64,
    now_ms: i64,
) -> bool {
    let ended = *status == EventStatus::Completed || (event_end_ms > 0 && event_end_ms <= now_ms);
    match window {
        DeliveryWindow::BeforeEventEnds => *status == EventStatus::Active && !ended,
        DeliveryWindow::AfterEventEnds => {
            matches!(status, EventStatus::Active | EventStatus::Completed) && ended
        }
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;
    const BEFORE: DeliveryWindow = DeliveryWindow::BeforeEventEnds;
    const AFTER: DeliveryWindow = DeliveryWindow::AfterEventEnds;

    #[test]
    fn windows_are_opposites_over_a_live_events_lifetime() {
        // Active event, end an hour away: pre-event kinds send, the survey waits.
        assert!(in_delivery_window(
            BEFORE,
            &EventStatus::Active,
            3_600_000,
            0
        ));
        assert!(!in_delivery_window(
            AFTER,
            &EventStatus::Active,
            3_600_000,
            0
        ));
        // Same event once it has finished: exactly the other way round.
        assert!(!in_delivery_window(
            BEFORE,
            &EventStatus::Active,
            3_600_000,
            3_600_000
        ));
        assert!(in_delivery_window(
            AFTER,
            &EventStatus::Active,
            3_600_000,
            3_600_000
        ));
    }

    #[test]
    fn a_completed_event_is_over_even_with_an_end_in_the_future() {
        assert!(in_delivery_window(
            AFTER,
            &EventStatus::Completed,
            i64::MAX,
            0
        ));
        assert!(!in_delivery_window(
            BEFORE,
            &EventStatus::Completed,
            i64::MAX,
            0
        ));
    }

    #[test]
    fn an_event_with_no_recorded_end_never_becomes_post_event() {
        assert!(in_delivery_window(BEFORE, &EventStatus::Active, 0, 0));
        assert!(!in_delivery_window(
            AFTER,
            &EventStatus::Active,
            0,
            i64::MAX
        ));
    }

    #[test]
    fn draft_and_archived_events_deliver_nothing_in_either_direction() {
        for status in [EventStatus::Draft, EventStatus::Archived] {
            for window in [BEFORE, AFTER] {
                assert!(!in_delivery_window(window, &status, 1, 0));
                assert!(!in_delivery_window(window, &status, 1, 2));
            }
        }
    }

    #[test]
    fn only_the_survey_waits_for_the_event_and_demands_attendance() {
        for kind in NotificationKind::ALL {
            assert_eq!(
                kind.delivery_window() == DeliveryWindow::AfterEventEnds,
                kind == NotificationKind::Survey,
                "{} is on the wrong side of the event",
                kind.as_str()
            );
            assert_eq!(kind.requires_check_in(), kind == NotificationKind::Survey);
            assert_eq!(NotificationKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(NotificationKind::parse("marketing"), None);
    }

    /// `cancel.sql` bulk-cancels queued rows for events that can no longer
    /// receive them, and has to split the two windows in SQL. That list is a
    /// copy of `delivery_window`, so it can drift: a kind added to the enum but
    /// not to the SQL would be treated as post-event by the bulk cancel and
    /// pre-event by `prepare`. Read the literals back out of the statement
    /// rather than trusting a comment.
    #[test]
    fn cancel_sql_covers_every_pre_event_kind() {
        let sql = include_str!("sql/cancel.sql");
        let (_, rest) = sql
            .split_once("notification_outbox.kind IN (")
            .expect("cancel.sql no longer partitions rows by kind");
        let (list, _) = rest.split_once(')').expect("unterminated kind list");
        let in_sql: Vec<&str> = list
            .split(',')
            .map(|item| item.trim().trim_matches('\''))
            .collect();

        let expected: Vec<&str> = NotificationKind::ALL
            .iter()
            .filter(|k| k.delivery_window() == DeliveryWindow::BeforeEventEnds)
            .map(|k| k.as_str())
            .collect();
        assert_eq!(
            in_sql, expected,
            "cancel.sql kind list drifted from DeliveryWindow"
        );
        assert!(
            !in_sql.contains(&NotificationKind::Survey.as_str()),
            "a post-event kind must not be cancelled for having outlived the event"
        );
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
