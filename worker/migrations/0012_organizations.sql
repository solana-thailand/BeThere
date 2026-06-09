-- Phase 3c: Organizations table.
-- Replaces KV keys: "org:{org_id}" → OrganizationConfig, "orgs" → OrgIndex.

CREATE TABLE IF NOT EXISTS organizations (
    id                  TEXT PRIMARY KEY,        -- slug-style identifier (e.g. "solana-thailand")
    name                TEXT NOT NULL,            -- display name
    contacts_sheet_id   TEXT NOT NULL DEFAULT '', -- Google Sheet ID (empty = global fallback)
    contacts_sheet_name TEXT NOT NULL DEFAULT 'Contacts',
    events_sheet_name   TEXT NOT NULL DEFAULT 'Events',
    owner_emails        TEXT NOT NULL DEFAULT '[]', -- JSON array of email strings
    created_at          TEXT NOT NULL,            -- ISO 8601
    updated_at          TEXT NOT NULL DEFAULT ''  -- ISO 8601
);

CREATE INDEX IF NOT EXISTS idx_organizations_name ON organizations(name);
