//! Claim lock handlers for the EventDurableObject.

use super::types::*;
use crate::durable_objects::event_do::EventDurableObject;

impl EventDurableObject {
    /// Acquire a claim lock. Returns success if the lock was acquired (no existing lock).
    /// Single-threaded DO execution guarantees no race condition.
    pub(super) fn handle_acquire_claim_lock(
        &self,
        lock_id: &str,
        event_id: &str,
        token: &str,
        wallet: &str,
        expires_at: &str,
    ) -> DoResponse {
        // Check if a lock already exists for this (event_id, token)
        let existing: Option<ExistingLock> = self
            .sql
            .exec(
                "SELECT lock_id, claimed_at FROM claim_locks WHERE event_id = ?1 AND token = ?2",
                Some(vec![event_id.into(), token.into()]),
            )
            .ok()
            .and_then(|cursor| cursor.one::<ExistingLock>().ok());

        if let Some(lock) = existing {
            // If already claimed (has claimed_at), permanently locked
            if lock.claimed_at.is_some() {
                return DoResponse::err("claim already completed");
            }
            // If not expired yet, still held
            // (expired locks could be cleaned up but for simplicity, reject)
            return DoResponse::err("claim is already being processed");
        }

        // Insert new lock
        let result = self.sql.exec(
            "INSERT INTO claim_locks (lock_id, event_id, token, wallet, expires_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            Some(vec![
                lock_id.into(),
                event_id.into(),
                token.into(),
                wallet.into(),
                expires_at.into(),
            ]),
        );

        match result {
            Ok(cursor) => {
                let written = cursor.rows_written();
                if written > 0 {
                    tracing::info!(
                        lock_id = %lock_id,
                        rows_written = written,
                        "DO: claim lock acquired"
                    );

                    // Async sync to D1
                    self.sync_claim_lock_to_d1(event_id, token);

                    DoResponse::ok()
                } else {
                    DoResponse::err("claim lock insert returned 0 rows")
                }
            }
            Err(e) => DoResponse::err(format!("DO acquire_claim_lock: {e:?}")),
        }
    }

    /// Finalize a claim lock after successful mint.
    pub(super) fn handle_finalize_claim_lock(
        &self,
        event_id: &str,
        token: &str,
        asset_id: &str,
        signature: &str,
        claimed_at: &str,
    ) -> DoResponse {
        let result = self.sql.exec(
            "UPDATE claim_locks \
             SET asset_id = ?1, signature = ?2, claimed_at = ?3, expires_at = NULL \
             WHERE event_id = ?4 AND token = ?5",
            Some(vec![
                asset_id.into(),
                signature.into(),
                claimed_at.into(),
                event_id.into(),
                token.into(),
            ]),
        );

        match result {
            Ok(cursor) => {
                let written = cursor.rows_written();
                if written > 0 {
                    tracing::info!(rows_written = written, "DO: claim lock finalized");

                    // Async sync to D1
                    self.sync_claim_lock_to_d1(event_id, token);

                    DoResponse::ok()
                } else {
                    DoResponse::err("claim lock not found for finalization")
                }
            }
            Err(e) => DoResponse::err(format!("DO finalize_claim_lock: {e:?}")),
        }
    }

    /// Release a claim lock on failure (delete so attendee can retry).
    pub(super) fn handle_release_claim_lock(&self, event_id: &str, token: &str) -> DoResponse {
        let result = self.sql.exec(
            "DELETE FROM claim_locks WHERE event_id = ?1 AND token = ?2",
            Some(vec![event_id.into(), token.into()]),
        );

        match result {
            Ok(cursor) => {
                tracing::info!(
                    rows_written = cursor.rows_written(),
                    "DO: claim lock released"
                );

                // Best-effort: also delete from D1
                self.delete_claim_lock_from_d1(event_id, token);

                DoResponse::ok()
            }
            Err(e) => DoResponse::err(format!("DO release_claim_lock: {e:?}")),
        }
    }
}
