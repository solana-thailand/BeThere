-- Add community_links JSON column to events table.
-- Stores community/social links (discord, telegram, x, etc.) as a JSON array.

ALTER TABLE events ADD COLUMN community_links TEXT NOT NULL DEFAULT '[]';
