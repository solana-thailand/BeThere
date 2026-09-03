//! Developer campaign progress and reward claim queries.

use worker::{D1Database, D1Type};

use super::types::{DeveloperCampaignProgressRow, DeveloperEventAttendanceRow};


#[allow(dead_code)]
pub(crate) async fn get_developer_progress(
    db: &D1Database,
    campaign_id: &str,
    developer_email: &str,
) -> Result<Option<DeveloperCampaignProgressRow>, String> {
    let args = [D1Type::Text(campaign_id), D1Type::Text(developer_email)];
    db.prepare(
        "SELECT * FROM developer_campaign_progress \
         WHERE campaign_id = ? AND developer_email = ? \
         LIMIT 1",
    )
    .bind_refs(&args)
    .map_err(|e| format!("D1 get_developer_progress bind: {e:?}"))?
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
         VALUES (?, ?, {events_completed}, {total_required}, \
         {is_complete}, {completed_at_expr}, NULL) \
         ON CONFLICT (campaign_id, developer_email) DO UPDATE SET \
         events_completed = excluded.events_completed, \
         total_required = excluded.total_required, \
         is_complete = excluded.is_complete, \
         completed_at = CASE WHEN excluded.is_complete = 1 THEN datetime('now') ELSE developer_campaign_progress.completed_at END"
    );
    let args = [D1Type::Text(campaign_id), D1Type::Text(developer_email)];
    db.prepare(&sql)
        .bind_refs(&args)
        .map_err(|e| format!("D1 upsert_developer_progress bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 upsert_developer_progress: {e:?}"))?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) async fn list_campaign_progress(
    db: &D1Database,
    campaign_id: &str,
) -> Result<Vec<DeveloperCampaignProgressRow>, String> {
    let result = db
        .prepare(
            "SELECT * FROM developer_campaign_progress \
             WHERE campaign_id = ? \
             ORDER BY developer_email ASC",
        )
        .bind_refs(&[D1Type::Text(campaign_id)])
        .map_err(|e| format!("D1 list_campaign_progress bind: {e:?}"))?
        .all()
        .await
        .map_err(|e| format!("D1 list_campaign_progress: {e:?}"))?;
    result
        .results::<DeveloperCampaignProgressRow>()
        .map_err(|e| format!("D1 list_campaign_progress results: {e:?}"))
}

/// Per-event attendance breakdown for every developer enrolled in a campaign.
///
/// Joins `developer_campaign_progress` → `campaign_events` → `events` (for the
/// name) → `attendees` (for the check-in status). Returns one row per
/// (developer, campaign-event) pair so the caller can group by developer.
pub(crate) async fn list_campaign_attendance(
    db: &D1Database,
    campaign_id: &str,
) -> Result<Vec<DeveloperEventAttendanceRow>, String> {
    let sql = "SELECT \
         dcp.developer_email AS developer_email, \
         ce.event_id AS event_id, \
         e.name AS event_name, \
         ce.sequence_order AS sequence_order, \
         ce.is_required AS is_required, \
         MAX(CASE WHEN a.checked_in_at IS NOT NULL THEN 1 ELSE 0 END) AS attended \
         FROM developer_campaign_progress dcp \
         JOIN campaign_events ce ON ce.campaign_id = dcp.campaign_id \
         LEFT JOIN events e ON e.id = ce.event_id \
         LEFT JOIN attendees a ON a.event_id = ce.event_id \
         AND a.email = dcp.developer_email \
         AND a.checked_in_at IS NOT NULL \
         WHERE dcp.campaign_id = ? \
         GROUP BY dcp.developer_email, ce.event_id, e.name, ce.sequence_order, ce.is_required \
         ORDER BY dcp.developer_email ASC, ce.sequence_order ASC";
    let result = db
        .prepare(sql)
        .bind_refs(&[D1Type::Text(campaign_id)])
        .map_err(|e| format!("D1 list_campaign_attendance bind: {e:?}"))?
        .all()
        .await
        .map_err(|e| format!("D1 list_campaign_attendance: {e:?}"))?;
    result
        .results::<DeveloperEventAttendanceRow>()
        .map_err(|e| format!("D1 list_campaign_attendance results: {e:?}"))
}

#[allow(dead_code)]
pub(crate) async fn list_developer_campaigns(
    db: &D1Database,
    developer_email: &str,
) -> Result<Vec<DeveloperCampaignProgressRow>, String> {
    let result = db
        .prepare(
            "SELECT * FROM developer_campaign_progress \
             WHERE developer_email = ? \
             ORDER BY campaign_id ASC",
        )
        .bind_refs(&[D1Type::Text(developer_email)])
        .map_err(|e| format!("D1 list_developer_campaigns bind: {e:?}"))?
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
    let args = [D1Type::Text(campaign_id), D1Type::Text(developer_email)];
    db.prepare(
        "UPDATE developer_campaign_progress \
         SET reward_claimed_at = datetime('now') \
         WHERE campaign_id = ? AND developer_email = ?",
    )
    .bind_refs(&args)
    .map_err(|e| format!("D1 mark_reward_claimed bind: {e:?}"))?
    .run()
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
    let args = [
        D1Type::Text(asset_id),
        D1Type::Text(signature),
        D1Type::Text(campaign_id),
        D1Type::Text(developer_email),
    ];
    db.prepare(
        "UPDATE developer_campaign_progress \
         SET reward_claimed_at = datetime('now'), \
             reward_asset_id = ?, \
             reward_signature = ? \
         WHERE campaign_id = ? AND developer_email = ?",
    )
    .bind_refs(&args)
    .map_err(|e| format!("D1 mark_reward_claimed_with_mint bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 mark_reward_claimed_with_mint: {e:?}"))?;
    Ok(())
}
