-- Durable journal for event NFT claims. The external mint result is recorded
-- before the attendee projection so retries can repair D1 without minting a
-- second badge.
CREATE TABLE IF NOT EXISTS nft_mint_jobs (
    event_id        TEXT NOT NULL,
    claim_token     TEXT NOT NULL,
    wallet          TEXT NOT NULL,
    provider_mint_id TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('pending','confirmed','persisted')),
    asset_id        TEXT,
    signature       TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (event_id, claim_token)
);

CREATE INDEX IF NOT EXISTS idx_nft_mint_jobs_status_updated
    ON nft_mint_jobs(status, updated_at);
