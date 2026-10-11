//! Regular rules (Super GOAT SG1b): an event's opt-in rule, read at
//! registration and edited in event settings. Who counts and how is
//! `door_history` alone (`crate::record`); this module stores the rule and
//! counts the qualifying registrants with the same fold.

use event_checkin_domain::models::record::{DoorRow, RegularRule, door_checkins, door_history};
use serde_json::Value;
use worker::{D1Database, D1Type};

use crate::db::d1_safe::safe_all_rows;

pub const RULE_SQL: &str =
    "SELECT min_door_checkins, deposit_thb FROM event_regular_rules WHERE event_id = ?1";
/// ?1 event, ?2 min, ?3 deposit, ?4 now.
pub const UPSERT_SQL: &str = "INSERT INTO event_regular_rules \
     (event_id, min_door_checkins, deposit_thb, updated_at) VALUES (?1, ?2, ?3, ?4) \
     ON CONFLICT(event_id) DO UPDATE SET min_door_checkins = excluded.min_door_checkins, \
     deposit_thb = excluded.deposit_thb, updated_at = excluded.updated_at";
pub const DELETE_SQL: &str = "DELETE FROM event_regular_rules WHERE event_id = ?1";

/// The door rows of every registrant of ?1 (each registrant's linked emails
/// included), tagged with the registrant's email in `reg`. Same columns and
/// filters as `record::DOOR_ROWS_SQL`; the fold is Rust's.
pub const REGISTRANT_DOOR_ROWS_SQL: &str = "WITH reg AS ( \
       SELECT DISTINCT LOWER(email) AS email FROM attendees \
       WHERE event_id = ?1 AND COALESCE(approval_status, '') <> 'invited' \
         AND COALESCE(email, '') <> ''), \
     person AS ( \
       SELECT r.email AS reg, r.email AS email FROM reg r \
       UNION \
       SELECT r.email, m.email FROM reg r \
         JOIN person_emails o ON o.email = r.email \
         JOIN person_emails m ON m.person_id = o.person_id) \
     SELECT p.reg AS reg, e.slug AS slug, e.name AS name, e.event_start_ms AS start_ms, \
       COALESCE(e.event_end_ms, 0) AS end_ms, \
       COALESCE(a.participation_type, '') AS pt, \
       (a.checked_in_at IS NOT NULL AND a.checked_in_at <> '') AS cin, \
       COALESCE(a.checked_in_by, '') AS cin_by, \
       INSTR(',' || REPLACE(LOWER(COALESCE(e.staff_emails, '') || ',' || COALESCE(e.organizer_emails, '')), ' ', '') || ',', \
             ',' || REPLACE(LOWER(a.email), ' ', '') || ',') > 0 AS crew \
     FROM person p \
     JOIN attendees a ON LOWER(a.email) = p.email \
     JOIN events e ON e.id = a.event_id \
     WHERE COALESCE(a.approval_status, '') <> 'invited' \
       AND e.status IN ('active', 'completed') \
       AND EXISTS (SELECT 1 FROM campaign_events ce WHERE ce.event_id = e.id)";

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

async fn rows(db: &D1Database, sql: &str, binds: &[D1Type<'_>]) -> Result<Vec<Value>, String> {
    let stmt = db
        .prepare(sql)
        .bind_refs(binds)
        .map_err(|e| format!("D1 regular bind: {e:?}"))?;
    safe_all_rows(&stmt).await
}

/// The event's rule, if it has one.
pub async fn rule(db: &D1Database, event_id: &str) -> Result<Option<RegularRule>, String> {
    let found = rows(db, RULE_SQL, &[D1Type::Text(event_id)]).await?;
    Ok(found.first().map(|r| RegularRule {
        min_door_checkins: r
            .get("min_door_checkins")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0),
        deposit_thb: r.get("deposit_thb").and_then(Value::as_u64).unwrap_or(0),
    }))
}

/// Save (`Some`, already validated by the caller) or remove (`None`) the rule.
pub async fn save(
    db: &D1Database,
    event_id: &str,
    rule: Option<RegularRule>,
) -> Result<(), String> {
    let now = now_iso();
    let min = rule.map_or(0, |r| r.min_door_checkins as i32);
    let deposit = rule.map_or(0, |r| r.deposit_thb.min(i32::MAX as u64) as i32);
    let stmt = match rule {
        Some(_) => db.prepare(UPSERT_SQL).bind_refs(&[
            D1Type::Text(event_id),
            D1Type::Integer(min),
            D1Type::Integer(deposit),
            D1Type::Text(&now),
        ]),
        None => db.prepare(DELETE_SQL).bind_refs(&[D1Type::Text(event_id)]),
    }
    .map_err(|e| format!("D1 regular bind: {e:?}"))?;
    stmt.run()
        .await
        .map_err(|e| format!("D1 regular write: {e:?}"))?;
    Ok(())
}

/// How many registrants of the event have at least `min` door check-ins,
/// each counted by `door_history` over their own linked emails.
pub fn count_qualifying(rows: &[Value], min: u32, now_ms: i64) -> u32 {
    let mut by_reg: Vec<(String, Vec<DoorRow>)> = Vec::new();
    let text = |r: &Value, k: &str| {
        r.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    for r in rows {
        let reg = text(r, "reg");
        let door = crate::record::door_rows(std::slice::from_ref(r))
            .into_iter()
            .next()
            .unwrap_or_default();
        match by_reg.iter_mut().find(|(e, _)| *e == reg) {
            Some((_, list)) => list.push(door),
            None => by_reg.push((reg, vec![door])),
        }
    }
    let n = by_reg
        .iter()
        .filter(|(_, list)| door_checkins(&door_history(list, now_ms)) >= min)
        .count();
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Registrants of `event_id` who qualify for `min` door check-ins.
pub async fn qualifying(db: &D1Database, event_id: &str, min: u32) -> Result<u32, String> {
    let found = rows(db, REGISTRANT_DOOR_ROWS_SQL, &[D1Type::Text(event_id)]).await?;
    Ok(count_qualifying(
        &found,
        min,
        chrono::Utc::now().timestamp_millis(),
    ))
}

/// Whether `email` (lowercased) holds a seat without a deposit at
/// `event_id` under its rule. `false` when there is no rule, or on any read
/// failure: a missed waiver costs the person a deposit they get back; a wrong
/// one costs the organizer a no-show.
pub async fn waives(db: &D1Database, event_id: &str, email: &str) -> bool {
    let Ok(Some(rule)) = rule(db, event_id).await else {
        return false;
    };
    match crate::record::my_record(db, email).await {
        Ok(record) => rule.qualifies(record.door_checkins),
        Err(e) => {
            tracing::warn!(error = %e, "regular rule: record read failed, no waiver");
            false
        }
    }
}
