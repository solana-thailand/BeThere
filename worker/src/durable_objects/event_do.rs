//! EventDurableObject — per-event Durable Object with SQLite storage.
//!
//! Each event gets its own DO instance (sharded by `event_id` as DO name).
//! SQLite inside the DO provides single-threaded, ACID-guaranteed writes.
//! After each write, the changed row is synced to D1 for read availability.
//!
//! Phase 1: Claim lock operations (acquire, finalize, release).
//! Later phases will add check-in, claim, upsert, deposit, refund.

use serde::{Deserialize, Serialize};
use worker::{DurableObject, Env, Request, Response, Result, State, durable_object};

// ---------------------------------------------------------------------------
// RPC request / response types
// ---------------------------------------------------------------------------

/// RPC request enum — Worker sends these as JSON in the DO fetch body.
#[derive(Deserialize, Serialize)]
#[serde(tag = "action")]
#[allow(clippy::enum_variant_names)]
pub(crate) enum DoRequest {
    #[serde(rename = "acquire_claim_lock")]
    AcquireClaimLock {
        lock_id: String,
        event_id: String,
        token: String,
        wallet: String,
        expires_at: String,
    },
    #[serde(rename = "finalize_claim_lock")]
    FinalizeClaimLock {
        event_id: String,
        token: String,
        asset_id: String,
        signature: String,
        claimed_at: String,
    },
    #[serde(rename = "release_claim_lock")]
    ReleaseClaimLock { event_id: String, token: String },
}

/// RPC response — DO returns this as JSON.
#[derive(Serialize)]
struct DoResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl DoResponse {
    fn ok() -> Self {
        Self {
            success: true,
            error: None,
        }
    }
    fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            error: Some(msg.into()),
        }
    }
}

// ---------------------------------------------------------------------------
// Durable Object
// ---------------------------------------------------------------------------

#[durable_object]
pub struct EventDurableObject {
    sql: worker::SqlStorage,
    env: Env,
}

impl DurableObject for EventDurableObject {
    fn new(state: State, env: Env) -> Self {
        let sql = state.storage().sql();
        init_schema(&sql);
        Self { sql, env }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let body: DoRequest = req.json().await?;
        let result = match body {
            DoRequest::AcquireClaimLock {
                lock_id,
                event_id,
                token,
                wallet,
                expires_at,
            } => self.handle_acquire_claim_lock(&lock_id, &event_id, &token, &wallet, &expires_at),

            DoRequest::FinalizeClaimLock {
                event_id,
                token,
                asset_id,
                signature,
                claimed_at,
            } => self.handle_finalize_claim_lock(
                &event_id,
                &token,
                &asset_id,
                &signature,
                &claimed_at,
            ),

            DoRequest::ReleaseClaimLock { event_id, token } => {
                self.handle_release_claim_lock(&event_id, &token)
            }
        };
        Response::from_json(&result)
    }
}

// ---------------------------------------------------------------------------
// Schema initialization
// ---------------------------------------------------------------------------

/// Create SQLite tables inside the DO if they don't exist.
/// Called once per DO instance cold start.
fn init_schema(sql: &worker::SqlStorage) {
    // Claim locks table — mirrors D1 claim_locks schema
    sql.exec(
        "CREATE TABLE IF NOT EXISTS claim_locks ( \
             event_id   TEXT NOT NULL, \
             token      TEXT NOT NULL, \
             lock_id    TEXT NOT NULL, \
             wallet     TEXT NOT NULL, \
             started_at TEXT NOT NULL DEFAULT (datetime('now')), \
             asset_id   TEXT, \
             signature  TEXT, \
             claimed_at TEXT, \
             expires_at TEXT NOT NULL, \
             PRIMARY KEY (event_id, token) \
         )",
        None,
    )
    .expect("DO init: create claim_locks table");

    sql.exec(
        "CREATE INDEX IF NOT EXISTS idx_claim_locks_expires ON claim_locks(expires_at)",
        None,
    )
    .expect("DO init: create claim_locks expires index");

    tracing::info!("DO schema initialized (claim_locks)");
}

// ---------------------------------------------------------------------------
// Claim lock handlers
// ---------------------------------------------------------------------------

impl EventDurableObject {
    /// Acquire a claim lock. Returns success if the lock was acquired (no existing lock).
    /// Single-threaded DO execution guarantees no race condition.
    fn handle_acquire_claim_lock(
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
    fn handle_finalize_claim_lock(
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
    fn handle_release_claim_lock(&self, event_id: &str, token: &str) -> DoResponse {
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

// ---------------------------------------------------------------------------
// D1 sync helpers
// ---------------------------------------------------------------------------

impl EventDurableObject {
    /// Sync a claim lock row from DO SQLite → D1 (fire-and-forget via wait_until).
    fn sync_claim_lock_to_d1(&self, event_id: &str, token: &str) {
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
    fn delete_claim_lock_from_d1(&self, event_id: &str, token: &str) {
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
}

// ---------------------------------------------------------------------------
// Deserialization helpers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ExistingLock {
    #[allow(dead_code)]
    lock_id: String,
    claimed_at: Option<String>,
}

#[derive(Deserialize)]
struct ClaimLockSyncRow {
    lock_id: String,
    event_id: String,
    token: String,
    wallet: String,
    started_at: String,
    asset_id: Option<String>,
    signature: Option<String>,
    claimed_at: Option<String>,
    expires_at: Option<String>,
}
