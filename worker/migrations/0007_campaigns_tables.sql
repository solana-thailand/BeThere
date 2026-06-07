-- Issue 049 Phase 3: Campaigns & Series
-- Idempotent: uses IF NOT EXISTS for safe re-runs.

-- ============================================================
-- CAMPAIGNS
-- Multi-event series with completion criteria and rewards.
-- ============================================================

CREATE TABLE IF NOT EXISTS campaigns (
    id                    TEXT PRIMARY KEY,               -- slug
    title                 TEXT NOT NULL,
    description           TEXT NOT NULL DEFAULT '',
    organization_id       TEXT NOT NULL DEFAULT '',
    status                TEXT NOT NULL DEFAULT 'draft',  -- draft/active/completed
    completion_criteria   TEXT NOT NULL DEFAULT '{}',     -- JSON config
    reward_type           TEXT NOT NULL DEFAULT 'none',   -- none/nft_certificate/badge
    reward_config         TEXT NOT NULL DEFAULT '{}',     -- JSON metadata for NFT/badge
    created_at            TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at            TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_campaigns_status ON campaigns(status);
CREATE INDEX IF NOT EXISTS idx_campaigns_org    ON campaigns(organization_id);

-- ============================================================
-- CAMPAIGN EVENTS
-- Links events to campaigns with ordering and required flag.
-- ============================================================

CREATE TABLE IF NOT EXISTS campaign_events (
    campaign_id    TEXT NOT NULL,
    event_id       TEXT NOT NULL,
    sequence_order INTEGER NOT NULL DEFAULT 0,
    is_required    INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (campaign_id, event_id)
);

CREATE INDEX IF NOT EXISTS idx_campaign_events_event ON campaign_events(event_id);

-- ============================================================
-- DEVELOPER CAMPAIGN PROGRESS
-- Tracks per-developer progress across campaign events.
-- ============================================================

CREATE TABLE IF NOT EXISTS developer_campaign_progress (
    campaign_id       TEXT NOT NULL,
    developer_email   TEXT NOT NULL,
    events_completed  INTEGER NOT NULL DEFAULT 0,
    total_required    INTEGER NOT NULL DEFAULT 0,
    is_complete       INTEGER NOT NULL DEFAULT 0,
    completed_at      TEXT,
    reward_claimed_at TEXT,
    PRIMARY KEY (campaign_id, developer_email)
);

CREATE INDEX IF NOT EXISTS idx_dev_campaign_progress_dev      ON developer_campaign_progress(developer_email);
CREATE INDEX IF NOT EXISTS idx_dev_campaign_progress_complete ON developer_campaign_progress(is_complete);
