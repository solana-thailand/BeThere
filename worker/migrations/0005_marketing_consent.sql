-- Issue: Marketing consent tracking for PDPA compliance.
--
-- Adds opt-in/opt-out tracking columns to attendees so that marketing
-- consent can be recorded per-row (updated across all rows for a given
-- email when the user unsubscribes).

ALTER TABLE attendees ADD COLUMN consent_marketing    BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE attendees ADD COLUMN consent_marketing_at  TEXT;
