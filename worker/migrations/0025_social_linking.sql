-- Migration 0025: Social account linking fields for developer profiles
-- Adds verified social handles: Telegram (new), and verified flags for existing handles.

-- Telegram (new field — not in original schema)
ALTER TABLE developer_profiles ADD COLUMN telegram_handle TEXT;
ALTER TABLE developer_profiles ADD COLUMN telegram_id TEXT;         -- Telegram numeric user ID for deep links

-- Verified flags for all social handles (auto-verified via OAuth/widget)
ALTER TABLE developer_profiles ADD COLUMN github_verified INTEGER NOT NULL DEFAULT 0;
ALTER TABLE developer_profiles ADD COLUMN telegram_verified INTEGER NOT NULL DEFAULT 0;
ALTER TABLE developer_profiles ADD COLUMN discord_verified INTEGER NOT NULL DEFAULT 0;

-- Timestamps for when each was last verified
ALTER TABLE developer_profiles ADD COLUMN github_verified_at TEXT;
ALTER TABLE developer_profiles ADD COLUMN telegram_verified_at TEXT;
ALTER TABLE developer_profiles ADD COLUMN discord_verified_at TEXT;
