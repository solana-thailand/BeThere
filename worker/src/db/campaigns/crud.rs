//! Campaign CRUD queries.

use worker::{D1Database, D1Type};

use super::types::CampaignRow;


#[allow(clippy::too_many_arguments)]
/// Insert a campaign.
///
/// Every value is bound as a parameter, so free-text fields (`title`,
/// `description`, the JSON blobs) cannot break out of the statement. Callers
/// still validate `id` and `status` for shape/domain reasons
/// (`handlers::campaigns::validate_campaign_id` / `validate_create_status`).
pub(crate) async fn create_campaign(
    db: &D1Database,
    id: &str,
    title: &str,
    description: &str,
    organization_id: &str,
    status: &str,
    completion_criteria: &str,
    reward_type: &str,
    reward_config: &str,
) -> Result<(), String> {
    let sql = "INSERT INTO campaigns (id, title, description, organization_id, status, \
         completion_criteria, reward_type, reward_config, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))";
    let args = [
        D1Type::Text(id),
        D1Type::Text(title),
        D1Type::Text(description),
        D1Type::Text(organization_id),
        D1Type::Text(status),
        D1Type::Text(completion_criteria),
        D1Type::Text(reward_type),
        D1Type::Text(reward_config),
    ];
    db.prepare(sql)
        .bind_refs(&args)
        .map_err(|e| format!("D1 create_campaign bind: {e:?}"))?
        .run()
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
    let sql = "UPDATE campaigns SET \
         title = ?, \
         description = ?, \
         completion_criteria = ?, \
         reward_type = ?, \
         reward_config = ?, \
         updated_at = datetime('now') \
         WHERE id = ?";
    let args = [
        D1Type::Text(title),
        D1Type::Text(description),
        D1Type::Text(completion_criteria),
        D1Type::Text(reward_type),
        D1Type::Text(reward_config),
        D1Type::Text(id),
    ];
    db.prepare(sql)
        .bind_refs(&args)
        .map_err(|e| format!("D1 update_campaign bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 update_campaign: {e:?}"))?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) async fn get_campaign(db: &D1Database, id: &str) -> Result<Option<CampaignRow>, String> {
    db.prepare("SELECT * FROM campaigns WHERE id = ? LIMIT 1")
        .bind_refs(&[D1Type::Text(id)])
        .map_err(|e| format!("D1 get_campaign bind: {e:?}"))?
        .first::<CampaignRow>(None)
        .await
        .map_err(|e| format!("D1 get_campaign query: {e:?}"))
}

/// Does a campaign with this id already exist?
///
/// Used by the create-form slug availability probe, so it selects a constant
/// instead of the whole row — nothing about the campaign is needed, only
/// whether the primary key is taken.
///
/// `id` MUST already be shape-validated by the caller (see
/// `handlers::campaigns::is_valid_campaign_id`): like the rest of this module
/// the query is interpolated rather than bound, so an unvalidated id would be
/// an injection vector.
pub(crate) async fn campaign_exists(db: &D1Database, id: &str) -> Result<bool, String> {
    let row = db
        .prepare("SELECT 1 AS present FROM campaigns WHERE id = ? LIMIT 1")
        .bind_refs(&[D1Type::Text(id)])
        .map_err(|e| format!("D1 campaign_exists bind: {e:?}"))?
        .first::<ExistsRow>(None)
        .await
        .map_err(|e| format!("D1 campaign_exists query: {e:?}"))?;
    Ok(row.is_some())
}

/// Single-column probe row for [`campaign_exists`].
#[derive(Debug, serde::Deserialize)]
struct ExistsRow {
    #[allow(dead_code)]
    present: i64,
}

/// Projection row for the `SELECT reward_config` in
/// [`campaign_collection_mints`]. Must list exactly the selected columns — see
/// [`CampaignIdRow`] for why a wider struct is a panic, not an error.
#[derive(Debug, serde::Deserialize)]
struct RewardConfigRow {
    reward_config: String,
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
        .results::<RewardConfigRow>()
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
    // Placeholders and args are pushed together so the two lists cannot drift.
    let mut clauses = Vec::new();
    let mut args = Vec::new();
    if let Some(org) = organization_id {
        clauses.push("organization_id = ?");
        args.push(D1Type::Text(org));
    }
    if let Some(s) = status {
        clauses.push("status = ?");
        args.push(D1Type::Text(s));
    }
    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };
    let sql = format!("SELECT * FROM campaigns {where_clause} ORDER BY created_at DESC");
    let result = db
        .prepare(&sql)
        .bind_refs(&args)
        .map_err(|e| format!("D1 list_campaigns bind: {e:?}"))?
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
    let args = [D1Type::Text(status), D1Type::Text(id)];
    db.prepare("UPDATE campaigns SET status = ?, updated_at = datetime('now') WHERE id = ?")
        .bind_refs(&args)
        .map_err(|e| format!("D1 update_campaign_status bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 update_campaign_status: {e:?}"))?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) async fn delete_campaign(db: &D1Database, id: &str) -> Result<(), String> {
    db.prepare("DELETE FROM campaigns WHERE id = ?")
        .bind_refs(&[D1Type::Text(id)])
        .map_err(|e| format!("D1 delete_campaign bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 delete_campaign: {e:?}"))?;
    Ok(())
}
