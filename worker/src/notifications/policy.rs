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

    /// How long past `due_at` a queued message is still worth delivering.
    ///
    /// Per kind rather than one cutoff because a flat one gets the queue
    /// exactly backwards (`.issues/128`): the backlog that has to die is
    /// months of surveys for finished events, and the rows sharing its age are
    /// the registration confirmations and deposit receipts people are still
    /// waiting on for an event that has not happened yet. Any single number
    /// low enough to catch the first also drops the second.
    pub(super) fn max_age_secs(self) -> i64 {
        match self {
            // Already bounded from both sides: `claim.sql` will not claim a
            // reminder until the start is within 24 h and `reminder_timing`
            // cancels it once the event has begun. The cap only has to survive
            // a cron outage over the window itself.
            Self::Reminder => 2 * 86_400,
            // "How was it" stops being a question and becomes an apology. The
            // answer also gets less useful to the organizer the further it is
            // from the day being remembered. Measured from `due_at`, which for
            // a survey is the moment the organizer opened post-event
            // registration (trigger `notification_post_event_survey`) — not the
            // event's end, so a form opened late still gets its three days.
            Self::Survey => 3 * 86_400,
            // Still literally true right up to the event — the event ending
            // already cancels them (`cancel.sql`) — so this cap is a judgment
            // call, not a correctness one: a confirmation that lands a
            // fortnight after you signed up is no longer confirming anything,
            // and a receipt for a payment made two weeks ago just announces
            // that a system woke up. The cost is real and one-sided: someone
            // who registers more than 14 days before an event that is still
            // upcoming loses their confirmation. `report` is the default mode
            // so that case is counted on a live queue before anything is
            // cancelled.
            Self::Registration | Self::DepositConfirmed | Self::DepositRejected => 14 * 86_400,
        }
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

/// What a dispatch run does with a message that has outlived
/// `NotificationKind::max_age_secs` (`NOTIFICATIONS_STALENESS`).
///
/// Three modes rather than a boolean because the first run of this guard is
/// against a queue that has never drained, and being wrong in either direction
/// is expensive: `Off` works through the whole backlog at
/// `NOTIFICATIONS_MAX_PER_RUN` a day, `Cancel` erases rows that cannot be
/// recovered. `Report` sits between them — it withholds the stale rows from the
/// claim loop and writes down what it would have cancelled, so the numbers can
/// be read off a real run before anything is destroyed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StalenessMode {
    Off,
    Report,
    Cancel,
}

impl StalenessMode {
    /// Parse `NOTIFICATIONS_STALENESS`.
    ///
    /// Unset or unrecognised is `Report`, for the same reason
    /// `DuplicateMode::parse` does it: a typo in the variable name must not
    /// silently disable a safety control, and must not silently start deleting
    /// queued mail either. Mirrors `THB_SLIP_DUPLICATE_MODE` deliberately —
    /// one convention for every operational dial in this Worker.
    pub(super) fn parse(raw: Option<&str>) -> Self {
        match raw.map(str::trim).unwrap_or("") {
            "off" => Self::Off,
            "cancel" => Self::Cancel,
            _ => Self::Report,
        }
    }

    /// The literal `claim.sql` compares `?1` against. Only `Off` is read in
    /// SQL; `Report` and `Cancel` both withhold stale rows from the claim loop
    /// and differ only in what the sweep afterwards does with them.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Report => "report",
            Self::Cancel => "cancel",
        }
    }
}

/// The token every staleness statement carries in place of the age table.
pub(super) const MAX_AGE_TOKEN: &str = "{{max_age_secs}}";

/// `CASE <column> WHEN 'registration' THEN 1209600 ... ELSE 0 END`, in seconds.
///
/// Emitted from `NotificationKind::ALL` rather than written out in the `.sql`
/// files so `max_age_secs` stays the only place the numbers live; a kind added
/// to the enum cannot be left out of the SQL the way `cancel.sql`'s hand-written
/// kind list can (which is why that one needs a drift test and this does not).
///
/// `ELSE 0` covers a row whose `kind` this build cannot parse: it is stale the
/// moment it falls due, which keeps it out of the claim loop that could only
/// cancel it anyway. `column` is a caller-supplied SQL identifier, never input.
pub(super) fn max_age_case_sql(column: &str) -> String {
    let mut sql = format!("CASE {column}");
    for kind in NotificationKind::ALL {
        sql.push_str(&format!(
            " WHEN '{}' THEN {}",
            kind.as_str(),
            kind.max_age_secs()
        ));
    }
    sql.push_str(" ELSE 0 END");
    sql
}

#[cfg(test)]
mod staleness_tests {
    use super::*;

    /// The exact production backlog this guard was written for
    /// (`.issues/128` §3, measured 2026-09-21). Ages are days behind `due_at`
    /// on 2026-09-22; the two columns that matter are which rows survive.
    #[test]
    fn the_measured_backlog_splits_into_rtm6_and_the_dead_surveys() {
        let day = 86_400;
        let survives =
            |kind: NotificationKind, age_days: i64| age_days * day <= kind.max_age_secs();

        // RTM#6 is five days away and nobody has been told anything. Every one
        // of these 62 rows has to go out.
        // 6 days old on 2026-09-22, and still only 11 on the event day itself —
        // the cap has to clear the whole remaining window, not just today.
        for age_days in [6, 11] {
            assert!(
                survives(NotificationKind::Registration, age_days),
                "24 registrations"
            );
            assert!(
                survives(NotificationKind::DepositConfirmed, age_days),
                "14 receipts"
            );
        }
        // Due on the 26th, so never old at all — and claim.sql will not take it
        // before then regardless.
        assert!(survives(NotificationKind::Reminder, -4), "24 reminders");

        // The ~187 surveys for events that ended weeks ago: all of them die.
        for age_days in [8, 9, 30] {
            assert!(
                !survives(NotificationKind::Survey, age_days),
                "a survey {age_days} days late is not a late survey"
            );
        }
    }

    /// A single flat cutoff is what this replaces, and no value of one can do
    /// the job — not because of any particular day's numbers, but because the
    /// two kinds decay against different clocks. A registration confirmation is
    /// true until its event ends, which can be months out; a survey is wrong
    /// days after its event ended. Age alone cannot tell them apart.
    #[test]
    fn no_flat_cutoff_can_separate_the_two_clocks() {
        let day = 86_400;
        // Someone registered for an event six weeks out and the queue never
        // ran. The event has still not happened: the confirmation is not stale,
        // it is merely late, and `cancel.sql` has not touched it.
        let registration_age = 45;
        // A survey for an event that ended four days ago. Nobody wants it.
        let survey_age = 4;

        for cutoff_days in 1..=90 {
            let sends_the_registration = registration_age <= cutoff_days;
            let withholds_the_survey = survey_age > cutoff_days;
            assert!(
                !(sends_the_registration && withholds_the_survey),
                "a flat {cutoff_days}-day cutoff would have worked; this guard need not be per-kind"
            );
        }

        // The per-kind table withholds the survey, as intended. It also drops
        // the 45-day registration — the deliberate cost of capping pre-event
        // kinds at a fortnight, and precisely the case `report` mode exists to
        // surface on a real queue before `cancel` is ever switched on.
        assert!(survey_age * day > NotificationKind::Survey.max_age_secs());
        assert!(registration_age * day > NotificationKind::Registration.max_age_secs());
    }

    #[test]
    fn every_kind_has_a_positive_finite_age_and_reminders_are_the_tightest() {
        for kind in NotificationKind::ALL {
            assert!(
                kind.max_age_secs() > 0,
                "{} would be stale on arrival",
                kind.as_str()
            );
        }
        let tightest = NotificationKind::ALL
            .iter()
            .map(|k| k.max_age_secs())
            .min()
            .expect("ALL is not empty");
        assert_eq!(NotificationKind::Reminder.max_age_secs(), tightest);
    }

    #[test]
    fn an_unrecognised_mode_neither_disables_the_guard_nor_deletes_mail() {
        assert_eq!(StalenessMode::parse(None), StalenessMode::Report);
        for raw in ["", "  ", "Off", "CANCEL", "repot", "1", "true"] {
            assert_eq!(
                StalenessMode::parse(Some(raw)),
                StalenessMode::Report,
                "{raw:?} must fall back to report"
            );
        }
        assert_eq!(StalenessMode::parse(Some(" off ")), StalenessMode::Off);
        assert_eq!(StalenessMode::parse(Some("cancel")), StalenessMode::Cancel);
        // Only `off` is compared in SQL, but a drifting literal would silently
        // leave the guard permanently on.
        assert_eq!(StalenessMode::Off.as_str(), "off");
    }

    #[test]
    fn the_age_table_names_every_kind_exactly_once() {
        let sql = max_age_case_sql("n.kind");
        assert!(sql.starts_with("CASE n.kind WHEN "));
        assert!(sql.ends_with(" ELSE 0 END"));
        for kind in NotificationKind::ALL {
            let arm = format!(" WHEN '{}' THEN {} ", kind.as_str(), kind.max_age_secs());
            assert_eq!(
                sql.matches(arm.trim_end()).count(),
                1,
                "{} is missing from the emitted age table",
                kind.as_str()
            );
        }
    }

    /// Every statement that has to agree on "stale" must carry the token, or it
    /// would ship the literal `{{max_age_secs}}` to D1 and fail at runtime —
    /// on the cron, where nobody is watching.
    #[test]
    fn every_staleness_statement_carries_the_token() {
        for (name, sql) in [
            ("claim.sql", include_str!("sql/claim.sql")),
            ("stale_cancel.sql", include_str!("sql/stale_cancel.sql")),
            ("stale_report.sql", include_str!("sql/stale_report.sql")),
        ] {
            assert_eq!(
                sql.matches(MAX_AGE_TOKEN).count(),
                1,
                "{name} must splice the age table exactly once"
            );
        }
    }
}

/// How many messages one dispatch run will take off the queue.
///
/// This used to be a bare `25` in the claim loop, which made it a real control
/// nobody could see or tune: `.plans/027` B and `.issues/128` both describe the
/// backlog as something that would go out "all at once", and it never could —
/// the cron is daily, so the true worst case was always 25 a day. Naming the
/// number puts the rate where it can be read, and lowering it is the lever for
/// watching a first enable go out slowly.
pub(super) const DEFAULT_MAX_PER_RUN: usize = 25;

/// Upper bound on `NOTIFICATIONS_MAX_PER_RUN`.
///
/// A cap on the cap: a fat-fingered extra zero must not turn one cron into a
/// mailing. Cloudflare's own daily send limit sits well above this, so the
/// binding constraint here is deliberately ours and not the provider's.
const MAX_PER_RUN_CEILING: usize = 100;

/// Parse `NOTIFICATIONS_MAX_PER_RUN`, clamped to `1..=MAX_PER_RUN_CEILING`.
///
/// Unset, unparseable and out-of-range all resolve rather than fail: this
/// number decides throughput, not correctness, and a dispatcher that refuses to
/// run because of a typo in a rate limit is worse than one that runs at 25.
/// Zero clamps up to one — pausing the queue is `NOTIFICATIONS_ENABLED`'s job,
/// and a silently zeroed rate is indistinguishable from a broken cron.
pub(super) fn max_per_run(raw: Option<&str>) -> usize {
    raw.map(str::trim)
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_PER_RUN)
        .clamp(1, MAX_PER_RUN_CEILING)
}

#[cfg(test)]
mod rate_tests {
    use super::*;

    #[test]
    fn an_unusable_value_falls_back_rather_than_stopping_the_queue() {
        for raw in [
            None,
            Some(""),
            Some("  "),
            Some("twenty"),
            Some("-5"),
            Some("2.5"),
        ] {
            assert_eq!(max_per_run(raw), DEFAULT_MAX_PER_RUN, "{raw:?}");
        }
        assert_eq!(max_per_run(Some(" 40 ")), 40);
    }

    #[test]
    fn the_rate_can_be_lowered_to_one_but_never_to_zero_or_to_a_mailing() {
        assert_eq!(max_per_run(Some("0")), 1);
        assert_eq!(max_per_run(Some("1")), 1);
        assert_eq!(max_per_run(Some("100")), MAX_PER_RUN_CEILING);
        assert_eq!(max_per_run(Some("100000")), MAX_PER_RUN_CEILING);
    }

    /// The default has to stay what the hardcoded loop bound was, or upgrading
    /// to this version would silently change production's send rate.
    #[test]
    fn the_default_is_the_rate_the_loop_always_had() {
        assert_eq!(DEFAULT_MAX_PER_RUN, 25);
    }
}
