//! Event series navigation (Plan 013 — public read for ticket page).

use wasm_bindgen_futures::JsFuture;
use worker::{D1Database, D1Type};

use super::types::CampaignRow;


/// One entry in a campaign's ordered event list — only the public-facing fields
/// needed to render prev/next + playlist links. Joined from `campaign_events`
/// and `events`.
#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
pub struct EventSeriesEntry {
    pub event_id: String,
    pub name: String,
    pub slug: String,
    pub event_start_ms: i64,
    pub sequence_order: i64,
}

/// Reverse lookup: find the (first) campaign that contains this event.
/// A campaign can always be resolved from an event because `campaign_events`
/// has `idx_campaign_events_event` on `event_id`.
///
/// Returns `Ok(None)` when the event belongs to no campaign — the caller treats
/// that as "hide the series section".
pub(crate) async fn get_campaign_for_event(
    db: &D1Database,
    event_id: &str,
) -> Result<Option<CampaignRow>, String> {
    let sql = "SELECT c.* FROM campaigns c \
         INNER JOIN campaign_events ce ON ce.campaign_id = c.id \
         WHERE ce.event_id = ? \
         LIMIT 1";
    // Bypass `.first::<T>()` — crashes on JsValue(null) when no row matches.
    let stmt = db
        .prepare(sql)
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 get_campaign_for_event bind: {e:?}"))?;
    let raw_result = JsFuture::from(
        stmt.inner()
            .all()
            .map_err(|e| format!("D1 get_campaign_for_event all() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_campaign_for_event all() await: {e:?}"))?;

    let results_key = wasm_bindgen::JsValue::from_str("results");
    let raw_rows =
        js_sys::Reflect::get(&raw_result, &results_key).unwrap_or(wasm_bindgen::JsValue::NULL);
    let json_str = js_sys::JSON::stringify(&raw_rows)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    let mut rows: Vec<CampaignRow> =
        serde_json::from_str(&json_str).map_err(|e| format!("deserialize campaign rows: {e:?}"))?;
    Ok(rows.pop())
}

/// List a campaign's events in `sequence_order`, joined to `events` for the
/// public-facing fields. Uses the raw JSON path to survive nullable columns
/// and a missing `events` row (defensive — a dangling `campaign_events` row
/// should not 500 the whole section).
pub(crate) async fn list_campaign_event_summaries(
    db: &D1Database,
    campaign_id: &str,
) -> Result<Vec<EventSeriesEntry>, String> {
    let sql = "SELECT ce.event_id AS event_id, e.name AS name, e.slug AS slug, \
                COALESCE(e.event_start_ms, 0) AS event_start_ms, ce.sequence_order AS sequence_order \
         FROM campaign_events ce \
         LEFT JOIN events e ON e.id = ce.event_id \
         WHERE ce.campaign_id = ? \
         ORDER BY ce.sequence_order ASC, e.event_start_ms ASC";

    let stmt = db
        .prepare(sql)
        .bind_refs(&[D1Type::Text(campaign_id)])
        .map_err(|e| format!("D1 list_campaign_event_summaries bind: {e:?}"))?;
    let raw_result = JsFuture::from(
        stmt.inner()
            .all()
            .map_err(|e| format!("D1 list_campaign_event_summaries all() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 list_campaign_event_summaries all() await: {e:?}"))?;

    let results_key = wasm_bindgen::JsValue::from_str("results");
    let raw_rows =
        js_sys::Reflect::get(&raw_result, &results_key).unwrap_or(wasm_bindgen::JsValue::NULL);
    let json_str = js_sys::JSON::stringify(&raw_rows)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    // Deserialize defensively: skip any row missing an event_id (orphan link).
    let rows: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap_or_default();
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let event_id = r.get("event_id").and_then(|v| v.as_str()).unwrap_or("");
        if event_id.is_empty() {
            continue;
        }
        out.push(EventSeriesEntry {
            event_id: event_id.to_string(),
            name: r
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            slug: r
                .get("slug")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            event_start_ms: r
                .get("event_start_ms")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            sequence_order: r
                .get("sequence_order")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
        });
    }
    Ok(out)
}

/// Locate `current_event_id` within an ordered `events` list and resolve its
/// previous/next neighbors by position.
///
/// Pure (no I/O) so the edge cases — first, last, single, orphan (linked but
/// missing from the joined events list) — are unit-tested directly. Returns:
///
/// - `(index, Some(prev), Some(next))` in the middle of the list,
/// - `(0, None, Some(next))` for the first event,
/// - `(last, Some(prev), None)` for the last event,
/// - `(-1, None, None)` when the event is not in the list (orphan link),
///   or when the list is empty.
///
/// The index is `i64` (not `usize`) so `-1` can be serialized in the API
/// response and the frontend can render the badge without a null-check dance.
pub fn compute_series_neighbors(
    events: &[EventSeriesEntry],
    current_event_id: &str,
) -> (i64, Option<EventSeriesEntry>, Option<EventSeriesEntry>) {
    match events.iter().position(|e| e.event_id == current_event_id) {
        Some(i) => {
            let prev = if i > 0 {
                events.get(i - 1).cloned()
            } else {
                None
            };
            let next = events.get(i + 1).cloned();
            (i as i64, prev, next)
        }
        // Event is linked to the campaign but missing from the joined list
        // (orphan campaign_events row, or events row deleted). Still return the
        // series so the badge can show; just no prev/next.
        None => (-1, None, None),
    }
}
