-- Issue 046 Phase 2d: Expand events table to full EventConfig schema.
-- Adds columns missing from the original 0002 migration so that
-- EventConfig can be fully round-tripped through D1.
--
-- All new columns have defaults so existing rows (if any) are unaffected.
--
-- NOTE: This migration is NOT idempotent — ALTER TABLE ADD COLUMN fails
-- if the column already exists. It relies on the d1_migrations tracker
-- to prevent re-execution. For databases with manually-created columns,
-- insert a record into d1_migrations before running:
--   INSERT INTO d1_migrations (id, name) VALUES (3, '0003_events_full_schema.sql');

ALTER TABLE events ADD COLUMN link TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN time_tba INTEGER NOT NULL DEFAULT 0;
ALTER TABLE events ADD COLUMN quiz_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE events ADD COLUMN nft_collection_mint TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN nft_metadata_uri TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN nft_image_url TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN nft_name_template TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN nft_symbol TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN nft_description_template TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN merkle_tree TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN staff_emails TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN claim_base_url TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN promptpay_id TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN escrow_address TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN organizer_wallet TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN on_chain_event_id INTEGER NOT NULL DEFAULT 0;
ALTER TABLE events ADD COLUMN refund_deadline_hours INTEGER NOT NULL DEFAULT 168;
ALTER TABLE events ADD COLUMN max_refundable_deposits INTEGER NOT NULL DEFAULT 0;
ALTER TABLE events ADD COLUMN description TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN visibility TEXT NOT NULL DEFAULT 'public';
ALTER TABLE events ADD COLUMN require_contact_info INTEGER NOT NULL DEFAULT 1;
ALTER TABLE events ADD COLUMN require_photo_consent INTEGER NOT NULL DEFAULT 0;
ALTER TABLE events ADD COLUMN in_person_capacity INTEGER;
ALTER TABLE events ADD COLUMN online_capacity INTEGER;
ALTER TABLE events ADD COLUMN online_open_mode TEXT NOT NULL DEFAULT 'auto';
ALTER TABLE events ADD COLUMN online_registration_open INTEGER NOT NULL DEFAULT 0;
ALTER TABLE events ADD COLUMN deposit_deadline_hours INTEGER;
ALTER TABLE events ADD COLUMN updated_by TEXT NOT NULL DEFAULT '';
