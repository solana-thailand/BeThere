//! The landing page's aggregates (`GET /api/public/stats`): one statement, no
//! parameters, no personal data. The query groups by the stored
//! `participation_type`; `fold_stats` classifies each value with the app's own
//! parser. Definitions live with the type,
//! `event_checkin_domain::models::public_stats`.

use event_checkin_domain::models::public_stats::{PublicStats, StatsRow, fold_stats};
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
    let rows = rows
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
    Ok(fold_stats(&rows, measured_at))
}
