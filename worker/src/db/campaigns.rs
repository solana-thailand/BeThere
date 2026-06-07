//! D1 campaign query helpers (Issue 049 Phase 3).
//!
//! Campaigns group events into series with completion tracking and rewards.
//! Three tables: campaigns, campaign_events, developer_campaign_progress.

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
