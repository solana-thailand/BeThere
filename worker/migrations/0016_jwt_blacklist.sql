-- Issue 053 Phase 3g: JWT blacklist table.
-- Replaces KV key "jwt_blacklist:{sha256(token)}" which used TTL for auto-expiry.
-- D1 has no TTL, so expired entries are pruned by the scheduled cleanup handler.

CREATE TABLE IF NOT EXISTS jwt_blacklist (
    token_hash   TEXT PRIMARY KEY,
    expires_at   INTEGER NOT NULL,  -- Unix timestamp (seconds)
    created_at   INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_jwt_blacklist_expires ON jwt_blacklist(expires_at);
