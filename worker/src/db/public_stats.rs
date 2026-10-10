//! The landing page's aggregates (`GET /api/public/stats`): one statement, no
//! parameters, no personal data. The query groups by the stored
//! `participation_type`; `fold_stats` classifies each value with the app's own
//! parser; a second statement reads the durations behind the timings.
//! Definitions live with the type,
//! `event_checkin_domain::models::public_stats`.

use event_checkin_domain::models::public_stats::{
    EventPayerRow, PublicStats, StatsRow, fold_event_payers, fold_stats,
};
use worker::D1Database;

/// Count everything in one D1 round trip. `measured_at` is the caller's.
pub(crate) async fn read_public_stats(
    db: &D1Database,
    measured_at: String,
) -> Result<PublicStats, String> {
    let stmt = db.prepare(include_str!("sql/public_stats.sql"));
    let rows = crate::db::d1_safe::safe_all_rows(&stmt)
        .await
        .map_err(|e| format!("D1 public stats: {e}"))?;
    let mut rows = rows
        .iter()
        .map(|row| {
            let text = |key: &str| {
                row.get(key)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            let count = |key: &str| {
                row.get(key)
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| format!("D1 public stats: '{key}' missing or not a count"))
            };
            Ok(StatsRow {
                kind: text("kind"),
                participation_type: text("pt"),
                n: count("n")?,
                checked_in: count("came")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    rows.extend(read_timing_rows(db).await?);
    let mut stats = fold_stats(&rows, measured_at);
    stats.payers_by_event = fold_event_payers(&read_event_payer_rows(db).await?);
    Ok(stats)
}

/// The per-event payers statement (.plans/045 R4.7): a third round trip, for
/// the same compound-SELECT cap as the timings.
async fn read_event_payer_rows(db: &D1Database) -> Result<Vec<EventPayerRow>, String> {
    let stmt = db.prepare(include_str!("sql/public_stats_by_event.sql"));
    let rows = crate::db::d1_safe::safe_all_rows(&stmt)
        .await
        .map_err(|e| format!("D1 public stats by event: {e}"))?;
    rows.iter()
        .map(|row| {
            let text = |key: &str| {
                row.get(key)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            let count = |key: &str| {
                row.get(key)
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| format!("D1 public stats by event: '{key}' not a count"))
            };
            Ok(EventPayerRow {
                event_id: text("event_id"),
                name: text("name"),
                slug: text("slug"),
                start_ms: row
                    .get("start_ms")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0),
                participation_type: text("pt"),
                n: count("n")?,
                checked_in: count("came")?,
            })
        })
        .collect()
}

/// The timings statement, as `slip_s` / `refund_s` rows for `fold_stats`. A
/// second round trip: D1 rejects one compound SELECT with both shapes, and the
/// edge cache in front of the route means a burst still reads D1 once.
async fn read_timing_rows(db: &D1Database) -> Result<Vec<StatsRow>, String> {
    let stmt = db.prepare(include_str!("sql/public_stats_timings.sql"));
    let rows = crate::db::d1_safe::safe_all_rows(&stmt)
        .await
        .map_err(|e| format!("D1 public stats timings: {e}"))?;
    let duration = |kind: &str, n: u64| StatsRow {
        kind: kind.to_string(),
        participation_type: String::new(),
        n,
        checked_in: 0,
    };
    Ok(rows
        .iter()
        .flat_map(|row| {
            let seconds = |key: &str| row.get(key).and_then(serde_json::Value::as_u64);
            [
                seconds("slip_s").map(|n| duration("slip_s", n)),
                seconds("refund_s").map(|n| duration("refund_s", n)),
            ]
        })
        .flatten()
        .collect())
}
