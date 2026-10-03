//! Head-count per participation type for one event (plan 028 W3).
//!
//! One `GROUP BY` row per distinct stored value instead of every attendee row.
//! The rows are classified in Rust by `TrackCounts::add`, so the tally uses the
//! same `ParticipationType::parse` as everything else.

use event_checkin_domain::models::attendee::TrackCounts;
use worker::D1Database;

/// Track counts for `event_id`, and whether D1 holds any attendee row for it.
///
/// `None` means the event has no D1 rows at all, which is every event whose
/// attendees still live only in its sheet; the caller falls back to the sheet.
pub(crate) async fn count_tracks_by_event(
    db: &D1Database,
    event_id: &str,
) -> Result<Option<TrackCounts>, String> {
    let rows = crate::db::d1_safe::query_rows_by_text(
        db,
        include_str!("../sql/attendee_counts_by_participation.sql"),
        event_id,
    )
    .await
    .map_err(|e| format!("D1 count_tracks_by_event: {e}"))?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut counts = TrackCounts::default();
    for row in &rows {
        let participation_type = row
            .get("participation_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let n = row
            .get("n")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("D1 count_tracks_by_event: row without a count: {row}"))?;
        counts.add(participation_type, u32::try_from(n).unwrap_or(u32::MAX));
    }
    Ok(Some(counts))
}

/// How many attendees of `event_id` have been checked in, from D1.
///
/// Display only: no gate reads this, so a failure is the caller's to show as
/// "unknown", never as zero. An event whose attendees live only in its sheet has
/// no check-in time in D1 and counts 0 here.
pub(crate) async fn count_checked_in_by_event(
    db: &D1Database,
    event_id: &str,
) -> Result<u32, String> {
    let rows = crate::db::d1_safe::query_rows_by_text(
        db,
        include_str!("../sql/attendee_checked_in_count.sql"),
        event_id,
    )
    .await
    .map_err(|e| format!("D1 count_checked_in_by_event: {e}"))?;
    let n = rows
        .first()
        .and_then(|row| row.get("n"))
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "D1 count_checked_in_by_event: no count row".to_string())?;
    Ok(u32::try_from(n).unwrap_or(u32::MAX))
}
