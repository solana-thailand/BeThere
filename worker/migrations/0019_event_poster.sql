-- 0019_event_poster.sql
-- Per-event marketing poster URL (served path, e.g. /api/storage/posters/{event_id}).
-- Mirrors the nft_image_url column added by 0003. Empty string = no poster →
-- event page hero falls back to the NFT badge image.
--
-- NOTE: This migration is NOT idempotent — ALTER TABLE ADD COLUMN fails
-- if the column already exists. It relies on the d1_migrations tracker
-- to prevent re-execution.
ALTER TABLE events ADD COLUMN poster_url TEXT NOT NULL DEFAULT '';
