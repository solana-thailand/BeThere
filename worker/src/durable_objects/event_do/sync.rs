//! D1 sync helpers — fire-and-forget DO SQLite → D1 replication.

use super::types::*;
use crate::durable_objects::event_do::EventDurableObject;

impl EventDurableObject {
    /// Sync a claim lock row from DO SQLite → D1 (fire-and-forget via wait_until).
    pub(super) fn sync_claim_lock_to_d1(&self, event_id: &str, token: &str) {
        // Read the current row from DO SQLite
        let lock: Option<ClaimLockSyncRow> = self
            .sql
            .exec(
                "SELECT lock_id, event_id, token, wallet, started_at, \
                        asset_id, signature, claimed_at, expires_at \
                 FROM claim_locks WHERE event_id = ?1 AND token = ?2",
                Some(vec![event_id.into(), token.into()]),
            )
            .ok()
            .and_then(|cursor| cursor.one::<ClaimLockSyncRow>().ok());

        let Some(lock) = lock else {
            tracing::warn!("DO sync: claim lock row not found after write");
            return;
        };

        // Upsert to D1
        if let Ok(d1) = self.env.d1("DB") {
            let result = d1
                .prepare(
                    "INSERT INTO claim_locks (lock_id, event_id, token, wallet, started_at, \
                     asset_id, signature, claimed_at, expires_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
                     ON CONFLICT (event_id, token) DO UPDATE SET \
                     lock_id = excluded.lock_id, wallet = excluded.wallet, \
                     started_at = excluded.started_at, asset_id = excluded.asset_id, \
                     signature = excluded.signature, claimed_at = excluded.claimed_at, \
                     expires_at = excluded.expires_at",
                )
                .bind_refs(&[
                    worker::d1::D1Type::Text(&lock.lock_id),
                    worker::d1::D1Type::Text(&lock.event_id),
                    worker::d1::D1Type::Text(&lock.token),
                    worker::d1::D1Type::Text(&lock.wallet),
                    worker::d1::D1Type::Text(&lock.started_at),
                    lock.asset_id
                        .as_deref()
                        .map(worker::d1::D1Type::Text)
                        .unwrap_or(worker::d1::D1Type::Null),
                    lock.signature
                        .as_deref()
                        .map(worker::d1::D1Type::Text)
                        .unwrap_or(worker::d1::D1Type::Null),
                    lock.claimed_at
                        .as_deref()
                        .map(worker::d1::D1Type::Text)
                        .unwrap_or(worker::d1::D1Type::Null),
                    lock.expires_at
                        .as_deref()
                        .map(worker::d1::D1Type::Text)
                        .unwrap_or(worker::d1::D1Type::Null),
                ]);
            match result {
                Ok(stmt) => {
                    // Use wasm_bindgen_futures to spawn the D1 write
                    let fut = async move {
                        if let Err(e) = stmt.run().await {
                            tracing::warn!("DO→D1 sync claim_lock failed: {e:?}");
                        }
                    };
                    wasm_bindgen_futures::spawn_local(fut);
                }
                Err(e) => {
                    tracing::warn!("DO→D1 sync claim_lock bind failed: {e:?}");
                }
            }
        }
    }

    /// Delete a claim lock from D1 (fire-and-forget).
    pub(super) fn delete_claim_lock_from_d1(&self, event_id: &str, token: &str) {
        if let Ok(d1) = self.env.d1("DB") {
            let result = d1
                .prepare("DELETE FROM claim_locks WHERE event_id = ?1 AND token = ?2")
                .bind_refs(&[
                    worker::d1::D1Type::Text(event_id),
                    worker::d1::D1Type::Text(token),
                ]);
            match result {
                Ok(stmt) => {
                    let fut = async move {
                        if let Err(e) = stmt.run().await {
                            tracing::warn!("DO→D1 delete claim_lock failed: {e:?}");
                        }
                    };
                    wasm_bindgen_futures::spawn_local(fut);
                }
                Err(e) => {
                    tracing::warn!("DO→D1 delete claim_lock bind failed: {e:?}");
                }
            }
        }
    }

    /// Sync an attendee row from DO SQLite → D1 (fire-and-forget).
    pub(super) fn sync_attendee_to_d1(&self, attendee_id: &str) {
        let row: Option<AttendeeSyncRow> = self
            .sql
            .exec(
                "SELECT id, event_id, email, name, approval_status, participation_type, \
                        contact_channel, contact_handle, checked_in_at, checked_in_by, \
                        claim_token, claimed_at, claim_asset_id, claim_signature \
                 FROM attendees WHERE id = ?1",
                Some(vec![attendee_id.into()]),
            )
            .ok()
            .and_then(|cursor| cursor.one::<AttendeeSyncRow>().ok());

        let Some(row) = row else {
            tracing::warn!("DO sync: attendee row not found after write");
            return;
        };

        if let Ok(d1) = self.env.d1("DB") {
            let result = d1
                .prepare(
                    "INSERT INTO attendees (id, event_id, email, name, approval_status, \
                         participation_type, contact_channel, contact_handle, \
                         checked_in_at, checked_in_by, claim_token, \
                         claimed_at, claim_asset_id, claim_signature, \
                         created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, \
                         datetime('now'), datetime('now')) \
                     ON CONFLICT (id) DO UPDATE SET \
                         name = excluded.name, \
                         approval_status = excluded.approval_status, \
                         participation_type = excluded.participation_type, \
                         contact_channel = excluded.contact_channel, \
                         contact_handle = excluded.contact_handle, \
                         checked_in_at = excluded.checked_in_at, \
                         checked_in_by = excluded.checked_in_by, \
                         claim_token = excluded.claim_token, \
                         claimed_at = excluded.claimed_at, \
                         claim_asset_id = excluded.claim_asset_id, \
                         claim_signature = excluded.claim_signature, \
                         updated_at = datetime('now')",
                )
                .bind_refs(&[
                    worker::d1::D1Type::Text(&row.id),
                    worker::d1::D1Type::Text(&row.event_id),
                    worker::d1::D1Type::Text(&row.email),
                    worker::d1::D1Type::Text(&row.name),
                    worker::d1::D1Type::Text(&row.approval_status),
                    worker::d1::D1Type::Text(&row.participation_type),
                    worker::d1::D1Type::Text(&row.contact_channel),
                    worker::d1::D1Type::Text(&row.contact_handle),
                    row.checked_in_at
                        .as_deref()
                        .map(worker::d1::D1Type::Text)
                        .unwrap_or(worker::d1::D1Type::Null),
                    row.checked_in_by
                        .as_deref()
                        .map(worker::d1::D1Type::Text)
                        .unwrap_or(worker::d1::D1Type::Null),
                    row.claim_token
                        .as_deref()
                        .map(worker::d1::D1Type::Text)
                        .unwrap_or(worker::d1::D1Type::Null),
                    row.claimed_at
                        .as_deref()
                        .map(worker::d1::D1Type::Text)
                        .unwrap_or(worker::d1::D1Type::Null),
                    row.claim_asset_id
                        .as_deref()
                        .map(worker::d1::D1Type::Text)
                        .unwrap_or(worker::d1::D1Type::Null),
                    row.claim_signature
                        .as_deref()
                        .map(worker::d1::D1Type::Text)
                        .unwrap_or(worker::d1::D1Type::Null),
                ]);
            match result {
                Ok(stmt) => {
                    let fut = async move {
                        if let Err(e) = stmt.run().await {
                            tracing::warn!("DO→D1 sync attendee failed: {e:?}");
                        }
                    };
                    wasm_bindgen_futures::spawn_local(fut);
                }
                Err(e) => {
                    tracing::warn!("DO→D1 sync attendee bind failed: {e:?}");
                }
            }
        }
    }
}
