//! EventDurableObject — per-event Durable Object with SQLite storage.
//!
//! Each event gets its own DO instance (sharded by `event_id` as DO name).
//! SQLite inside the DO provides single-threaded, ACID-guaranteed writes.
//! After each write, the changed row is synced to D1 for read availability.
//!
//! Phase 1: Claim lock operations (acquire, finalize, release).
//! Phase 2: Check-in, undo check-in, claim attendee, upsert attendee.
//! Later phases will add deposit, refund.

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
    // ── Phase 1: Claim lock operations ──
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

    // ── Phase 2: Check-in & claim operations ──
    #[serde(rename = "check_in")]
    CheckIn {
        attendee_id: String,
        event_id: String,
        checked_in_at: String,
        checked_in_by: String,
        claim_token: String,
    },
    #[serde(rename = "undo_check_in")]
    UndoCheckIn {
        attendee_id: String,
        event_id: String,
    },
    #[serde(rename = "claim_attendee")]
    ClaimAttendee {
        event_id: String,
        claim_token: String,
        claimed_at: String,
        claim_asset_id: String,
        claim_signature: String,
    },
    #[serde(rename = "upsert_attendee")]
    UpsertAttendee {
        id: String,
        event_id: String,
        email: String,
        name: String,
        approval_status: String,
        participation_type: String,
        contact_channel: String,
        contact_handle: String,
    },
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
            // Phase 1: Claim lock operations
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

            // Phase 2: Check-in & claim operations
            DoRequest::CheckIn {
                attendee_id,
                event_id,
                checked_in_at,
                checked_in_by,
                claim_token,
            } => self.handle_check_in(
                &attendee_id,
                &event_id,
                &checked_in_at,
                &checked_in_by,
                &claim_token,
            ),

            DoRequest::UndoCheckIn {
                attendee_id,
                event_id,
            } => self.handle_undo_check_in(&attendee_id, &event_id),

            DoRequest::ClaimAttendee {
                event_id,
                claim_token,
                claimed_at,
                claim_asset_id,
                claim_signature,
            } => self.handle_claim_attendee(
                &event_id,
                &claim_token,
                &claimed_at,
                &claim_asset_id,
                &claim_signature,
            ),

            DoRequest::UpsertAttendee {
                id,
                event_id,
                email,
                name,
                approval_status,
                participation_type,
                contact_channel,
                contact_handle,
            } => self.handle_upsert_attendee(UpsertAttendeeParams {
                id: &id,
                event_id: &event_id,
                email: &email,
                name: &name,
                approval_status: &approval_status,
                participation_type: &participation_type,
                contact_channel: &contact_channel,
                contact_handle: &contact_handle,
            }),
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

    // Attendees table — mirrors D1 attendees schema for Phase 2 operations
    sql.exec(
        "CREATE TABLE IF NOT EXISTS attendees ( \
             id                TEXT PRIMARY KEY, \
             event_id          TEXT NOT NULL, \
             email             TEXT NOT NULL, \
             name              TEXT NOT NULL, \
             approval_status   TEXT NOT NULL DEFAULT 'pending', \
             participation_type TEXT NOT NULL DEFAULT 'In-Person', \
             contact_channel   TEXT NOT NULL DEFAULT '', \
             contact_handle    TEXT NOT NULL DEFAULT '', \
             checked_in_at     TEXT, \
             checked_in_by     TEXT, \
             claim_token       TEXT, \
             claimed_at        TEXT, \
             claim_asset_id    TEXT, \
             claim_signature   TEXT, \
             created_at        TEXT NOT NULL DEFAULT (datetime('now')), \
             updated_at        TEXT NOT NULL DEFAULT (datetime('now')) \
         )",
        None,
    )
    .expect("DO init: create attendees table");

    sql.exec(
        "CREATE INDEX IF NOT EXISTS idx_attendees_event ON attendees(event_id)",
        None,
    )
    .expect("DO init: create attendees event index");

    sql.exec(
        "CREATE INDEX IF NOT EXISTS idx_attendees_claim_token ON attendees(claim_token)",
        None,
    )
    .expect("DO init: create attendees claim_token index");

    tracing::info!("DO schema initialized (claim_locks, attendees)");
}

/// Parameters for the `UpsertAttendee` DO RPC.
pub(crate) struct UpsertAttendeeParams<'a> {
    pub(crate) id: &'a str,
    pub(crate) event_id: &'a str,
    pub(crate) email: &'a str,
    pub(crate) name: &'a str,
    pub(crate) approval_status: &'a str,
    pub(crate) participation_type: &'a str,
    pub(crate) contact_channel: &'a str,
    pub(crate) contact_handle: &'a str,
}

// ---------------------------------------------------------------------------
// Deserialization helpers (Phase 1)
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

// ---------------------------------------------------------------------------
// Deserialization helpers (Phase 2)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ExistingCheckIn {
    checked_in_at: Option<String>,
    claim_token: Option<String>,
}

#[derive(Deserialize)]
struct AttendeeSyncRow {
    id: String,
    event_id: String,
    email: String,
    name: String,
    approval_status: String,
    participation_type: String,
    contact_channel: String,
    contact_handle: String,
    checked_in_at: Option<String>,
    checked_in_by: Option<String>,
    claim_token: Option<String>,
    claimed_at: Option<String>,
    claim_asset_id: Option<String>,
    claim_signature: Option<String>,
}

#[derive(Deserialize)]
struct IdRow {
    id: String,
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
// Phase 2: Check-in & claim handlers
// ---------------------------------------------------------------------------

impl EventDurableObject {
    /// Check in an attendee — sets checked_in_at, checked_in_by, claim_token.
    /// Idempotent: if already checked in with the same claim_token, returns success.
    /// If checked in with a different claim_token, returns error.
    fn handle_check_in(
        &self,
        attendee_id: &str,
        _event_id: &str,
        checked_in_at: &str,
        checked_in_by: &str,
        claim_token: &str,
    ) -> DoResponse {
        // Check existing state
        let existing: Option<ExistingCheckIn> = self
            .sql
            .exec(
                "SELECT checked_in_at, claim_token FROM attendees WHERE id = ?1",
                Some(vec![attendee_id.into()]),
            )
            .ok()
            .and_then(|cursor| cursor.one::<ExistingCheckIn>().ok());

        match existing {
            Some(row) if row.checked_in_at.is_some() => {
                // Already checked in — idempotent if same token
                if row.claim_token.as_deref() == Some(claim_token) {
                    tracing::info!(
                        attendee_id = %attendee_id,
                        "DO: check-in idempotent (already checked in)"
                    );
                    return DoResponse::ok();
                }
                return DoResponse::err("attendee is already checked in");
            }
            None => {
                return DoResponse::err("attendee not found");
            }
            _ => {} // Not checked in yet, proceed
        }

        let result = self.sql.exec(
            "UPDATE attendees \
             SET checked_in_at = ?1, checked_in_by = ?2, claim_token = ?3, \
                 updated_at = datetime('now') \
             WHERE id = ?4",
            Some(vec![
                checked_in_at.into(),
                checked_in_by.into(),
                claim_token.into(),
                attendee_id.into(),
            ]),
        );

        match result {
            Ok(cursor) => {
                let written = cursor.rows_written();
                if written > 0 {
                    tracing::info!(
                        attendee_id = %attendee_id,
                        rows_written = written,
                        "DO: attendee checked in"
                    );
                    self.sync_attendee_to_d1(attendee_id);
                    DoResponse::ok()
                } else {
                    DoResponse::err("check-in update matched 0 rows")
                }
            }
            Err(e) => DoResponse::err(format!("DO check_in: {e:?}")),
        }
    }

    /// Undo a check-in — clears checked_in_at, checked_in_by, claim_token.
    fn handle_undo_check_in(&self, attendee_id: &str, _event_id: &str) -> DoResponse {
        let result = self.sql.exec(
            "UPDATE attendees \
             SET checked_in_at = NULL, checked_in_by = NULL, claim_token = NULL, \
                 updated_at = datetime('now') \
             WHERE id = ?1",
            Some(vec![attendee_id.into()]),
        );

        match result {
            Ok(cursor) => {
                tracing::info!(
                    attendee_id = %attendee_id,
                    rows_written = cursor.rows_written(),
                    "DO: check-in undone"
                );
                self.sync_attendee_to_d1(attendee_id);
                DoResponse::ok()
            }
            Err(e) => DoResponse::err(format!("DO undo_check_in: {e:?}")),
        }
    }

    /// Mark an attendee as claimed after NFT mint.
    fn handle_claim_attendee(
        &self,
        event_id: &str,
        claim_token: &str,
        claimed_at: &str,
        claim_asset_id: &str,
        claim_signature: &str,
    ) -> DoResponse {
        let result = self.sql.exec(
            "UPDATE attendees \
             SET claimed_at = ?1, claim_asset_id = ?2, claim_signature = ?3, \
                 updated_at = datetime('now') \
             WHERE event_id = ?4 AND claim_token = ?5",
            Some(vec![
                claimed_at.into(),
                claim_asset_id.into(),
                claim_signature.into(),
                event_id.into(),
                claim_token.into(),
            ]),
        );

        match result {
            Ok(cursor) => {
                let written = cursor.rows_written();
                if written > 0 {
                    tracing::info!(rows_written = written, "DO: attendee claimed");
                    // Look up attendee_id for D1 sync
                    let id = self.resolve_attendee_id_by_claim_token(event_id, claim_token);
                    if let Some(id) = id {
                        self.sync_attendee_to_d1(&id);
                    }
                    DoResponse::ok()
                } else {
                    DoResponse::err("claim update matched 0 rows — attendee not found")
                }
            }
            Err(e) => DoResponse::err(format!("DO claim_attendee: {e:?}")),
        }
    }

    /// Insert or update an attendee. Idempotent upsert by `id`.
    fn handle_upsert_attendee(&self, p: UpsertAttendeeParams<'_>) -> DoResponse {
        let result = self.sql.exec(
            "INSERT INTO attendees (id, event_id, email, name, approval_status, \
                 participation_type, contact_channel, contact_handle, \
                 created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'), datetime('now')) \
             ON CONFLICT (id) DO UPDATE SET \
                 name = excluded.name, \
                 approval_status = excluded.approval_status, \
                 participation_type = excluded.participation_type, \
                 contact_channel = excluded.contact_channel, \
                 contact_handle = excluded.contact_handle, \
                 updated_at = datetime('now')",
            Some(vec![
                p.id.into(),
                p.event_id.into(),
                p.email.into(),
                p.name.into(),
                p.approval_status.into(),
                p.participation_type.into(),
                p.contact_channel.into(),
                p.contact_handle.into(),
            ]),
        );

        match result {
            Ok(cursor) => {
                tracing::info!(
                    id = %p.id,
                    rows_written = cursor.rows_written(),
                    "DO: attendee upserted"
                );
                self.sync_attendee_to_d1(p.id);
                DoResponse::ok()
            }
            Err(e) => DoResponse::err(format!("DO upsert_attendee: {e:?}")),
        }
    }

    /// Resolve attendee id by (event_id, claim_token).
    fn resolve_attendee_id_by_claim_token(
        &self,
        event_id: &str,
        claim_token: &str,
    ) -> Option<String> {
        self.sql
            .exec(
                "SELECT id FROM attendees WHERE event_id = ?1 AND claim_token = ?2",
                Some(vec![event_id.into(), claim_token.into()]),
            )
            .ok()
            .and_then(|cursor| cursor.one::<IdRow>().ok())
            .map(|r| r.id)
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

    /// Sync an attendee row from DO SQLite → D1 (fire-and-forget).
    fn sync_attendee_to_d1(&self, attendee_id: &str) {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: serialize then deserialize and verify equality.
    fn assert_round_trip(expected_json: &str, value: &DoRequest) {
        let serialized =
            serde_json::to_string(value).unwrap_or_else(|e| panic!("Failed to serialize: {e}"));
        assert_eq!(serialized, expected_json, "serialization mismatch");

        let deserialized: DoRequest = serde_json::from_str(expected_json)
            .unwrap_or_else(|e| panic!("Failed to deserialize: {e}"));
        assert_eq!(
            serde_json::to_string(&deserialized).unwrap(),
            expected_json,
            "round-trip deserialization mismatch"
        );
    }

    // ==========================================================================
    // AcquireClaimLock serde
    // ==========================================================================

    #[test]
    fn acquire_claim_lock_round_trip() {
        let json = r#"{"action":"acquire_claim_lock","lock_id":"lid-1","event_id":"evt-1","token":"tok-1","wallet":"wallet-addr","expires_at":"2025-01-01T00:00:00Z"}"#;
        let value = DoRequest::AcquireClaimLock {
            lock_id: "lid-1".to_string(),
            event_id: "evt-1".to_string(),
            token: "tok-1".to_string(),
            wallet: "wallet-addr".to_string(),
            expires_at: "2025-01-01T00:00:00Z".to_string(),
        };
        assert_round_trip(json, &value);
    }

    #[test]
    fn acquire_claim_lock_action_tag_is_snake_case() {
        let value = DoRequest::AcquireClaimLock {
            lock_id: "lid".to_string(),
            event_id: "evt".to_string(),
            token: "tok".to_string(),
            wallet: "w".to_string(),
            expires_at: "exp".to_string(),
        };
        let serialized = serde_json::to_string(&value).unwrap();
        assert!(
            serialized.contains("\"action\":\"acquire_claim_lock\""),
            "action tag must be acquire_claim_lock, got: {serialized}"
        );
    }

    #[test]
    fn acquire_claim_lock_all_fields_present() {
        let value = DoRequest::AcquireClaimLock {
            lock_id: "lid".to_string(),
            event_id: "evt".to_string(),
            token: "tok".to_string(),
            wallet: "w".to_string(),
            expires_at: "exp".to_string(),
        };
        let serialized = serde_json::to_string(&value).unwrap();
        let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert!(v.get("lock_id").is_some(), "missing lock_id");
        assert!(v.get("event_id").is_some(), "missing event_id");
        assert!(v.get("token").is_some(), "missing token");
        assert!(v.get("wallet").is_some(), "missing wallet");
        assert!(v.get("expires_at").is_some(), "missing expires_at");
    }

    // ==========================================================================
    // FinalizeClaimLock serde
    // ==========================================================================

    #[test]
    fn finalize_claim_lock_round_trip() {
        let json = r#"{"action":"finalize_claim_lock","event_id":"evt-1","token":"tok-1","asset_id":"asset-1","signature":"sig-1","claimed_at":"2025-01-01T00:00:00Z"}"#;
        let value = DoRequest::FinalizeClaimLock {
            event_id: "evt-1".to_string(),
            token: "tok-1".to_string(),
            asset_id: "asset-1".to_string(),
            signature: "sig-1".to_string(),
            claimed_at: "2025-01-01T00:00:00Z".to_string(),
        };
        assert_round_trip(json, &value);
    }

    #[test]
    fn finalize_claim_lock_action_tag_is_snake_case() {
        let value = DoRequest::FinalizeClaimLock {
            event_id: "evt".to_string(),
            token: "tok".to_string(),
            asset_id: "a".to_string(),
            signature: "s".to_string(),
            claimed_at: "c".to_string(),
        };
        let serialized = serde_json::to_string(&value).unwrap();
        assert!(
            serialized.contains("\"action\":\"finalize_claim_lock\""),
            "action tag must be finalize_claim_lock, got: {serialized}"
        );
    }

    #[test]
    fn finalize_claim_lock_all_fields_present() {
        let value = DoRequest::FinalizeClaimLock {
            event_id: "evt".to_string(),
            token: "tok".to_string(),
            asset_id: "a".to_string(),
            signature: "s".to_string(),
            claimed_at: "c".to_string(),
        };
        let serialized = serde_json::to_string(&value).unwrap();
        let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert!(v.get("event_id").is_some(), "missing event_id");
        assert!(v.get("token").is_some(), "missing token");
        assert!(v.get("asset_id").is_some(), "missing asset_id");
        assert!(v.get("signature").is_some(), "missing signature");
        assert!(v.get("claimed_at").is_some(), "missing claimed_at");
    }

    // ==========================================================================
    // ReleaseClaimLock serde
    // ==========================================================================

    #[test]
    fn release_claim_lock_round_trip() {
        let json = r#"{"action":"release_claim_lock","event_id":"evt-1","token":"tok-1"}"#;
        let value = DoRequest::ReleaseClaimLock {
            event_id: "evt-1".to_string(),
            token: "tok-1".to_string(),
        };
        assert_round_trip(json, &value);
    }

    #[test]
    fn release_claim_lock_action_tag_is_snake_case() {
        let value = DoRequest::ReleaseClaimLock {
            event_id: "evt".to_string(),
            token: "tok".to_string(),
        };
        let serialized = serde_json::to_string(&value).unwrap();
        assert!(
            serialized.contains("\"action\":\"release_claim_lock\""),
            "action tag must be release_claim_lock, got: {serialized}"
        );
    }

    #[test]
    fn release_claim_lock_all_fields_present() {
        let value = DoRequest::ReleaseClaimLock {
            event_id: "evt".to_string(),
            token: "tok".to_string(),
        };
        let serialized = serde_json::to_string(&value).unwrap();
        let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert!(v.get("event_id").is_some(), "missing event_id");
        assert!(v.get("token").is_some(), "missing token");
    }

    // ==========================================================================
    // Unknown action rejection
    // ==========================================================================

    #[test]
    fn unknown_action_rejected() {
        let json = r#"{"action":"do_something","event_id":"evt","token":"tok"}"#;
        let result = serde_json::from_str::<DoRequest>(json);
        assert!(
            result.is_err(),
            "expected deserialization failure for unknown action"
        );
    }

    #[test]
    fn missing_action_rejected() {
        let json = r#"{"event_id":"evt","token":"tok"}"#;
        let result = serde_json::from_str::<DoRequest>(json);
        assert!(
            result.is_err(),
            "expected deserialization failure for missing action"
        );
    }

    // ==========================================================================
    // DoResponse
    // ==========================================================================

    #[test]
    fn do_response_ok_serializes() {
        let resp = DoResponse::ok();
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, "{\"success\":true}");
    }

    #[test]
    fn do_response_err_serializes() {
        let resp = DoResponse::err("something went wrong");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error\":\"something went wrong\""));
    }

    // ==========================================================================
    // Phase 2: CheckIn serde
    // ==========================================================================

    #[test]
    fn check_in_round_trip() {
        let json = r#"{"action":"check_in","attendee_id":"att-1","event_id":"evt-1","checked_in_at":"2025-06-01T10:00:00Z","checked_in_by":"staff@test.com","claim_token":"tok-1"}"#;
        let value = DoRequest::CheckIn {
            attendee_id: "att-1".to_string(),
            event_id: "evt-1".to_string(),
            checked_in_at: "2025-06-01T10:00:00Z".to_string(),
            checked_in_by: "staff@test.com".to_string(),
            claim_token: "tok-1".to_string(),
        };
        assert_round_trip(json, &value);
    }

    #[test]
    fn check_in_action_tag_is_snake_case() {
        let value = DoRequest::CheckIn {
            attendee_id: "a".to_string(),
            event_id: "e".to_string(),
            checked_in_at: "t".to_string(),
            checked_in_by: "s".to_string(),
            claim_token: "c".to_string(),
        };
        let serialized = serde_json::to_string(&value).unwrap();
        assert!(
            serialized.contains("\"action\":\"check_in\""),
            "action tag must be check_in, got: {serialized}"
        );
    }

    #[test]
    fn check_in_all_fields_present() {
        let value = DoRequest::CheckIn {
            attendee_id: "a".to_string(),
            event_id: "e".to_string(),
            checked_in_at: "t".to_string(),
            checked_in_by: "s".to_string(),
            claim_token: "c".to_string(),
        };
        let serialized = serde_json::to_string(&value).unwrap();
        let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert!(v.get("attendee_id").is_some(), "missing attendee_id");
        assert!(v.get("event_id").is_some(), "missing event_id");
        assert!(v.get("checked_in_at").is_some(), "missing checked_in_at");
        assert!(v.get("checked_in_by").is_some(), "missing checked_in_by");
        assert!(v.get("claim_token").is_some(), "missing claim_token");
    }

    // ==========================================================================
    // Phase 2: UndoCheckIn serde
    // ==========================================================================

    #[test]
    fn undo_check_in_round_trip() {
        let json = r#"{"action":"undo_check_in","attendee_id":"att-1","event_id":"evt-1"}"#;
        let value = DoRequest::UndoCheckIn {
            attendee_id: "att-1".to_string(),
            event_id: "evt-1".to_string(),
        };
        assert_round_trip(json, &value);
    }

    #[test]
    fn undo_check_in_action_tag_is_snake_case() {
        let value = DoRequest::UndoCheckIn {
            attendee_id: "a".to_string(),
            event_id: "e".to_string(),
        };
        let serialized = serde_json::to_string(&value).unwrap();
        assert!(
            serialized.contains("\"action\":\"undo_check_in\""),
            "action tag must be undo_check_in, got: {serialized}"
        );
    }

    // ==========================================================================
    // Phase 2: ClaimAttendee serde
    // ==========================================================================

    #[test]
    fn claim_attendee_round_trip() {
        let json = r#"{"action":"claim_attendee","event_id":"evt-1","claim_token":"tok-1","claimed_at":"2025-06-01T12:00:00Z","claim_asset_id":"asset-1","claim_signature":"sig-1"}"#;
        let value = DoRequest::ClaimAttendee {
            event_id: "evt-1".to_string(),
            claim_token: "tok-1".to_string(),
            claimed_at: "2025-06-01T12:00:00Z".to_string(),
            claim_asset_id: "asset-1".to_string(),
            claim_signature: "sig-1".to_string(),
        };
        assert_round_trip(json, &value);
    }

    #[test]
    fn claim_attendee_action_tag_is_snake_case() {
        let value = DoRequest::ClaimAttendee {
            event_id: "e".to_string(),
            claim_token: "t".to_string(),
            claimed_at: "c".to_string(),
            claim_asset_id: "a".to_string(),
            claim_signature: "s".to_string(),
        };
        let serialized = serde_json::to_string(&value).unwrap();
        assert!(
            serialized.contains("\"action\":\"claim_attendee\""),
            "action tag must be claim_attendee, got: {serialized}"
        );
    }

    #[test]
    fn claim_attendee_all_fields_present() {
        let value = DoRequest::ClaimAttendee {
            event_id: "e".to_string(),
            claim_token: "t".to_string(),
            claimed_at: "c".to_string(),
            claim_asset_id: "a".to_string(),
            claim_signature: "s".to_string(),
        };
        let serialized = serde_json::to_string(&value).unwrap();
        let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert!(v.get("event_id").is_some(), "missing event_id");
        assert!(v.get("claim_token").is_some(), "missing claim_token");
        assert!(v.get("claimed_at").is_some(), "missing claimed_at");
        assert!(v.get("claim_asset_id").is_some(), "missing claim_asset_id");
        assert!(
            v.get("claim_signature").is_some(),
            "missing claim_signature"
        );
    }

    // ==========================================================================
    // Phase 2: UpsertAttendee serde
    // ==========================================================================

    #[test]
    fn upsert_attendee_round_trip() {
        let json = r#"{"action":"upsert_attendee","id":"att-1","event_id":"evt-1","email":"test@test.com","name":"Test User","approval_status":"approved","participation_type":"In-Person","contact_channel":"telegram","contact_handle":"@test"}"#;
        let value = DoRequest::UpsertAttendee {
            id: "att-1".to_string(),
            event_id: "evt-1".to_string(),
            email: "test@test.com".to_string(),
            name: "Test User".to_string(),
            approval_status: "approved".to_string(),
            participation_type: "In-Person".to_string(),
            contact_channel: "telegram".to_string(),
            contact_handle: "@test".to_string(),
        };
        assert_round_trip(json, &value);
    }

    #[test]
    fn upsert_attendee_action_tag_is_snake_case() {
        let value = DoRequest::UpsertAttendee {
            id: "a".to_string(),
            event_id: "e".to_string(),
            email: "t@t.com".to_string(),
            name: "n".to_string(),
            approval_status: "p".to_string(),
            participation_type: "i".to_string(),
            contact_channel: "c".to_string(),
            contact_handle: "h".to_string(),
        };
        let serialized = serde_json::to_string(&value).unwrap();
        assert!(
            serialized.contains("\"action\":\"upsert_attendee\""),
            "action tag must be upsert_attendee, got: {serialized}"
        );
    }

    #[test]
    fn upsert_attendee_all_fields_present() {
        let value = DoRequest::UpsertAttendee {
            id: "a".to_string(),
            event_id: "e".to_string(),
            email: "t@t.com".to_string(),
            name: "n".to_string(),
            approval_status: "p".to_string(),
            participation_type: "i".to_string(),
            contact_channel: "c".to_string(),
            contact_handle: "h".to_string(),
        };
        let serialized = serde_json::to_string(&value).unwrap();
        let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert!(v.get("id").is_some(), "missing id");
        assert!(v.get("event_id").is_some(), "missing event_id");
        assert!(v.get("email").is_some(), "missing email");
        assert!(v.get("name").is_some(), "missing name");
        assert!(
            v.get("approval_status").is_some(),
            "missing approval_status"
        );
        assert!(
            v.get("participation_type").is_some(),
            "missing participation_type"
        );
        assert!(
            v.get("contact_channel").is_some(),
            "missing contact_channel"
        );
        assert!(v.get("contact_handle").is_some(), "missing contact_handle");
    }
}
