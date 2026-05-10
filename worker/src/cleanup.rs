//! Cron-triggered KV cleanup for expired event data (A3 — sustainability audit).
//!
//! Runs daily at 03:00 UTC via Cloudflare Workers cron trigger.
//! Iterates all events and deletes per-attendee/per-session KV keys whose
//! retention window has expired, then removes fully-expired event configs.
//!
//! Retention policy (measured from `event_end_ms`):
//!
//! | Key pattern                          | Retention               |
//! |--------------------------------------|-------------------------|
//! | `event:{id}:quiz:progress:*`         | event_end + 30 days    |
//! | `event:{id}:adventure:progress:*`    | event_end + 30 days    |
//! | `event:{id}:claim_lock:*`            | event_end + 90 days    |
//! | `event:{id}:deposit:status:*`        | event_end + 90 days    |
//! | `event:{id}:deposit:thb:*`           | event_end + 90 days    |
//! | `event:{id}:deposit:thb:list`        | event_end + 90 days    |
//! | `event:{id}:quiz:questions`          | event_end + 365 days   |
//! | `event:{id}:adventure:config`        | event_end + 365 days   |
//! | `event:{id}` (EventConfig)           | event_end + 365 days   |

use worker::KvStore;

use crate::event_store::{get_event_config, get_event_index, save_event_index};

// ---------------------------------------------------------------------------
// Retention constants (seconds)
// ---------------------------------------------------------------------------

/// Short-lived session data: quiz/adventure progress.
/// Expires 30 days after event end.
const RETENTION_SESSION_SECS: i64 = 30 * 86_400; // 2_592_000

/// Mid-lived financial data: deposits, claim locks.
/// Expires 90 days after event end (aligned with claim_lock finalize TTL).
const RETENTION_FINANCIAL_SECS: i64 = 90 * 86_400; // 7_776_000

/// Long-lived config data: event config, quiz/adventure config.
/// Expires 365 days after event end.
const RETENTION_CONFIG_SECS: i64 = 365 * 86_400; // 31_536_000

// ---------------------------------------------------------------------------
// Cleanup entry point
// ---------------------------------------------------------------------------

/// Run the daily cleanup pass.
///
/// Returns a summary of deleted keys for logging.
pub async fn run_cleanup(kv: &KvStore) -> CleanupSummary {
    let mut summary = CleanupSummary::default();
    let now_ms = chrono::Utc::now().timestamp_millis();

    let index = match get_event_index(kv).await {
        Ok(idx) => idx,
        Err(e) => {
            tracing::error!(error = %e, "cleanup: failed to read event index, aborting");
            return summary;
        }
    };

    let mut index_changed = false;
    let mut updated_events = index.events.clone();

    for meta in &index.events {
        let event_id = &meta.id;
        let event_end_ms = meta.event_end_ms;

        // Load full config for refund_deadline_hours
        let config = get_event_config(kv, event_id).await.unwrap_or(None);
        let refund_deadline_secs = config
            .as_ref()
            .map(|c| i64::from(c.refund_deadline_hours) * 3600)
            .unwrap_or(7 * 86_400); // default 7 days

        let event_end_secs = event_end_ms / 1000;

        // Phase 1: Delete expired session keys (quiz progress, adventure progress)
        let session_cutoff = event_end_secs + RETENTION_SESSION_SECS;
        if now_ms / 1000 > session_cutoff {
            let prefix = format!("event:{event_id}:quiz:progress:");
            summary.quiz_progress_deleted += delete_keys_by_prefix(kv, &prefix).await;

            let prefix = format!("event:{event_id}:adventure:progress:");
            summary.adventure_progress_deleted += delete_keys_by_prefix(kv, &prefix).await;
        }

        // Phase 2: Delete expired financial keys (deposits, claim locks)
        let financial_cutoff = event_end_secs + refund_deadline_secs + RETENTION_FINANCIAL_SECS;
        if now_ms / 1000 > financial_cutoff {
            let prefix = format!("event:{event_id}:claim_lock:");
            summary.claim_locks_deleted += delete_keys_by_prefix(kv, &prefix).await;

            let prefix = format!("event:{event_id}:deposit:status:");
            summary.deposit_status_deleted += delete_keys_by_prefix(kv, &prefix).await;

            let prefix = format!("event:{event_id}:deposit:thb:");
            summary.thb_deposits_deleted += delete_keys_by_prefix(kv, &prefix).await;
        }

        // Phase 3: Delete expired config keys (quiz config, adventure config, event config)
        let config_cutoff = event_end_secs + RETENTION_CONFIG_SECS;
        if now_ms / 1000 > config_cutoff {
            // Quiz questions
            let key = format!("event:{event_id}:quiz:questions");
            if kv.delete(&key).await.is_ok() {
                summary.config_keys_deleted += 1;
            }

            // Adventure config
            let key = format!("event:{event_id}:adventure:config");
            if kv.delete(&key).await.is_ok() {
                summary.config_keys_deleted += 1;
            }

            // Event config
            let key = format!("event:{event_id}");
            if kv.delete(&key).await.is_ok() {
                summary.config_keys_deleted += 1;
            }

            // Remove from index
            updated_events.retain(|e| e.id != *event_id);
            index_changed = true;
            summary.events_removed += 1;

            tracing::info!(event_id = %event_id, "cleanup: fully expired event removed from index");
        }
    }

    // Persist updated index if events were removed
    if index_changed {
        let new_index = event_checkin_domain::models::event::EventIndex {
            events: updated_events,
        };
        if let Err(e) = save_event_index(kv, &new_index).await {
            tracing::error!(error = %e, "cleanup: failed to save updated event index");
        }
    }

    tracing::info!(
        quiz_progress = summary.quiz_progress_deleted,
        adventure_progress = summary.adventure_progress_deleted,
        claim_locks = summary.claim_locks_deleted,
        deposit_status = summary.deposit_status_deleted,
        thb_deposits = summary.thb_deposits_deleted,
        config_keys = summary.config_keys_deleted,
        events_removed = summary.events_removed,
        "cleanup: daily pass complete"
    );

    summary
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Delete all KV keys matching a prefix using paginated list + delete.
async fn delete_keys_by_prefix(kv: &KvStore, prefix: &str) -> usize {
    let mut deleted = 0;
    let mut cursor: Option<String> = None;

    loop {
        let mut builder = kv.list().prefix(prefix.to_string());
        if let Some(c) = cursor.take() {
            builder = builder.cursor(c);
        }

        match builder.execute().await {
            Ok(resp) => {
                for key in &resp.keys {
                    if kv.delete(&key.name).await.is_ok() {
                        deleted += 1;
                    }
                }
                if resp.list_complete {
                    break;
                }
                cursor = resp.cursor;
            }
            Err(e) => {
                tracing::warn!(prefix = %prefix, error = ?e, "cleanup: failed to list keys");
                break;
            }
        }
    }

    deleted
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

/// Summary of a single cleanup pass — returned for logging/testing.
#[derive(Debug, Default)]
pub struct CleanupSummary {
    pub quiz_progress_deleted: usize,
    pub adventure_progress_deleted: usize,
    pub claim_locks_deleted: usize,
    pub deposit_status_deleted: usize,
    pub thb_deposits_deleted: usize,
    pub config_keys_deleted: usize,
    pub events_removed: usize,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retention_constants_are_reasonable() {
        // 30 days
        assert_eq!(RETENTION_SESSION_SECS, 2_592_000);
        // 90 days
        assert_eq!(RETENTION_FINANCIAL_SECS, 7_776_000);
        // 365 days
        assert_eq!(RETENTION_CONFIG_SECS, 31_536_000);
    }

    #[test]
    fn test_cleanup_summary_default() {
        let summary = CleanupSummary::default();
        assert_eq!(summary.quiz_progress_deleted, 0);
        assert_eq!(summary.adventure_progress_deleted, 0);
        assert_eq!(summary.claim_locks_deleted, 0);
        assert_eq!(summary.deposit_status_deleted, 0);
        assert_eq!(summary.thb_deposits_deleted, 0);
        assert_eq!(summary.config_keys_deleted, 0);
        assert_eq!(summary.events_removed, 0);
    }
}
