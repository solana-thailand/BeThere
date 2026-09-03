//! Campaign ↔ event membership queries.

use worker::{D1Database, D1Type};

use super::types::CampaignEventRow;


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
         VALUES (?, ?, {sequence_order}, {is_required})"
    );
    let args = [D1Type::Text(campaign_id), D1Type::Text(event_id)];
    db.prepare(&sql)
        .bind_refs(&args)
        .map_err(|e| format!("D1 add_campaign_event bind: {e:?}"))?
        .run()
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
    let args = [D1Type::Text(campaign_id), D1Type::Text(event_id)];
    db.prepare("DELETE FROM campaign_events WHERE campaign_id = ? AND event_id = ?")
        .bind_refs(&args)
        .map_err(|e| format!("D1 remove_campaign_event bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 remove_campaign_event: {e:?}"))?;
    Ok(())
}

pub(crate) async fn list_campaign_events(
    db: &D1Database,
    campaign_id: &str,
) -> Result<Vec<CampaignEventRow>, String> {
    let result = db
        .prepare("SELECT * FROM campaign_events WHERE campaign_id = ? ORDER BY sequence_order ASC")
        .bind_refs(&[D1Type::Text(campaign_id)])
        .map_err(|e| format!("D1 list_campaign_events bind: {e:?}"))?
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
    db.prepare("DELETE FROM campaign_events WHERE campaign_id = ?")
        .bind_refs(&[D1Type::Text(campaign_id)])
        .map_err(|e| format!("D1 set_campaign_events delete bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 set_campaign_events delete: {e:?}"))?;

    for (event_id, sequence_order, is_required) in events {
        let sql = format!(
            "INSERT INTO campaign_events (campaign_id, event_id, sequence_order, is_required) \
             VALUES (?, ?, {sequence_order}, {is_required})"
        );
        let args = [D1Type::Text(campaign_id), D1Type::Text(event_id)];
        db.prepare(&sql)
            .bind_refs(&args)
            .map_err(|e| format!("D1 set_campaign_events insert bind: {e:?}"))?
            .run()
            .await
            .map_err(|e| format!("D1 set_campaign_events insert: {e:?}"))?;
    }

    Ok(())
}
