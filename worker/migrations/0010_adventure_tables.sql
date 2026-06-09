-- Issue 053 Phase 3a: Adventure config + progress tables.
--
-- Moves adventure data from KV to D1 so progress is durable and queryable.
-- Replaces KV keys:
--   event:{id}:adventure:config
--   event:{id}:adventure:progress:{claim_token}
-- All columns have defaults so the migration is idempotent and safe to re-run.

-- ---------------------------------------------------------------------------
-- Adventure config — one row per event
-- (replaces KV key "event:{id}:adventure:config")
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS adventure_configs (
    event_id        TEXT NOT NULL PRIMARY KEY,
    config_json     TEXT NOT NULL DEFAULT '{}',
    enabled         INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_adventure_configs_enabled
    ON adventure_configs (enabled);

-- ---------------------------------------------------------------------------
-- Adventure progress — one row per attendee per event
-- (replaces KV key "event:{id}:adventure:progress:{claim_token}")
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS adventure_progress (
    event_id                TEXT NOT NULL,
    claim_token             TEXT NOT NULL,
    progress_json           TEXT NOT NULL DEFAULT '{}',
    passed                  INTEGER NOT NULL DEFAULT 0,
    total_moves             INTEGER NOT NULL DEFAULT 0,
    total_time_seconds      INTEGER NOT NULL DEFAULT 0,
    levels_completed_count  INTEGER NOT NULL DEFAULT 0,
    last_played_at          TEXT,
    created_at              TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at              TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (event_id, claim_token)
);

CREATE INDEX IF NOT EXISTS idx_adventure_progress_event
    ON adventure_progress (event_id);

CREATE INDEX IF NOT EXISTS idx_adventure_progress_passed
    ON adventure_progress (event_id, passed);

CREATE INDEX IF NOT EXISTS idx_adventure_progress_last_played
    ON adventure_progress (event_id, last_played_at DESC);
