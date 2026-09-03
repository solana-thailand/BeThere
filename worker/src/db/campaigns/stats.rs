//! Campaign dashboard completion statistics.

use worker::{D1Database, D1Type};

use super::types::{CampaignCompletionStats, EventDropOff};


/// Per-event drop-off row from the join between campaign_events and attendees.
#[derive(Debug, Clone, serde::Deserialize)]
struct EventDropOffRow {
    event_id: String,
    sequence_order: i64,
    attended: i64,
}

/// Build the totals query for [`campaign_completion_stats`].
///
/// Extracted so the `COALESCE` is unit-testable. It is not cosmetic: SQLite's
/// `SUM` over an empty set returns NULL, which cannot deserialize into
/// `TotalsRow::total_completed` (`i64`), so the whole stats call 500s. Every
/// campaign has no enrolled developers at creation, so this fired on every new
/// campaign until 2026-08-21.
pub(super) fn totals_sql() -> &'static str {
    "SELECT \
     COUNT(*) AS total_enrolled, \
     COALESCE(SUM(CASE WHEN is_complete = 1 THEN 1 ELSE 0 END), 0) AS total_completed \
     FROM developer_campaign_progress \
     WHERE campaign_id = ?"
}

#[allow(dead_code)]
pub(crate) async fn campaign_completion_stats(
    db: &D1Database,
    campaign_id: &str,
) -> Result<CampaignCompletionStats, String> {
    // Total enrolled and completed from developer_campaign_progress.
    let totals = db
        .prepare(totals_sql())
        .bind_refs(&[D1Type::Text(campaign_id)])
        .map_err(|e| format!("D1 campaign_completion_stats totals bind: {e:?}"))?
        .first::<TotalsRow>(None)
        .await
        .map_err(|e| format!("D1 campaign_completion_stats totals: {e:?}"))?;

    let total_enrolled = totals.as_ref().map(|t| t.total_enrolled).unwrap_or(0);
    let total_completed = totals.as_ref().map(|t| t.total_completed).unwrap_or(0);
    let completion_rate = if total_enrolled > 0 {
        (total_completed as f64) / (total_enrolled as f64)
    } else {
        0.0
    };

    // Per-event drop-off: for each campaign_event, count enrolled developers who
    // checked in to that event (attendees.checked_in_at IS NOT NULL).
    let dropoff_sql = "SELECT ce.event_id, ce.sequence_order, \
         COUNT(a.id) AS attended \
         FROM campaign_events ce \
         LEFT JOIN attendees a ON a.event_id = ce.event_id \
         AND a.checked_in_at IS NOT NULL \
         AND a.email IN ( \
         SELECT developer_email FROM developer_campaign_progress \
         WHERE campaign_id = ? \
         ) \
         WHERE ce.campaign_id = ? \
         GROUP BY ce.event_id, ce.sequence_order \
         ORDER BY ce.sequence_order ASC";
    let result = db
        .prepare(dropoff_sql)
        .bind_refs(&[D1Type::Text(campaign_id), D1Type::Text(campaign_id)])
        .map_err(|e| format!("D1 campaign_completion_stats dropoff bind: {e:?}"))?
        .all()
        .await
        .map_err(|e| format!("D1 campaign_completion_stats dropoff: {e:?}"))?;
    let dropoff_rows = result
        .results::<EventDropOffRow>()
        .map_err(|e| format!("D1 campaign_completion_stats dropoff results: {e:?}"))?;

    let events = dropoff_rows
        .into_iter()
        .map(|row| EventDropOff {
            event_id: row.event_id,
            sequence_order: row.sequence_order,
            attended: row.attended,
            total_in_campaign: total_enrolled,
        })
        .collect();

    Ok(CampaignCompletionStats {
        total_enrolled,
        total_completed,
        completion_rate,
        events,
    })
}

/// Helper row for the totals aggregation query.
#[derive(Debug, Clone, serde::Deserialize)]
struct TotalsRow {
    total_enrolled: i64,
    /// MUST stay non-optional only because the query wraps the `SUM` in
    /// `COALESCE(..., 0)`. SQLite's `SUM` over an empty set is NULL, and a NULL
    /// here fails deserialization and surfaces as a 500 — which is exactly what
    /// every campaign with no enrolled developers did until 2026-08-21, i.e.
    /// every campaign the moment it was created. Found via the plan 016
    /// staging click-through.
    total_completed: i64,
}
