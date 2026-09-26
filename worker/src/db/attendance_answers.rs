//! `attendance_answers` — the staff-recorded "can you still come?" answer per
//! registrant (migration 0052).

use std::collections::HashMap;

use event_checkin_domain::models::attendee::{AttendanceAnswer, ParticipationType};
use event_checkin_domain::models::deposit::RefundQueueContext;
use worker::D1Database;

use worker::d1::D1Type;

/// Record `answer` for an attendee of `event_id`, or clear it with `None`.
///
/// The write goes through `SELECT … FROM attendees`, so it only lands when the
/// attendee really belongs to the event. Returns whether a row was written
/// (or removed); `false` means no such attendee on this event.
pub async fn set(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
    answer: Option<AttendanceAnswer>,
    updated_by: &str,
) -> Result<bool, String> {
    let stmt = match answer {
        Some(answer) => db
            .prepare(include_str!("sql/attendance_answer_set.sql"))
            .bind_refs(&[
                D1Type::Text(event_id),
                D1Type::Text(attendee_id),
                D1Type::Text(answer.as_str()),
                D1Type::Text(updated_by),
            ]),
        None => db
            .prepare(include_str!("sql/attendance_answer_clear.sql"))
            .bind_refs(&[D1Type::Text(event_id), D1Type::Text(attendee_id)]),
    }
    .map_err(|e| format!("D1 attendance_answers set bind: {e:?}"))?;

    let result = stmt
        .run()
        .await
        .map_err(|e| format!("D1 attendance_answers set run: {e:?}"))?;
    let changes = result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0);
    Ok(changes > 0)
}

/// Every recorded answer on `event_id`, keyed by attendee id. Attendees with
/// no answer are absent. Rows whose value is not a known answer are skipped.
pub async fn by_event(
    db: &D1Database,
    event_id: &str,
) -> Result<HashMap<String, AttendanceAnswer>, String> {
    let stmt = db
        .prepare("SELECT attendee_id, answer FROM attendance_answers WHERE event_id = ?1")
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 attendance_answers by_event bind: {e:?}"))?;
    let rows = super::d1_safe::safe_all_rows(&stmt)
        .await
        .map_err(|e| format!("D1 attendance_answers by_event: {e}"))?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let id = row.get("attendee_id")?.as_str()?.to_string();
            let answer = AttendanceAnswer::parse(row.get("answer")?.as_str()?)?;
            Some((id, answer))
        })
        .collect())
}

/// Participation, check-in and answer for every attendee of `event_id`, for
/// narrowing the refund queue. One query; attendees with no answer carry
/// `attendance_answer: None`.
pub async fn refund_context_by_attendee(
    db: &D1Database,
    event_id: &str,
) -> Result<HashMap<String, RefundQueueContext>, String> {
    let stmt = db
        .prepare(include_str!("sql/refund_queue_context.sql"))
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 refund_context bind: {e:?}"))?;
    let rows = super::d1_safe::safe_all_rows(&stmt)
        .await
        .map_err(|e| format!("D1 refund_context: {e}"))?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let id = row.get("attendee_id")?.as_str()?.to_string();
            let context = RefundQueueContext {
                participation_type: ParticipationType::parse(
                    row.get("participation_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default(),
                ),
                checked_in: row
                    .get("checked_in")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0)
                    != 0,
                attendance_answer: row
                    .get("answer")
                    .and_then(|v| v.as_str())
                    .and_then(AttendanceAnswer::parse),
            };
            Some((id, context))
        })
        .collect())
}
