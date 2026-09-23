//! Locate events that list an email in `staff_emails` (plan 028 W2).
//!
//! The result is a candidate list, not a grant: KV holds the authoritative
//! event config, and a D1 row can outlive a failed KV-side delete or update.
//! Callers must confirm each id against the KV config before granting access.

use worker::D1Database;
use worker::d1::D1Type;

/// Upper bound on candidates read per lookup. One confirmed hit is enough, so
/// this only caps the work a pathological staff list can cause.
pub const STAFF_CANDIDATE_LIMIT: i32 = 20;

/// Normalise an email the way the SQL predicate compares it: the column is
/// lowercased with spaces stripped, so the needle must be too.
pub fn staff_needle(email: &str) -> String {
    email.trim().to_lowercase().replace(' ', "")
}

/// Ids of events whose `staff_emails` contains `email`, newest first.
pub async fn event_ids_for_staff(db: &D1Database, email: &str) -> Result<Vec<String>, String> {
    let needle = staff_needle(email);
    if needle.is_empty() || needle.contains(',') {
        return Ok(Vec::new());
    }
    let stmt = db
        .prepare(include_str!("sql/event_ids_for_staff.sql"))
        .bind_refs(&[
            D1Type::Text(&needle),
            D1Type::Integer(STAFF_CANDIDATE_LIMIT),
        ])
        .map_err(|e| format!("D1 event_ids_for_staff bind: {e:?}"))?;
    let rows = crate::db::d1_safe::safe_all_rows(&stmt)
        .await
        .map_err(|e| format!("D1 event_ids_for_staff execute: {e}"))?;
    Ok(rows
        .into_iter()
        .filter_map(|row| row.get("id")?.as_str().map(str::to_owned))
        .collect())
}
