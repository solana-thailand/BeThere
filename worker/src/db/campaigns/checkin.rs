//! Auto-progress on check-in (Issue 051 Phase 1).

use worker::{D1Database, D1Type};

use super::progress::upsert_developer_progress;

/// Projection row for `SELECT DISTINCT campaign_id` — one field, because
/// `D1Result::results::<T>()` (worker 0.8.1) `unwrap()`s each row's
/// deserialization. A struct with fields the projection does not select is not
/// a recoverable `Err`, it is an unconditional wasm panic that aborts the whole
/// `wait_until` context. Reusing the 4-field `CampaignEventRow` here is what
/// made campaign auto-progress panic on every check-in.
#[derive(Debug, Clone, serde::Deserialize)]
struct CampaignIdRow {
    campaign_id: String,
}

/// After a successful check-in, update campaign progress for any campaigns that include this event.
/// Non-blocking: errors are logged but don't affect check-in.
pub(crate) async fn on_event_checkin(
    db: &D1Database,
    event_id: &str,
    developer_email: &str,
    developer_fingerprint: &str,
) {
    // 1. Find all campaigns that include this event
    let campaigns_stmt = match db
        .prepare("SELECT DISTINCT campaign_id FROM campaign_events WHERE event_id = ?")
        .bind_refs(&[D1Type::Text(event_id)])
    {
        Ok(stmt) => stmt,
        Err(e) => {
            tracing::warn!(event_id = %event_id, error = %e, "campaign auto-progress: failed to bind campaigns query");
            return;
        }
    };
    let result = match campaigns_stmt.all().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(event_id = %event_id, error = %e, "campaign auto-progress: failed to find campaigns");
            return;
        }
    };

    let campaign_rows = match result.results::<CampaignIdRow>() {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(event_id = %event_id, error = %e, "campaign auto-progress: failed to parse campaign rows");
            return;
        }
    };

    for row in campaign_rows {
        let campaign_id = &row.campaign_id;

        // 2. Count total required events for this campaign
        let total_sql =
            "SELECT COUNT(*) AS cnt FROM campaign_events WHERE campaign_id = ? AND is_required = 1";
        let total_stmt = match db
            .prepare(total_sql)
            .bind_refs(&[D1Type::Text(campaign_id)])
        {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::warn!(campaign_id = %campaign_id, error = %e, "campaign auto-progress: failed to bind total count");
                continue;
            }
        };
        let total_result = match total_stmt.first::<TotalCountRow>(None).await {
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
        let completed_sql = "SELECT COUNT(*) AS cnt FROM campaign_events ce \
             INNER JOIN attendees a ON a.event_id = ce.event_id \
             WHERE ce.campaign_id = ? \
             AND a.email = ? \
             AND a.checked_in_at IS NOT NULL";
        let completed_stmt = match db
            .prepare(completed_sql)
            .bind_refs(&[D1Type::Text(campaign_id), D1Type::Text(developer_email)])
        {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::warn!(campaign_id = %campaign_id, error = %e, "campaign auto-progress: failed to bind completed count");
                continue;
            }
        };
        let completed_result = match completed_stmt.first::<TotalCountRow>(None).await {
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
                developer_fingerprint = %developer_fingerprint,
                error = %e,
                "campaign auto-progress: failed to upsert"
            );
        } else {
            tracing::info!(
                campaign_id = %campaign_id,
                developer_fingerprint = %developer_fingerprint,
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
