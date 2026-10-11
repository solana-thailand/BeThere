//! Your record (Super GOAT SG1): the signed-in person's door rows and badges
//! from D1, folded by `event_checkin_domain::models::record::door_history`
//! (the single definition; read its docs for what counts). The SQL only
//! narrows: the person's linked emails (`person_emails_of!`), no staff
//! invitations, events that ran (active or completed) and belong to a
//! campaign. Staff, track and virtual check-ins are decided in Rust.

use event_checkin_domain::models::record::{Badge, DoorRow, MyRecord, door_checkins, door_history};
use serde_json::Value;
use worker::{D1Database, D1Type};

use crate::db::d1_safe::safe_all_rows;

/// The person's attendee rows at door events. `crew`: the row's email is on
/// that event's own staff or organizer list (the delimiter-safe match of
/// `public_stats.sql`).
pub const DOOR_ROWS_SQL: &str = concat!(
    "SELECT e.slug AS slug, e.name AS name, e.event_start_ms AS start_ms, \
       COALESCE(e.event_end_ms, 0) AS end_ms, \
       COALESCE(a.participation_type, '') AS pt, \
       (a.checked_in_at IS NOT NULL AND a.checked_in_at <> '') AS cin, \
       COALESCE(a.checked_in_by, '') AS cin_by, \
       INSTR(',' || REPLACE(LOWER(COALESCE(e.staff_emails, '') || ',' || COALESCE(e.organizer_emails, '')), ' ', '') || ',', \
             ',' || REPLACE(LOWER(a.email), ' ', '') || ',') > 0 AS crew \
     FROM attendees a JOIN events e ON e.id = a.event_id \
     WHERE LOWER(a.email) IN ",
    crate::db::person::person_emails_of!("?1"),
    " AND COALESCE(a.approval_status, '') <> 'invited' \
       AND e.status IN ('active', 'completed') \
       AND EXISTS (SELECT 1 FROM campaign_events ce WHERE ce.event_id = e.id) \
     ORDER BY e.event_start_ms, e.slug"
);

/// Badges the person claimed, at any event.
pub const BADGES_SQL: &str = concat!(
    "SELECT e.slug AS slug, e.name AS name, a.claim_asset_id AS asset_id \
     FROM attendees a JOIN events e ON e.id = a.event_id \
     WHERE LOWER(a.email) IN ",
    crate::db::person::person_emails_of!("?1"),
    " AND COALESCE(a.claim_asset_id, '') <> '' \
     ORDER BY e.event_start_ms, e.slug"
);

fn text(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn flag(row: &Value, key: &str) -> bool {
    row.get(key).and_then(Value::as_i64).unwrap_or(0) != 0
}

/// Rows of `DOOR_ROWS_SQL` as the domain's input.
pub fn door_rows(rows: &[Value]) -> Vec<DoorRow> {
    rows.iter()
        .map(|r| DoorRow {
            event_slug: text(r, "slug"),
            event_name: text(r, "name"),
            start_ms: r.get("start_ms").and_then(Value::as_i64).unwrap_or(0),
            end_ms: r.get("end_ms").and_then(Value::as_i64).unwrap_or(0),
            participation_type: text(r, "pt"),
            checked_in: flag(r, "cin"),
            checked_in_by: text(r, "cin_by"),
            crew: flag(r, "crew"),
        })
        .collect()
}

/// Rows of `BADGES_SQL`, one per event (two linked emails, one badge each, list once).
pub fn badges(rows: &[Value]) -> Vec<Badge> {
    let mut out: Vec<Badge> = Vec::new();
    for r in rows {
        let badge = Badge {
            event_slug: text(r, "slug"),
            event_name: text(r, "name"),
            asset_id: text(r, "asset_id"),
        };
        if !out.iter().any(|b| b.event_slug == badge.event_slug) {
            out.push(badge);
        }
    }
    out
}

async fn all(db: &D1Database, sql: &str, email: &str) -> Result<Vec<Value>, String> {
    let stmt = db
        .prepare(sql)
        .bind_refs(&[D1Type::Text(email)])
        .map_err(|e| format!("D1 record bind: {e:?}"))?;
    safe_all_rows(&stmt).await
}

/// The record of the person `email` (lowercased by the caller) belongs to.
pub async fn my_record(db: &D1Database, email: &str) -> Result<MyRecord, String> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let events = door_history(&door_rows(&all(db, DOOR_ROWS_SQL, email).await?), now_ms);
    let badges = badges(&all(db, BADGES_SQL, email).await?);
    Ok(MyRecord {
        door_checkins: door_checkins(&events),
        events,
        badges,
        badge_cluster: String::new(),
    })
}
