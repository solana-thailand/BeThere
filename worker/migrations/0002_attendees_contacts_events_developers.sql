-- Issue 046 Phase 2a + Issue 049: D1 Phase 2 — Attendees, Contacts, Events, Staff, Developer Profiles
-- Idempotent: uses IF NOT EXISTS for safe re-runs.

-- ============================================================
-- ATTENDEES
-- Replaces: Attendees Google Sheet (per-event)
-- ============================================================

CREATE TABLE IF NOT EXISTS attendees (
    id                  TEXT PRIMARY KEY,              -- api_id (UUID from Sheets row)
    event_id            TEXT NOT NULL,                 -- FK → events.id
    email               TEXT NOT NULL,
    name                TEXT NOT NULL DEFAULT '',
    approval_status     TEXT NOT NULL DEFAULT 'approved',
    participation_type  TEXT NOT NULL DEFAULT 'in_person',
    checked_in_at       TEXT,
    checked_in_by       TEXT,
    claim_token         TEXT UNIQUE,
    claimed_at          TEXT,
    claim_asset_id      TEXT,
    claim_signature     TEXT,
    qr_url              TEXT,
    contact_channel     TEXT NOT NULL DEFAULT '',
    contact_handle      TEXT NOT NULL DEFAULT '',
    deposit_status      TEXT NOT NULL DEFAULT 'none',
    deposit_amount_usdc INTEGER NOT NULL DEFAULT 0,
    deposit_amount_thb  INTEGER NOT NULL DEFAULT 0,
    deposit_tx_hash     TEXT,
    deposit_slip_r2_key TEXT,
    deposit_verified_at TEXT,
    deposit_verified_by TEXT,
    refund_tx_hash      TEXT,
    refund_marked_at    TEXT,
    refund_marked_by    TEXT,
    refund_link         TEXT,
    bank_name           TEXT,
    bank_account_number TEXT,
    bank_account_name   TEXT,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at          TEXT NOT NULL DEFAULT (datetime('now')),
    sheet_row_index     INTEGER,
    synced_at           TEXT
);

CREATE INDEX IF NOT EXISTS idx_attendees_event       ON attendees(event_id);
CREATE INDEX IF NOT EXISTS idx_attendees_email       ON attendees(email);
CREATE INDEX IF NOT EXISTS idx_attendees_claim_token ON attendees(claim_token) WHERE claim_token IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_attendees_approval    ON attendees(event_id, approval_status);
CREATE INDEX IF NOT EXISTS idx_attendees_deposit     ON attendees(event_id, deposit_status) WHERE deposit_status != 'none';

-- ============================================================
-- CONTACTS
-- Replaces: Contacts Google Sheet (master, per-org)
-- ============================================================

CREATE TABLE IF NOT EXISTS contacts (
    email                TEXT PRIMARY KEY,             -- lowercased email
    name                 TEXT NOT NULL DEFAULT '',
    first_registered     TEXT NOT NULL DEFAULT (datetime('now')),
    last_registered      TEXT NOT NULL DEFAULT (datetime('now')),
    events_joined        TEXT NOT NULL DEFAULT '',     -- comma-separated event IDs
    event_count          INTEGER NOT NULL DEFAULT 0,
    contact_channel      TEXT NOT NULL DEFAULT '',
    contact_handle       TEXT NOT NULL DEFAULT '',
    deposit_credit_thb   INTEGER NOT NULL DEFAULT 0,
    deposit_credit_usdc  INTEGER NOT NULL DEFAULT 0,
    deposit_credit_since TEXT,
    synced_at            TEXT
);

CREATE INDEX IF NOT EXISTS idx_contacts_events ON contacts(events_joined);

-- ============================================================
-- EVENTS
-- Replaces: KV event:{id} + Events tab in Google Sheets
-- ============================================================

CREATE TABLE IF NOT EXISTS events (
    id                   TEXT PRIMARY KEY,             -- slug-based
    name                 TEXT NOT NULL,
    slug                 TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'draft',
    event_format         TEXT NOT NULL DEFAULT 'in_person',
    event_start_ms       INTEGER NOT NULL,
    event_end_ms         INTEGER NOT NULL,
    deposit_enabled      INTEGER NOT NULL DEFAULT 0,
    deposit_amount_usdc  INTEGER NOT NULL DEFAULT 0,
    deposit_amount_thb   INTEGER NOT NULL DEFAULT 0,
    escrow_status        TEXT NOT NULL DEFAULT 'none',
    escrow_pda           TEXT,
    location             TEXT NOT NULL DEFAULT '',
    tagline              TEXT NOT NULL DEFAULT '',
    organizer_emails     TEXT NOT NULL DEFAULT '',
    organization_id      TEXT NOT NULL DEFAULT '',
    video_url            TEXT NOT NULL DEFAULT '',
    sheet_id             TEXT NOT NULL DEFAULT '',
    sheet_name           TEXT NOT NULL DEFAULT 'Attendees',
    staff_sheet_name     TEXT NOT NULL DEFAULT 'staff',
    capacity             INTEGER NOT NULL DEFAULT 0,
    total_attendees      INTEGER NOT NULL DEFAULT 0,
    created_at           TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at           TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_events_status ON events(status);
CREATE INDEX IF NOT EXISTS idx_events_org   ON events(organization_id);
CREATE INDEX IF NOT EXISTS idx_events_slug  ON events(slug);

-- ============================================================
-- STAFF
-- Replaces: Staff sheet in Google Sheets
-- ============================================================

CREATE TABLE IF NOT EXISTS staff (
    email    TEXT NOT NULL,
    event_id TEXT NOT NULL,
    role     TEXT NOT NULL DEFAULT 'staff',
    name     TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (email, event_id)
);

CREATE INDEX IF NOT EXISTS idx_staff_event ON staff(event_id);

-- ============================================================
-- DEVELOPER PROFILES (Issue 049)
-- Rich developer profile built incrementally across events.
-- Upserted when registration responses have profile_field = true.
-- ============================================================

CREATE TABLE IF NOT EXISTS developer_profiles (
    email              TEXT PRIMARY KEY,               -- lowercased, from OAuth
    display_name       TEXT NOT NULL DEFAULT '',
    wallet_address     TEXT,                            -- Solana wallet
    github_handle      TEXT,
    discord_handle     TEXT,
    twitter_handle     TEXT,
    experience_level   TEXT,                            -- beginner/mid/senior/lead
    primary_role       TEXT,                            -- dev/designer/pm/founder/student
    tech_stack         TEXT NOT NULL DEFAULT '[]',      -- JSON array: ["Rust","TypeScript"]
    interests          TEXT NOT NULL DEFAULT '[]',      -- JSON array: ["DeFi","ZK","Gaming"]
    learning_goals     TEXT NOT NULL DEFAULT '',
    expectations       TEXT NOT NULL DEFAULT '',         -- per-event, latest wins
    company_org        TEXT NOT NULL DEFAULT '',
    location_city      TEXT NOT NULL DEFAULT '',
    consent_outreach   INTEGER NOT NULL DEFAULT 0,      -- PDPA: can we contact?
    first_seen_at      TEXT NOT NULL DEFAULT (datetime('now')),
    last_active_at     TEXT NOT NULL DEFAULT (datetime('now')),
    total_events       INTEGER NOT NULL DEFAULT 0,
    badges_earned      TEXT NOT NULL DEFAULT '[]',      -- JSON array of badge IDs
    created_at         TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at         TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_dev_profiles_experience ON developer_profiles(experience_level);
CREATE INDEX IF NOT EXISTS idx_dev_profiles_role       ON developer_profiles(primary_role);
CREATE INDEX IF NOT EXISTS idx_dev_profiles_location   ON developer_profiles(location_city);
CREATE INDEX IF NOT EXISTS idx_dev_profiles_consent    ON developer_profiles(consent_outreach) WHERE consent_outreach = 1;

-- ============================================================
-- REGISTRATION RESPONSES (Issue 049)
-- Per-event configurable form responses.
-- Raw answers stored here; profile_field = true also upserts developer_profiles.
-- ============================================================

CREATE TABLE IF NOT EXISTS registration_responses (
    id                TEXT PRIMARY KEY,                 -- UUID v7
    event_id          TEXT NOT NULL,
    developer_email   TEXT NOT NULL,
    field_key         TEXT NOT NULL,
    field_value       TEXT NOT NULL DEFAULT '',
    is_profile_field  INTEGER NOT NULL DEFAULT 0,
    answered_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_reg_responses_event   ON registration_responses(event_id);
CREATE INDEX IF NOT EXISTS idx_reg_responses_dev     ON registration_responses(developer_email);
CREATE INDEX IF NOT EXISTS idx_reg_responses_event_dev ON registration_responses(event_id, developer_email);
