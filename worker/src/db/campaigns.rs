//! D1 campaign query helpers (Issue 049 Phase 3).
//!
//! Campaigns group events into series with completion tracking and rewards.
//! Three tables: campaigns, campaign_events, developer_campaign_progress.

use wasm_bindgen_futures::JsFuture;
use worker::D1Database;

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct CampaignRow {
    pub id: String,
    pub title: String,
    pub description: String,
    pub organization_id: String,
    pub status: String,
    pub completion_criteria: String, // JSON
    pub reward_type: String,
    pub reward_config: String, // JSON
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct CampaignEventRow {
    pub campaign_id: String,
    pub event_id: String,
    pub sequence_order: i64,
    pub is_required: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct DeveloperCampaignProgressRow {
    pub campaign_id: String,
    pub developer_email: String,
    pub events_completed: i64,
    pub total_required: i64,
    pub is_complete: i64,
    pub completed_at: Option<String>,
    pub reward_claimed_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Dashboard stats types
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct CampaignCompletionStats {
    pub total_enrolled: i64,
    pub total_completed: i64,
    pub completion_rate: f64,
    pub events: Vec<EventDropOff>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct EventDropOff {
    pub event_id: String,
    pub sequence_order: i64,
    pub attended: i64,
    pub total_in_campaign: i64,
}

// ---------------------------------------------------------------------------
// Campaign CRUD
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_campaign(
    db: &D1Database,
    id: &str,
    title: &str,
    description: &str,
    organization_id: &str,
    completion_criteria: &str,
    reward_type: &str,
    reward_config: &str,
) -> Result<(), String> {
    let sql = format!(
        "INSERT INTO campaigns (id, title, description, organization_id, status, \
         completion_criteria, reward_type, reward_config, created_at, updated_at) \
         VALUES ('{id}', '{title}', '{description}', '{organization_id}', 'draft', \
         '{completion_criteria}', '{reward_type}', '{reward_config}', \
         datetime('now'), datetime('now'))"
    );
    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 create_campaign: {e:?}"))?;
    Ok(())
}

pub(crate) async fn update_campaign(
    db: &D1Database,
    id: &str,
    title: &str,
    description: &str,
    completion_criteria: &str,
    reward_type: &str,
    reward_config: &str,
) -> Result<(), String> {
    let sql = format!(
        "UPDATE campaigns SET \
         title = '{title}', \
         description = '{description}', \
         completion_criteria = '{completion_criteria}', \
         reward_type = '{reward_type}', \
         reward_config = '{reward_config}', \
         updated_at = datetime('now') \
         WHERE id = '{id}'"
    );
    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 update_campaign: {e:?}"))?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) async fn get_campaign(db: &D1Database, id: &str) -> Result<Option<CampaignRow>, String> {
    let sql = format!("SELECT * FROM campaigns WHERE id = '{id}' LIMIT 1");
    db.prepare(&sql)
        .first::<CampaignRow>(None)
        .await
        .map_err(|e| format!("D1 get_campaign query: {e:?}"))
}

#[allow(dead_code)]
/// Fetch all distinct `collection_mint` values from active campaigns with
/// `reward_type = 'nft_certificate'`. Used to classify NFTs as campaign vs event.
pub(crate) async fn campaign_collection_mints(db: &D1Database) -> Result<Vec<String>, String> {
    let sql = "SELECT reward_config FROM campaigns WHERE status = 'active' AND reward_type = 'nft_certificate'";
    let result = db
        .prepare(sql)
        .all()
        .await
        .map_err(|e| format!("D1 campaign_collection_mints: {e:?}"))?;
    let rows = result
        .results::<CampaignRow>()
        .map_err(|e| format!("D1 campaign_collection_mints results: {e:?}"))?;

    let mut mints = Vec::new();
    for row in &rows {
        if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&row.reward_config)
            && let Some(mint) = cfg.get("collection_mint").and_then(|v| v.as_str())
            && !mint.is_empty()
        {
            mints.push(mint.to_string());
        }
    }
    Ok(mints)
}

pub(crate) async fn list_campaigns(
    db: &D1Database,
    organization_id: Option<&str>,
    status: Option<&str>,
) -> Result<Vec<CampaignRow>, String> {
    let mut clauses = Vec::new();
    if let Some(org) = organization_id {
        clauses.push(format!("organization_id = '{org}'"));
    }
    if let Some(s) = status {
        clauses.push(format!("status = '{s}'"));
    }
    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };
    let sql = format!("SELECT * FROM campaigns {where_clause} ORDER BY created_at DESC");
    let result = db
        .prepare(&sql)
        .all()
        .await
        .map_err(|e| format!("D1 list_campaigns: {e:?}"))?;
    result
        .results::<CampaignRow>()
        .map_err(|e| format!("D1 list_campaigns results: {e:?}"))
}

pub(crate) async fn update_campaign_status(
    db: &D1Database,
    id: &str,
    status: &str,
) -> Result<(), String> {
    let sql = format!(
        "UPDATE campaigns SET status = '{status}', updated_at = datetime('now') \
         WHERE id = '{id}'"
    );
    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 update_campaign_status: {e:?}"))?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) async fn delete_campaign(db: &D1Database, id: &str) -> Result<(), String> {
    let sql = format!("DELETE FROM campaigns WHERE id = '{id}'");
    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 delete_campaign: {e:?}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Campaign Events
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub(crate) async fn add_campaign_event(
    db: &D1Database,
    campaign_id: &str,
    event_id: &str,
    sequence_order: i64,
    is_required: i64,
) -> Result<(), String> {
    let sql = format!(
        "INSERT INTO campaign_events (campaign_id, event_id, sequence_order, is_required) \
         VALUES ('{campaign_id}', '{event_id}', {sequence_order}, {is_required})"
    );
    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 add_campaign_event: {e:?}"))?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) async fn remove_campaign_event(
    db: &D1Database,
    campaign_id: &str,
    event_id: &str,
) -> Result<(), String> {
    let sql = format!(
        "DELETE FROM campaign_events \
         WHERE campaign_id = '{campaign_id}' AND event_id = '{event_id}'"
    );
    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 remove_campaign_event: {e:?}"))?;
    Ok(())
}

pub(crate) async fn list_campaign_events(
    db: &D1Database,
    campaign_id: &str,
) -> Result<Vec<CampaignEventRow>, String> {
    let sql = format!(
        "SELECT * FROM campaign_events \
         WHERE campaign_id = '{campaign_id}' \
         ORDER BY sequence_order ASC"
    );
    let result = db
        .prepare(&sql)
        .all()
        .await
        .map_err(|e| format!("D1 list_campaign_events: {e:?}"))?;
    result
        .results::<CampaignEventRow>()
        .map_err(|e| format!("D1 list_campaign_events results: {e:?}"))
}

/// Full replace: delete all existing events for the campaign, then batch-insert new ones.
pub(crate) async fn set_campaign_events(
    db: &D1Database,
    campaign_id: &str,
    events: &[(String, i64, i64)], // (event_id, sequence_order, is_required)
) -> Result<(), String> {
    let delete_sql = format!("DELETE FROM campaign_events WHERE campaign_id = '{campaign_id}'");
    db.exec(&delete_sql)
        .await
        .map_err(|e| format!("D1 set_campaign_events delete: {e:?}"))?;

    for (event_id, sequence_order, is_required) in events {
        let sql = format!(
            "INSERT INTO campaign_events (campaign_id, event_id, sequence_order, is_required) \
             VALUES ('{campaign_id}', '{event_id}', {sequence_order}, {is_required})"
        );
        db.exec(&sql)
            .await
            .map_err(|e| format!("D1 set_campaign_events insert: {e:?}"))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Developer Campaign Progress
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub(crate) async fn get_developer_progress(
    db: &D1Database,
    campaign_id: &str,
    developer_email: &str,
) -> Result<Option<DeveloperCampaignProgressRow>, String> {
    let sql = format!(
        "SELECT * FROM developer_campaign_progress \
         WHERE campaign_id = '{campaign_id}' AND developer_email = '{developer_email}' \
         LIMIT 1"
    );
    db.prepare(&sql)
        .first::<DeveloperCampaignProgressRow>(None)
        .await
        .map_err(|e| format!("D1 get_developer_progress query: {e:?}"))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn upsert_developer_progress(
    db: &D1Database,
    campaign_id: &str,
    developer_email: &str,
    events_completed: i64,
    total_required: i64,
    is_complete: i64,
) -> Result<(), String> {
    let completed_at_expr = if is_complete == 1 {
        "datetime('now')"
    } else {
        "NULL"
    };
    let sql = format!(
        "INSERT INTO developer_campaign_progress \
         (campaign_id, developer_email, events_completed, total_required, is_complete, \
          completed_at, reward_claimed_at) \
         VALUES ('{campaign_id}', '{developer_email}', {events_completed}, {total_required}, \
         {is_complete}, {completed_at_expr}, NULL) \
         ON CONFLICT (campaign_id, developer_email) DO UPDATE SET \
         events_completed = excluded.events_completed, \
         total_required = excluded.total_required, \
         is_complete = excluded.is_complete, \
         completed_at = CASE WHEN excluded.is_complete = 1 THEN datetime('now') ELSE developer_campaign_progress.completed_at END"
    );
    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 upsert_developer_progress: {e:?}"))?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) async fn list_campaign_progress(
    db: &D1Database,
    campaign_id: &str,
) -> Result<Vec<DeveloperCampaignProgressRow>, String> {
    let sql = format!(
        "SELECT * FROM developer_campaign_progress \
         WHERE campaign_id = '{campaign_id}' \
         ORDER BY developer_email ASC"
    );
    let result = db
        .prepare(&sql)
        .all()
        .await
        .map_err(|e| format!("D1 list_campaign_progress: {e:?}"))?;
    result
        .results::<DeveloperCampaignProgressRow>()
        .map_err(|e| format!("D1 list_campaign_progress results: {e:?}"))
}

#[allow(dead_code)]
pub(crate) async fn list_developer_campaigns(
    db: &D1Database,
    developer_email: &str,
) -> Result<Vec<DeveloperCampaignProgressRow>, String> {
    let sql = format!(
        "SELECT * FROM developer_campaign_progress \
         WHERE developer_email = '{developer_email}' \
         ORDER BY campaign_id ASC"
    );
    let result = db
        .prepare(&sql)
        .all()
        .await
        .map_err(|e| format!("D1 list_developer_campaigns: {e:?}"))?;
    result
        .results::<DeveloperCampaignProgressRow>()
        .map_err(|e| format!("D1 list_developer_campaigns results: {e:?}"))
}

#[allow(dead_code)]
pub(crate) async fn mark_reward_claimed(
    db: &D1Database,
    campaign_id: &str,
    developer_email: &str,
) -> Result<(), String> {
    let sql = format!(
        "UPDATE developer_campaign_progress \
         SET reward_claimed_at = datetime('now') \
         WHERE campaign_id = '{campaign_id}' AND developer_email = '{developer_email}'"
    );
    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 mark_reward_claimed: {e:?}"))?;
    Ok(())
}

/// Mark campaign reward claimed with minted NFT details (asset_id + signature).
pub(crate) async fn mark_reward_claimed_with_mint(
    db: &D1Database,
    campaign_id: &str,
    developer_email: &str,
    asset_id: &str,
    signature: &str,
) -> Result<(), String> {
    let sql = format!(
        "UPDATE developer_campaign_progress \
         SET reward_claimed_at = datetime('now'), \
             reward_asset_id = '{asset_id}', \
             reward_signature = '{signature}' \
         WHERE campaign_id = '{campaign_id}' AND developer_email = '{developer_email}'"
    );
    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 mark_reward_claimed_with_mint: {e:?}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Campaign Dashboard Stats
// ---------------------------------------------------------------------------

/// Per-event drop-off row from the join between campaign_events and attendees.
#[derive(Debug, Clone, serde::Deserialize)]
struct EventDropOffRow {
    event_id: String,
    sequence_order: i64,
    attended: i64,
}

#[allow(dead_code)]
pub(crate) async fn campaign_completion_stats(
    db: &D1Database,
    campaign_id: &str,
) -> Result<CampaignCompletionStats, String> {
    // Total enrolled and completed from developer_campaign_progress.
    let totals_sql = format!(
        "SELECT \
         COUNT(*) AS total_enrolled, \
         SUM(CASE WHEN is_complete = 1 THEN 1 ELSE 0 END) AS total_completed \
         FROM developer_campaign_progress \
         WHERE campaign_id = '{campaign_id}'"
    );
    let totals = db
        .prepare(&totals_sql)
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
    let dropoff_sql = format!(
        "SELECT ce.event_id, ce.sequence_order, \
         COUNT(a.id) AS attended \
         FROM campaign_events ce \
         LEFT JOIN attendees a ON a.event_id = ce.event_id \
         AND a.checked_in_at IS NOT NULL \
         AND a.email IN ( \
         SELECT developer_email FROM developer_campaign_progress \
         WHERE campaign_id = '{campaign_id}' \
         ) \
         WHERE ce.campaign_id = '{campaign_id}' \
         GROUP BY ce.event_id, ce.sequence_order \
         ORDER BY ce.sequence_order ASC"
    );
    let result = db
        .prepare(&dropoff_sql)
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
    total_completed: i64,
}

// ---------------------------------------------------------------------------
// Auto-Progress on Check-In (Issue 051 Phase 1)
// ---------------------------------------------------------------------------

/// After a successful check-in, update campaign progress for any campaigns that include this event.
/// Non-blocking: errors are logged but don't affect check-in.
pub(crate) async fn on_event_checkin(db: &D1Database, event_id: &str, developer_email: &str) {
    // 1. Find all campaigns that include this event
    let campaigns_sql =
        format!("SELECT DISTINCT campaign_id FROM campaign_events WHERE event_id = '{event_id}'");
    let result = match db.prepare(&campaigns_sql).all().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(event_id = %event_id, error = %e, "campaign auto-progress: failed to find campaigns");
            return;
        }
    };

    let campaign_rows = match result.results::<CampaignEventRow>() {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(event_id = %event_id, error = %e, "campaign auto-progress: failed to parse campaign rows");
            return;
        }
    };

    for row in campaign_rows {
        let campaign_id = &row.campaign_id;

        // 2. Count total required events for this campaign
        let total_sql = format!(
            "SELECT COUNT(*) AS cnt FROM campaign_events WHERE campaign_id = '{campaign_id}' AND is_required = 1"
        );
        let total_result = match db.prepare(&total_sql).first::<TotalCountRow>(None).await {
            Ok(Some(r)) => r,
            Ok(None) => {
                tracing::warn!(campaign_id = %campaign_id, "campaign auto-progress: no total count");
                continue;
            }
            Err(e) => {
                tracing::warn!(campaign_id = %campaign_id, error = %e, "campaign auto-progress: failed to count total");
                continue;
            }
        };

        // 3. Count events this developer has checked into for this campaign
        let completed_sql = format!(
            "SELECT COUNT(*) AS cnt FROM campaign_events ce \
             INNER JOIN attendees a ON a.event_id = ce.event_id \
             WHERE ce.campaign_id = '{campaign_id}' \
             AND a.email = '{developer_email}' \
             AND a.checked_in_at IS NOT NULL"
        );
        let completed_result = match db
            .prepare(&completed_sql)
            .first::<TotalCountRow>(None)
            .await
        {
            Ok(Some(r)) => r,
            Ok(None) => {
                tracing::warn!(campaign_id = %campaign_id, "campaign auto-progress: no completed count");
                continue;
            }
            Err(e) => {
                tracing::warn!(campaign_id = %campaign_id, error = %e, "campaign auto-progress: failed to count completed");
                continue;
            }
        };

        let events_completed = completed_result.cnt;
        let total_required = total_result.cnt;
        let is_complete = if events_completed >= total_required {
            1
        } else {
            0
        };

        // 4. Upsert progress
        if let Err(e) = upsert_developer_progress(
            db,
            campaign_id,
            developer_email,
            events_completed,
            total_required,
            is_complete,
        )
        .await
        {
            tracing::warn!(
                campaign_id = %campaign_id,
                developer_email = %developer_email,
                error = %e,
                "campaign auto-progress: failed to upsert"
            );
        } else {
            tracing::info!(
                campaign_id = %campaign_id,
                developer_email = %developer_email,
                events_completed = events_completed,
                total_required = total_required,
                is_complete = is_complete,
                "campaign auto-progress updated"
            );
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TotalCountRow {
    cnt: i64,
}

// ---------------------------------------------------------------------------
// Event Series Navigation (Plan 013 — public read for ticket page)
// ---------------------------------------------------------------------------

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
    let sql = format!(
        "SELECT c.* FROM campaigns c \
         INNER JOIN campaign_events ce ON ce.campaign_id = c.id \
         WHERE ce.event_id = '{event_id}' \
         LIMIT 1"
    );
    // Bypass `.first::<T>()` — crashes on JsValue(null) when no row matches.
    let stmt = db.prepare(&sql);
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
    let sql = format!(
        "SELECT ce.event_id AS event_id, e.name AS name, e.slug AS slug, \
                COALESCE(e.event_start_ms, 0) AS event_start_ms, ce.sequence_order AS sequence_order \
         FROM campaign_events ce \
         LEFT JOIN events e ON e.id = ce.event_id \
         WHERE ce.campaign_id = '{campaign_id}' \
         ORDER BY ce.sequence_order ASC, e.event_start_ms ASC"
    );

    let stmt = db.prepare(&sql);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, seq: i64) -> EventSeriesEntry {
        EventSeriesEntry {
            event_id: id.to_string(),
            name: format!("Event {id}"),
            slug: format!("slug-{id}"),
            event_start_ms: 0,
            sequence_order: seq,
        }
    }

    // Series with 3 events: A → B → C. Reused across the position tests.
    fn three_event_series() -> Vec<EventSeriesEntry> {
        vec![entry("A", 1), entry("B", 2), entry("C", 3)]
    }

    // --- Edge: empty list ---
    #[test]
    fn empty_list_returns_orphan_index() {
        let (idx, prev, next) = compute_series_neighbors(&[], "A");
        assert_eq!(idx, -1);
        assert!(prev.is_none());
        assert!(next.is_none());
    }

    // --- Edge: orphan (linked but missing from joined events list) ---
    #[test]
    fn orphan_event_returns_negative_index_and_no_neighbors() {
        let events = three_event_series();
        let (idx, prev, next) = compute_series_neighbors(&events, "ZZZ");
        assert_eq!(idx, -1);
        assert!(prev.is_none());
        assert!(next.is_none());
    }

    // --- Edge: single-event campaign ---
    #[test]
    fn single_event_has_index_zero_and_no_neighbors() {
        let events = vec![entry("solo", 1)];
        let (idx, prev, next) = compute_series_neighbors(&events, "solo");
        assert_eq!(idx, 0);
        assert!(prev.is_none(), "single event should have no previous");
        assert!(next.is_none(), "single event should have no next");
    }

    // --- Edge: first event in a multi-event series ---
    #[test]
    fn first_event_has_no_previous() {
        let events = three_event_series();
        let (idx, prev, next) = compute_series_neighbors(&events, "A");
        assert_eq!(idx, 0);
        assert!(prev.is_none(), "first event should have no previous");
        assert_eq!(next.as_ref().unwrap().event_id, "B");
    }

    // --- Edge: last event in a multi-event series ---
    #[test]
    fn last_event_has_no_next() {
        let events = three_event_series();
        let (idx, prev, next) = compute_series_neighbors(&events, "C");
        assert_eq!(idx, 2);
        assert_eq!(prev.as_ref().unwrap().event_id, "B");
        assert!(next.is_none(), "last event should have no next");
    }

    // --- Middle of the series ---
    #[test]
    fn middle_event_has_both_neighbors() {
        let events = three_event_series();
        let (idx, prev, next) = compute_series_neighbors(&events, "B");
        assert_eq!(idx, 1);
        assert_eq!(prev.as_ref().unwrap().event_id, "A");
        assert_eq!(next.as_ref().unwrap().event_id, "C");
    }

    // --- Position is by list index, not sequence_order ---
    // A campaign_events row could (theoretically) carry the same sequence_order
    // twice; the handler relies on list position as the source of truth.
    #[test]
    fn neighbors_use_list_index_not_sequence_order() {
        // Deliberately non-sequential order values; list order is A,B,C.
        let events = vec![entry("A", 5), entry("B", 5), entry("C", 1)];
        let (idx, prev, next) = compute_series_neighbors(&events, "B");
        assert_eq!(idx, 1);
        assert_eq!(prev.as_ref().unwrap().event_id, "A");
        assert_eq!(next.as_ref().unwrap().event_id, "C");
    }

    // --- Neighbors are cloned by value, not by reference ---
    #[test]
    fn neighbors_carry_full_entry_payload() {
        let events = three_event_series();
        let (_, _, next) = compute_series_neighbors(&events, "B");
        let n = next.unwrap();
        assert_eq!(n.event_id, "C");
        assert_eq!(n.name, "Event C");
        assert_eq!(n.slug, "slug-C");
        assert_eq!(n.sequence_order, 3);
    }

    // --- First match wins when an id appears twice (defensive) ---
    #[test]
    fn duplicate_event_id_uses_first_match() {
        let events = vec![entry("dup", 1), entry("other", 2), entry("dup", 3)];
        // First "dup" is at index 0 → no previous, next is "other".
        let (idx, prev, next) = compute_series_neighbors(&events, "dup");
        assert_eq!(idx, 0);
        assert!(prev.is_none());
        assert_eq!(next.as_ref().unwrap().event_id, "other");
    }
}
