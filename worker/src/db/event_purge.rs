//! Hard-deleting an event takes its own rows with it (.issues/187).
//!
//! `DELETE /api/events/{id}/delete` used to remove the `events` row only, so
//! the event's attendees, deposits and answers stayed behind, pointing at
//! nothing. Every write smoke left one attendee and one ฿500 fixture deposit
//! in prod that way, and the nightly purge, which works per event end, never
//! reaches an event that no longer exists.
//!
//! Order: archive the deposits' amounts (the money record outlives the event,
//! same rule as the nightly purge), check the archive covers every live
//! deposit, then delete the event-scoped rows in one batch
//! (`sql/event_purge.sql`). If the archive is incomplete nothing is deleted.

use worker::{D1Database, D1Type};

/// One DELETE per table, in order; see the header of the file.
const PURGE_SQL: &str = include_str!("sql/event_purge.sql");

/// Tables with an `event_id` that a hard delete deliberately keeps. Together
/// with the tables in `sql/event_purge.sql` they must cover every `event_id`
/// table (worker/tests/security/test_event_purge.py).
pub const EVENT_PURGE_KEEPS: [&str; 7] = [
    "audit_log",
    "credit_ledger",
    "thb_deposit_archive",
    "onchain_events",
    "escrow_index",
    "nft_mint_jobs",
    // Deleted by sync_delete_event_from_d1 right after the events row.
    "event_summaries",
];

/// The purge's statements, without the header comment.
pub fn purge_statements() -> Vec<String> {
    PURGE_SQL
        .split("\n;\n")
        .map(|s| {
            s.lines()
                .filter(|l| !l.trim_start().starts_with("--"))
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Archive the event's deposits, then delete everything that belongs to it
/// except the events row itself. Returns how many deposits the archive holds
/// for the event.
pub async fn purge_event_rows(db: &D1Database, event_id: &str) -> Result<i64, String> {
    let coverage = super::thb_deposits::archive_thb_deposits_for_event(db, event_id).await?;
    if !coverage.is_complete() {
        return Err(format!(
            "{} deposit(s) not archived; nothing deleted",
            coverage.unarchived
        ));
    }
    let statements = purge_statements()
        .into_iter()
        .map(|sql| {
            db.prepare(sql)
                .bind_refs(&[D1Type::Text(event_id)])
                .map_err(|e| format!("D1 event purge bind: {e:?}"))
        })
        .collect::<Result<Vec<_>, String>>()?;
    db.batch(statements)
        .await
        .map_err(|e| format!("D1 event purge batch: {e:?}"))?;
    Ok(coverage.archived)
}
