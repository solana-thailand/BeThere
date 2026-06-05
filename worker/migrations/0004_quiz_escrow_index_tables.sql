-- Issue 046 P4: Quiz configs, quiz progress, and escrow index tables.
--
-- Moves quiz and escrow-index data from KV to D1 so the EVENTS KV binding
-- can be fully removed.  All columns have defaults so the migration is
-- idempotent and safe to re-run.

-- ---------------------------------------------------------------------------
-- Quiz config — one row per event (replaces KV key "event:{id}:quiz:questions")
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS quiz_configs (
    event_id        TEXT NOT NULL PRIMARY KEY,
    config_json     TEXT NOT NULL DEFAULT '{}',
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ---------------------------------------------------------------------------
-- Quiz progress — one row per attendee per event
-- (replaces KV key "event:{id}:quiz:progress:{token}")
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS quiz_progress (
    event_id        TEXT NOT NULL,
    claim_token     TEXT NOT NULL,
    progress_json   TEXT NOT NULL DEFAULT '{}',
    passed          INTEGER NOT NULL DEFAULT 0,
    attempts        INTEGER NOT NULL DEFAULT 0,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (event_id, claim_token)
);

-- Quick lookup: "has this attendee passed the quiz?"
CREATE INDEX IF NOT EXISTS idx_quiz_progress_passed
    ON quiz_progress (event_id, passed);

-- ---------------------------------------------------------------------------
-- Escrow index — reverse lookup from escrow address to event ID
-- (replaces KV key "escrow:{address}")
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS escrow_index (
    escrow_address  TEXT NOT NULL PRIMARY KEY,
    event_id        TEXT NOT NULL,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
