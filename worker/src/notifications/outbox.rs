//! D1 persistence for notification history and lifecycle transitions.
use super::{InboxNotification, Notification};
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
        let kind = text(&row, "kind")?;
        let event_name = text(&row, "event_name")?;
        let event_id = text(&row, "event_id")?;
        let attendee_id = text(&row, "attendee_id")?;
        let deposit_status = text(&row, "deposit_status")?;
        let deposit_enabled = integer(&row, "deposit_enabled")? != 0;
        let deposit_verified = integer(&row, "deposit_verified")? != 0;
        let participation_type = text(&row, "participation_type")?;
        let (title, body, action_label) = presentation(&kind, &event_name);
        let deposit_needed = needs_deposit(
            deposit_enabled,
            &participation_type,
            deposit_verified,
            &deposit_status,
        );
        let action_is_deposit = kind != "deposit_confirmed" && deposit_needed;
        let attendee_path = urlencoding::encode(&attendee_id);
        let event_query = urlencoding::encode(&event_id);
        let action_url = if action_is_deposit {
            format!("/deposit/{attendee_path}?event_id={event_query}")
        } else {
            format!("/ticket/{attendee_path}?event_id={event_query}")
        };
        items.push(InboxNotification {
            id,
            kind,
            event_name,
            title: title.into(),
            body,
            action_url,
            action_label: if action_is_deposit {
                "Complete deposit".into()
            } else {
                action_label.into()
            },
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

fn presentation(kind: &str, event: &str) -> (&'static str, String, &'static str) {
    match kind {
        "reminder" => (
            "Event reminder",
            format!("{event} starts soon."),
            "View ticket",
        ),
        "deposit_confirmed" => (
            "Deposit confirmed",
            format!("Your deposit for {event} is confirmed."),
            "View ticket",
        ),
        "deposit_rejected" => (
            "Deposit needs attention",
            format!("Your payment slip for {event} needs attention."),
            "Review deposit",
        ),
        _ => (
            "Registration saved",
            format!("Your registration for {event} is saved."),
            "View ticket",
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
    use super::needs_deposit;

    #[test]
    fn deposit_action_uses_current_state_and_skips_online_attendees() {
        assert!(needs_deposit(true, "in_person", false, "none"));
        assert!(!needs_deposit(true, "online", false, "none"));
        assert!(!needs_deposit(true, "retrospective", false, "none"));
        assert!(!needs_deposit(true, "in_person", true, "none"));
        assert!(!needs_deposit(true, "in_person", false, "held_as_credit"));
        assert!(!needs_deposit(false, "in_person", false, "none"));
    }
}
