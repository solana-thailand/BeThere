//! D1 persistence for notification history and lifecycle transitions.
use super::{InboxNotification, Notification, policy::NotificationKind};
use crate::db::d1_safe::safe_all_rows;
use event_checkin_domain::models::attendee::ParticipationType;
use worker::{D1Database, d1::D1Type};

pub async fn list(
    db: &D1Database,
    event_id: &str,
    before: i64,
) -> Result<Vec<Notification>, String> {
    let stmt = db.prepare("SELECT n.*,a.name AS recipient_name,a.email AS recipient_email FROM notification_outbox n LEFT JOIN attendees a ON a.id=n.attendee_id AND a.event_id=n.event_id WHERE n.event_id=?1 AND n.id<?2 ORDER BY n.id DESC LIMIT 50")
        .bind_refs(&[D1Type::Text(event_id),D1Type::Real(before as f64)]).map_err(|e|e.to_string())?;
    safe_all_rows(&stmt)
        .await?
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
        .collect()
}

pub async fn retry(db: &D1Database, event_id: &str, id: i64) -> Result<bool, String> {
    // Uncertain/accepted sends cannot be retried from this API: avoid duplicate delivery.
    let stmt = db
        .prepare(include_str!("sql/retry.sql"))
        .bind_refs(&[D1Type::Text(event_id), D1Type::Real(id as f64)])
        .map_err(|e| e.to_string())?;
    Ok(!safe_all_rows(&stmt).await?.is_empty())
}

pub async fn list_for_attendee(
    db: &D1Database,
    email: &str,
    before: i64,
) -> Result<(Vec<InboxNotification>, i64), String> {
    let params = [D1Type::Text(email), D1Type::Real(before as f64)];
    let stmt = db
        .prepare(include_str!("sql/inbox_list.sql"))
        .bind_refs(&params)
        .map_err(|e| e.to_string())?;
    let rows = safe_all_rows(&stmt).await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let id = integer(&row, "id")?;
        let kind_column = text(&row, "kind")?;
        // The `kind` CHECK keeps this total; a row from a newer deploy is
        // skipped rather than shown under some other kind's wording.
        let Some(kind) = NotificationKind::parse(&kind_column) else {
            tracing::warn!(notification_id = id, "inbox row has an unknown kind");
            continue;
        };
        let event_name = text(&row, "event_name")?;
        let event_id = text(&row, "event_id")?;
        let attendee_id = text(&row, "attendee_id")?;
        let deposit_status = text(&row, "deposit_status")?;
        let deposit_enabled = integer(&row, "deposit_enabled")? != 0;
        let deposit_verified = integer(&row, "deposit_verified")? != 0;
        let participation_type = text(&row, "participation_type")?;
        let (title, body, action_label) = presentation(kind, &event_name);
        let deposit_needed = needs_deposit(
            deposit_enabled,
            &participation_type,
            deposit_verified,
            &deposit_status,
        );
        let event_slug = text(&row, "slug")?;
        let (action_url, action_label) =
            inbox_action(kind, deposit_needed, &attendee_id, &event_id, action_label);
        items.push(InboxNotification {
            id,
            kind: kind.as_str().to_string(),
            event_name,
            event_slug,
            participation_type: participation_type.clone(),
            title: title.into(),
            body,
            action_url,
            action_label,
            due_at: integer(&row, "due_at")?,
            read_at: optional_integer(&row, "read_at"),
        });
    }
    let count = db
        .prepare(include_str!("sql/inbox_unread.sql"))
        .bind_refs(&[D1Type::Text(email)])
        .map_err(|e| e.to_string())?;
    let unread = safe_all_rows(&count)
        .await?
        .first()
        .map(|v| integer(v, "count"))
        .transpose()?
        .unwrap_or(0);
    Ok((items, unread))
}

fn needs_deposit(
    deposit_enabled: bool,
    participation_type: &str,
    deposit_verified: bool,
    legacy_status: &str,
) -> bool {
    deposit_enabled
        && matches!(
            ParticipationType::parse(participation_type),
            ParticipationType::InPerson
        )
        && !deposit_verified
        && !matches!(
            legacy_status,
            "confirmed" | "verified" | "held_as_credit" | "refunded"
        )
}

pub async fn mark_read(db: &D1Database, email: &str, id: i64) -> Result<bool, String> {
    let stmt = db
        .prepare(include_str!("sql/inbox_read.sql"))
        .bind_refs(&[D1Type::Real(id as f64), D1Type::Text(email)])
        .map_err(|e| e.to_string())?;
    Ok(!safe_all_rows(&stmt).await?.is_empty())
}

pub async fn mark_all_read(db: &D1Database, email: &str) -> Result<(), String> {
    db.prepare(include_str!("sql/inbox_read_all.sql"))
        .bind_refs(&[D1Type::Text(email)])
        .map_err(|e| e.to_string())?
        .run()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Where the inbox entry's button goes. Named so the choice cannot collapse
/// back into a boolean the day a third destination appears.
#[derive(Clone, Copy)]
enum Action {
    Ticket,
    Deposit,
    Feedback,
}

/// Where an inbox row's button sends the reader, and what it says.
///
/// Pulled out of `list_for_attendee` so it can be tested: that function needs a
/// live `D1Database`, so for as long as this decision lived inside it the only
/// coverage of the survey's destination was the *email* renderer's test in
/// `content.rs`. The two are separate code paths that have to agree, which is
/// exactly the shape of defect `.issues/086` and the `duplicated-state-transition-paths`
/// note are about.
fn inbox_action(
    kind: NotificationKind,
    deposit_needed: bool,
    attendee_id: &str,
    event_id: &str,
    action_label: &'static str,
) -> (String, String) {
    // Mirrors `content::render`: the survey asks its questions elsewhere, and an
    // outstanding deposit cannot divert a message about an event that has
    // already happened.
    let action = match kind {
        NotificationKind::Survey => Action::Feedback,
        NotificationKind::DepositConfirmed => Action::Ticket,
        _ if deposit_needed => Action::Deposit,
        _ => Action::Ticket,
    };
    let attendee_path = urlencoding::encode(attendee_id);
    let event_query = urlencoding::encode(event_id);
    match action {
        // `/feedback`, not the single-event form: a person with three
        // outstanding surveys should answer them on one page rather than follow
        // three links (`.issues/091`). The per-event route stays live for the QR
        // codes printed on the recap posters, but no notification points at it.
        Action::Feedback => ("/feedback".to_string(), action_label.to_string()),
        Action::Deposit => (
            format!("/deposit/{attendee_path}?event_id={event_query}"),
            "Complete deposit".to_string(),
        ),
        Action::Ticket => (
            format!("/ticket/{attendee_path}?event_id={event_query}"),
            action_label.to_string(),
        ),
    }
}

fn presentation(kind: NotificationKind, event: &str) -> (&'static str, String, &'static str) {
    match kind {
        NotificationKind::Registration => (
            "Registration saved",
            format!("Your registration for {event} is saved."),
            "View ticket",
        ),
        NotificationKind::Reminder => (
            "Event reminder",
            format!("{event} starts soon."),
            "View ticket",
        ),
        NotificationKind::DepositConfirmed => (
            "Deposit confirmed",
            format!("Your deposit for {event} is confirmed."),
            "View ticket",
        ),
        NotificationKind::DepositRejected => (
            "Deposit needs attention",
            format!("Your payment slip for {event} needs attention."),
            "Review deposit",
        ),
        NotificationKind::Survey => (
            "How was the event?",
            format!("Four quick questions about {event}."),
            "Answer the questions",
        ),
    }
}

fn text(value: &serde_json::Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("invalid notification field: {key}"))
}
fn integer(value: &serde_json::Value, key: &str) -> Result<i64, String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| format!("invalid notification field: {key}"))
}
fn optional_integer(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(serde_json::Value::as_i64)
}

pub(super) async fn run_sql(db: &D1Database, sql: &str) -> Result<(), String> {
    db.prepare(sql).run().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{NotificationKind, inbox_action, needs_deposit, presentation};

    /// The kind used to arrive as a string with a catch-all arm, so a `survey`
    /// row would have been shown to the attendee as "Registration saved".
    #[test]
    fn every_kind_has_its_own_wording() {
        let mut titles: Vec<&str> = NotificationKind::ALL
            .iter()
            .map(|k| presentation(*k, "Demo Day").0)
            .collect();
        let total = titles.len();
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(titles.len(), total, "two kinds share a title: {titles:?}");
    }

    #[test]
    fn deposit_action_uses_current_state_and_skips_online_attendees() {
        assert!(needs_deposit(true, "in_person", false, "none"));
        assert!(!needs_deposit(true, "online", false, "none"));
        assert!(!needs_deposit(true, "retrospective", false, "none"));
        assert!(!needs_deposit(true, "in_person", true, "none"));
        assert!(!needs_deposit(true, "in_person", false, "held_as_credit"));
        assert!(!needs_deposit(false, "in_person", false, "none"));
    }

    /// The survey's inbox button must reach the combined page, and must never
    /// reach the single-event form — that route still exists for the recap QR
    /// codes, and one message per event attended is the thing `/feedback` was
    /// built to stop (`.issues/091`).
    #[test]
    fn survey_inbox_button_points_at_the_combined_feedback_page() {
        let (url, label) = inbox_action(
            NotificationKind::Survey,
            true, // an unpaid deposit must not hijack a post-event message
            "att-1",
            "evt-1",
            "Share your feedback",
        );
        assert_eq!(url, "/feedback");
        assert_eq!(label, "Share your feedback");
    }

    /// The same decision the email renderer makes, asserted on the inbox path so
    /// the two cannot drift apart silently.
    #[test]
    fn every_other_kind_keeps_its_own_destination() {
        let ticket = inbox_action(NotificationKind::Reminder, false, "att-1", "evt-1", "View");
        assert!(ticket.0.starts_with("/ticket/att-1?event_id=evt-1"));

        let deposit = inbox_action(NotificationKind::Reminder, true, "att-1", "evt-1", "View");
        assert!(deposit.0.starts_with("/deposit/att-1?event_id=evt-1"));
        assert_eq!(deposit.1, "Complete deposit");

        // A confirmed deposit goes to the ticket even while `deposit_needed`
        // is still true — the arm order in `inbox_action` is load-bearing.
        let confirmed = inbox_action(
            NotificationKind::DepositConfirmed,
            true,
            "att-1",
            "evt-1",
            "View",
        );
        assert!(confirmed.0.starts_with("/ticket/"));
    }

    /// Ids reach the URL escaped, not raw.
    #[test]
    fn action_urls_encode_their_ids() {
        let (url, _) = inbox_action(NotificationKind::Reminder, false, "att/1", "evt 1", "View");
        assert_eq!(url, "/ticket/att%2F1?event_id=evt%201");
    }

    /// The feedback page asks a different question set per participation type,
    /// so the inbox payload has to carry it — the view always selected it, it
    /// was only ever used to decide whether a deposit was outstanding
    /// (`.issues/098`).
    #[test]
    fn deposit_logic_and_the_exposed_participation_type_do_not_share_a_meaning() {
        // `needs_deposit` narrows to in-person; the field the page reads must
        // stay the raw value, including the ones that never owe a deposit.
        assert!(!needs_deposit(true, "online", false, "none"));
        assert!(!needs_deposit(true, "retrospective", false, "none"));
        assert!(needs_deposit(true, "in_person", false, "none"));
    }
}
