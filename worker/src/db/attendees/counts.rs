//! Head-count per participation type for one event (plan 028 W3).
//!
//! One `GROUP BY` row per distinct stored value instead of every attendee row.
//! The rows are classified in Rust by `TrackCounts::add`, so the tally uses the
//! same `ParticipationType::parse` as everything else.

use event_checkin_domain::models::attendee::TrackCounts;
use worker::D1Database;
use worker::d1::D1Type;

/// Track counts for `event_id`, and whether D1 holds any attendee row for it.
///
/// `None` means the event has no D1 rows at all, which is every event whose
/// attendees still live only in its sheet; the caller falls back to the sheet.
pub(crate) async fn count_tracks_by_event(
    db: &D1Database,
    event_id: &str,
) -> Result<Option<TrackCounts>, String> {
    let stmt = db
        .prepare(include_str!("../sql/attendee_counts_by_participation.sql"))
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 count_tracks_by_event bind: {e:?}"))?;
    let rows = crate::db::d1_safe::safe_all_rows(&stmt)
        .await
        .map_err(|e| format!("D1 count_tracks_by_event execute: {e}"))?;
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
