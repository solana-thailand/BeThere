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
//! | `event:{id}:audit` entries            | event_end + 90 days    |
//! | `event:{id}:audit` (orphaned)         | removed when event not in index |
//!
//! The D1 side of the financial window (`thb_deposits`, `deposit_statuses`) is
//! deleted on the same schedule, **after** the amounts are copied into the
//! PII-free `thb_deposit_archive` (migration 0044). Keep the money, drop the
//! people — see `.issues/126` for the run that did only the second half.
//! R2 slip images are NOT touched by this module (`.issues/126` §3).

use worker::KvStore;

use crate::audit_store::AuditEntry;
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

/// Audit log entry retention: remove individual entries older than this.
/// Default 90 days from event end.
const RETENTION_AUDIT_SECS: i64 = 90 * 86_400; // 7_776_000

/// On-chain event dedup key retention: remove `onchain:sig:*` keys older than this.
/// Aligned with financial data retention (90 days after event end).
/// Note: dedup keys are cleaned up by `cleanup_onchain_dedup_keys` in daily cron.
#[allow(dead_code)]
const RETENTION_ONCHAIN_DEDUP_SECS: i64 = 90 * 86_400; // 7_776_000

// ---------------------------------------------------------------------------
// Cleanup entry point
// ---------------------------------------------------------------------------

/// Run the daily cleanup pass.
///
/// Returns a summary of deleted keys for logging.
pub async fn run_cleanup(kv: &KvStore, d1: Option<&worker::D1Database>) -> CleanupSummary {
    let mut summary = CleanupSummary::default();
    let now_ms = chrono::Utc::now().timestamp_millis();

    let index = match get_event_index(kv).await {
        Ok(idx) => idx,
        Err(e) => {
            tracing::error!(error = %e, "cleanup: failed to read event index, aborting");
            summary.failures.push(CleanupFailure::EventIndexUnreadable);
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

            // Also delete deposit statuses from D1
            if let Some(db) = d1
                && let Err(e) =
                    crate::db::deposit_statuses::delete_deposit_statuses_for_event(db, event_id)
                        .await
            {
                tracing::warn!(
                    event_id = %event_id,
                    error = %e,
                    "D1 deposit statuses cleanup failed"
                );
            }

            let prefix = format!("event:{event_id}:deposit:thb:");
            summary.thb_deposits_deleted += delete_keys_by_prefix(kv, &prefix).await;

            // Also delete THB deposits from D1 — but ARCHIVE FIRST, and only
            // delete if the archive succeeded.
            //
            // The delete is correct for what this table holds that is personal:
            // `attendee_name`, `bank_account`, `bank_name`, `account_name`, and
            // a URL to an image of a bank slip. It was never correct for the
            // amounts. Before this gate existed the cron took RTM#3's 14
            // deposits (7,000 THB) on 2026-09-19, one of them neither refunded
            // nor held as credit, leaving no record in D1 that the money had
            // ever come in (`.issues/126`). `credit_ledger` survives cleanup but
            // only sees the credit path — 2 rows against those 14.
            //
            // A failed archive must therefore stop the delete, the same way a
            // failed ledger reversal stops the payout clear: the data is gone
            // for good either way, so the only recoverable choice is to leave
            // the rows for tomorrow's run and say so loudly.
            if let Some(db) = d1 {
                match crate::db::thb_deposits::archive_thb_deposits_for_event(db, event_id).await {
                    // The archive must cover every live row before any of them
                    // go. A successful write is not enough on its own: 0044
                    // keyed the archive on (event_id, attendee_id) while
                    // `thb_deposits` has no such constraint, so an event with
                    // two rows for one attendee archived one and deleted both —
                    // 700 THB in the reproduction, reported as success
                    // (`.issues/127`). The key is per row now, and this checks
                    // that no live deposit is missing an archive row carrying
                    // its id — matched, not counted, so it cannot be satisfied
                    // by an archive that is merely large.
                    Ok(coverage) if coverage.is_complete() => {
                        summary.deposits_archived += coverage.archived as usize;
                        if let Err(e) =
                            crate::db::thb_deposits::delete_thb_deposits_for_event(db, event_id)
                                .await
                        {
                            tracing::warn!(
                                event_id = %event_id,
                                error = %e,
                                "D1 THB deposits cleanup failed"
                            );
                        }
                    }
                    Ok(coverage) => {
                        tracing::error!(
                            event_id = %event_id,
                            archived = coverage.archived,
                            unarchived = coverage.unarchived,
                            "D1 THB deposit archive does not cover every live deposit — \
                             deposits NOT deleted, will retry tomorrow"
                        );
                        summary
                            .failures
                            .push(CleanupFailure::DepositArchiveIncomplete {
                                event_id: event_id.clone(),
                                archived: coverage.archived,
                                unarchived: coverage.unarchived,
                            });
                    }
                    Err(e) => {
                        tracing::error!(
                            event_id = %event_id,
                            error = %e,
                            "D1 THB deposit archive failed — deposits NOT deleted, will retry tomorrow"
                        );
                        summary.failures.push(CleanupFailure::DepositArchiveFailed {
                            event_id: event_id.clone(),
                        });
                    }
                }
            }
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

        // Phase 4: Age-based audit log pruning (entries older than 90 days past event end)
        let audit_cutoff = event_end_secs + RETENTION_AUDIT_SECS;
        if now_ms / 1000 > audit_cutoff {
            summary.audit_entries_pruned +=
                prune_old_audit_entries(kv, event_id, audit_cutoff).await;
        }

        // Phase 4b: Delete on-chain event data when financial data expires
        if now_ms / 1000 > financial_cutoff {
            // KV onchain event list (legacy — now in D1, but may still exist)
            let key = format!("event:{event_id}:onchain");
            if kv.delete(&key).await.is_ok() {
                summary.onchain_events_deleted += 1;
            }

            // Clean up polling cursor (legacy KV)
            if let Ok(Some(config)) = crate::event_store::get_event_config(kv, event_id).await
                && !config.escrow_address.is_empty()
            {
                let cursor_key = format!("onchain:cursor:{}", config.escrow_address);
                let _ = kv.delete(&cursor_key).await;
            }

            // D1: delete onchain events for this event
            if let Some(db) = d1
                && let Ok(stmt) = db
                    .prepare("DELETE FROM onchain_events WHERE event_id = ?")
                    .bind_refs(&[worker::d1::D1Type::Text(event_id)])
            {
                let _ = stmt.run().await;
            }
        }
    }

    // Persist updated index if events were removed
    if index_changed {
        let new_index = event_checkin_domain::models::event::EventIndex {
            events: updated_events,
        };
        if let Err(e) = save_event_index(kv, &new_index).await {
            tracing::error!(error = %e, "cleanup: failed to save updated event index");
            summary.failures.push(CleanupFailure::EventIndexUnsaveable);
        }
    }

    // Phase 5: Remove orphaned audit logs (events fully deleted from index)
    let known_ids: std::collections::HashSet<String> =
        index.events.iter().map(|e| e.id.clone()).collect();
    summary.orphaned_audit_deleted = cleanup_orphaned_audit_logs(kv, &known_ids).await;

    // Phase 6: Clean up on-chain dedup
    // KV dedup keys (legacy)
    summary.onchain_dedup_deleted = cleanup_onchain_dedup_keys(kv).await;
    // D1 dedup entries (90 days)
    if let Some(db) = d1
        && let Ok(count) = crate::db::onchain_events::cleanup_old_dedup_entries(db, 90).await
    {
        summary.onchain_dedup_deleted += count;
    }

    // Phase 7: Prune expired JWT blacklist entries
    if let Some(db) = d1
        && let Err(e) = crate::db::jwt_blacklist::cleanup_expired(db).await
    {
        tracing::warn!(error = %e, "JWT blacklist cleanup failed");
    }

    tracing::info!(
        quiz_progress = summary.quiz_progress_deleted,
        adventure_progress = summary.adventure_progress_deleted,
        claim_locks = summary.claim_locks_deleted,
        deposit_status = summary.deposit_status_deleted,
        thb_deposits = summary.thb_deposits_deleted,
        deposits_archived = summary.deposits_archived,
        config_keys = summary.config_keys_deleted,
        events_removed = summary.events_removed,
        audit_entries_pruned = summary.audit_entries_pruned,
        orphaned_audit_deleted = summary.orphaned_audit_deleted,
        onchain_events_deleted = summary.onchain_events_deleted,
        onchain_dedup_deleted = summary.onchain_dedup_deleted,
        jwt_blacklist_deleted = summary.jwt_blacklist_deleted,
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
// Audit cleanup helpers
// ---------------------------------------------------------------------------

/// Remove audit logs for events that no longer exist in the index (orphaned).
///
/// Lists all KV keys with prefix `"event:"` and filters for those ending in `":audit"`.
/// If the extracted event ID is not in `known_ids`, the key is deleted.
pub async fn cleanup_orphaned_audit_logs(
    kv: &KvStore,
    known_ids: &std::collections::HashSet<String>,
) -> usize {
    let mut deleted = 0;
    let mut cursor: Option<String> = None;

    loop {
        let mut builder = kv.list().prefix("event:".to_string());
        if let Some(c) = cursor.take() {
            builder = builder.cursor(c);
        }

        match builder.execute().await {
            Ok(resp) => {
                for key in &resp.keys {
                    let name = &key.name;
                    // Match pattern: event:{id}:audit
                    if name.ends_with(":audit") && name.starts_with("event:") {
                        // Extract event ID between "event:" and ":audit"
                        let maybe_id = &name["event:".len()..name.len() - ":audit".len()];
                        // Skip if the ID contains ':' — it's a nested key like event:{id}:claim_lock:...
                        if maybe_id.contains(':') {
                            continue;
                        }
                        if !known_ids.contains(maybe_id) && kv.delete(name).await.is_ok() {
                            deleted += 1;
                            tracing::info!(
                                event_id = %maybe_id,
                                "cleanup: deleted orphaned audit log"
                            );
                        }
                    }
                }
                if resp.list_complete {
                    break;
                }
                cursor = resp.cursor;
            }
            Err(e) => {
                tracing::warn!(error = ?e, "cleanup: failed to list keys for orphaned audit check");
                break;
            }
        }
    }

    deleted
}

/// Prune individual audit entries older than `cutoff_epoch_secs`.
///
/// Reads the audit log for `event_id`, removes entries whose `timestamp`
/// parses to a datetime before `cutoff_epoch_secs`, and writes back.
async fn prune_old_audit_entries(kv: &KvStore, event_id: &str, cutoff_epoch_secs: i64) -> usize {
    let key = format!("event:{event_id}:audit");
    let raw: Option<String> = match kv.get(&key).text().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(event_id = %event_id, error = ?e, "cleanup: audit read failed");
            return 0;
        }
    };

    let json = match raw {
        None => return 0,
        Some(j) => j,
    };

    let entries: Vec<AuditEntry> = match serde_json::from_str(&json) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(event_id = %event_id, error = ?e, "cleanup: audit parse failed");
            return 0;
        }
    };

    let original_len = entries.len();
    let retained: Vec<AuditEntry> = entries
        .into_iter()
        .filter(|e| {
            // Parse ISO 8601 timestamp and compare against cutoff
            chrono::DateTime::parse_from_rfc3339(&e.timestamp)
                .map(|dt| dt.timestamp() >= cutoff_epoch_secs)
                .unwrap_or(true) // keep entries with unparseable timestamps
        })
        .collect();

    let pruned = original_len.saturating_sub(retained.len());
    if pruned == 0 {
        return 0;
    }

    // Write back the retained entries
    let new_json = match serde_json::to_string(&retained) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(event_id = %event_id, error = ?e, "cleanup: audit serialize failed");
            return 0;
        }
    };

    // Use the same pattern as audit_store::write_entries
    match kv.put(&key, &new_json) {
        Ok(builder) => {
            if let Err(e) = builder.execute().await {
                tracing::warn!(event_id = %event_id, error = ?e, "cleanup: audit write-back failed");
                return 0;
            }
        }
        Err(e) => {
            tracing::warn!(event_id = %event_id, error = ?e, "cleanup: audit put failed");
            return 0;
        }
    }

    tracing::info!(
        event_id = %event_id,
        pruned = pruned,
        retained = retained.len(),
        "cleanup: pruned old audit entries"
    );

    pruned
}

// ---------------------------------------------------------------------------
// On-chain event cleanup
// ---------------------------------------------------------------------------

/// Clean up on-chain event dedup keys (`onchain:sig:*`).
///
/// These keys don't have timestamps, so we simply delete all of them
/// during the daily cron. They're short-lived markers used to prevent
/// duplicate indexing — they're recreated as needed.
pub async fn cleanup_onchain_dedup_keys(kv: &KvStore) -> usize {
    delete_keys_by_prefix(kv, "onchain:sig:").await
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

/// A cleanup phase that could not do its job.
///
/// Deliberately a closed enum and not a free-form string: these travel to Slack,
/// and a raw D1/KV error can echo a bound value back out of the isolate. The
/// detail stays in `tracing`; this is the alertable shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupFailure {
    /// The event index could not be read — the entire run aborted.
    EventIndexUnreadable,
    /// The pruned index could not be written back; removals retry tomorrow.
    EventIndexUnsaveable,
    /// The archive query failed for one event; its deposits were not deleted.
    DepositArchiveFailed { event_id: String },
    /// The archive does not carry every live deposit id, so the delete was
    /// refused (`.issues/127`). Nothing was lost — but it will repeat nightly.
    DepositArchiveIncomplete {
        event_id: String,
        archived: i64,
        unarchived: i64,
    },
}

impl std::fmt::Display for CleanupFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EventIndexUnreadable => {
                write!(f, "event index unreadable — the whole run aborted")
            }
            Self::EventIndexUnsaveable => {
                write!(f, "pruned event index could not be saved")
            }
            Self::DepositArchiveFailed { event_id } => {
                write!(f, "deposit archive query failed for event {event_id}")
            }
            Self::DepositArchiveIncomplete {
                event_id,
                archived,
                unarchived,
            } => write!(
                f,
                "deposit archive incomplete for event {event_id} — {archived} archived, \
                 {unarchived} live deposit(s) unmatched, delete refused"
            ),
        }
    }
}

/// Summary of a single cleanup pass — returned for logging/testing.
#[derive(Debug, Default)]
pub struct CleanupSummary {
    pub quiz_progress_deleted: usize,
    pub adventure_progress_deleted: usize,
    pub claim_locks_deleted: usize,
    pub deposit_status_deleted: usize,
    pub thb_deposits_deleted: usize,
    /// Rows the PII-free `thb_deposit_archive` holds for the events touched this
    /// run. Not "rows inserted": the archive is idempotent, so a re-run reports
    /// the same coverage rather than zero.
    pub deposits_archived: usize,
    pub config_keys_deleted: usize,
    pub events_removed: usize,
    pub audit_entries_pruned: usize,
    pub orphaned_audit_deleted: usize,
    pub onchain_events_deleted: usize,
    pub onchain_dedup_deleted: usize,
    pub jwt_blacklist_deleted: usize,
    /// Phases that failed this run. Empty on a healthy pass.
    ///
    /// Exists because every one of these sites used to write only to `tracing`:
    /// a malformed index aborted staging's cleanup every night from 2026-09-19
    /// and nobody knew until someone went looking. The daily cron alerts on a
    /// non-empty list, matching the credit-ledger and NFT-journal reconciles.
    pub failures: Vec<CleanupFailure>,
}

impl CleanupSummary {
    /// Every phase completed. Mirrors the other nightly reconcile reports.
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty()
    }
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
        // 90 days (audit)
        assert_eq!(RETENTION_AUDIT_SECS, 7_776_000);
        // 90 days (onchain dedup)
        assert_eq!(RETENTION_ONCHAIN_DEDUP_SECS, 7_776_000);
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
        assert_eq!(summary.audit_entries_pruned, 0);
        assert_eq!(summary.orphaned_audit_deleted, 0);
        assert_eq!(summary.onchain_events_deleted, 0);
        assert_eq!(summary.onchain_dedup_deleted, 0);
    }
}
