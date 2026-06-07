//! Schema initialization for the EventDurableObject SQLite storage.

/// Create SQLite tables inside the DO if they don't exist.
/// Called once per DO instance cold start.
pub(super) fn init_schema(sql: &worker::SqlStorage) {
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
