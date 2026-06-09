-- Phase 3b: On-chain events, dedup, and cursors (Issue #053)
-- Replaces KV keys:
--   "event:{id}:onchain" → Vec<OnChainEvent> JSON
--   "onchain:sig:{signature}" → "1" (TTL dedup)
--   "onchain:cursor:{escrow_addr}" → last signature string

CREATE TABLE onchain_events (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id              TEXT NOT NULL,
    signature             TEXT NOT NULL UNIQUE,
    slot                  INTEGER NOT NULL,
    block_time            INTEGER NOT NULL,
    instruction           TEXT NOT NULL,
    escrow_address        TEXT NOT NULL,
    target_escrow_address TEXT,
    organizer             TEXT,
    attendee              TEXT,
    amount                INTEGER,
    indexed_at            TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_onchain_events_event     ON onchain_events(event_id, block_time DESC);
CREATE INDEX idx_onchain_events_escrow    ON onchain_events(escrow_address);
CREATE INDEX idx_onchain_events_organizer ON onchain_events(organizer);
CREATE INDEX idx_onchain_events_attendee  ON onchain_events(attendee);

CREATE TABLE onchain_dedup (
    signature   TEXT PRIMARY KEY,
    indexed_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE onchain_cursors (
    escrow_address     TEXT PRIMARY KEY,
    last_signature     TEXT NOT NULL,
    updated_at         TEXT NOT NULL DEFAULT (datetime('now'))
);
