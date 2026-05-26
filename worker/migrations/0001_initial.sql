-- Issue 037: D1 Phase 1 — Claim Locks + Audit Trail
-- Idempotent: uses IF NOT EXISTS for safe re-runs.

-- Claim dedup locks — replaces KV key "event:{id}:claim_lock:{token}"
CREATE TABLE IF NOT EXISTS claim_locks (
    event_id   TEXT NOT NULL,
    token      TEXT NOT NULL,
    lock_id    TEXT NOT NULL,
    wallet     TEXT NOT NULL,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    -- Finalized fields (NULL until mint completes)
    asset_id   TEXT,
    signature  TEXT,
    claimed_at TEXT,
    expires_at TEXT NOT NULL,
    PRIMARY KEY (event_id, token)
);

CREATE INDEX IF NOT EXISTS idx_claim_locks_expires ON claim_locks(expires_at);

-- Append-only audit log — replaces KV JSON arrays
CREATE TABLE IF NOT EXISTS audit_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id    TEXT NOT NULL,
    timestamp   TEXT NOT NULL DEFAULT (datetime('now')),
    actor       TEXT NOT NULL,
    action      TEXT NOT NULL,
    target      TEXT NOT NULL,
    description TEXT NOT NULL,
    metadata    TEXT  -- JSON string
);

CREATE INDEX IF NOT EXISTS idx_audit_event_time ON audit_log(event_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_action     ON audit_log(action);
