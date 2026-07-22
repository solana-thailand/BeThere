-- Issue #061 Phase 3: Exit path for held-as-credit deposits.
-- Adds `credit_refund_requested` to contacts so an attendee who previously
-- held their THB deposit as rolling credit can request its return. The
-- organizer processes the actual payout through the existing THB refund queue
-- tooling (manual / batch); this flag is the visibility/queue signal only.
--
-- Lives on `contacts` (not `thb_deposits`) because rolling credit is a
-- cross-event balance — a single contact may hold credit from multiple past
-- deposits across different events, and a refund-from-credit request is
-- against the rolling balance, not any specific source deposit. Mirrors the
-- `deposit_credit_thb/usdc/since` columns K–M already on this table.
--
-- Column conventions mirror `held_as_credit` / `held_as_credit_at` from
-- migration 0022 (which mirror `refunded` / `refunded_at` from 0013):
--   - boolean as INTEGER NOT NULL DEFAULT 0
--   - timestamp as nullable TEXT (ISO 8601)
--
-- Idempotent re-request semantics: an attendee clicking "Request Return"
-- again after a prior request re-stamps `credit_refund_requested_at` to the
-- latest click. This is intentional — it surfaces "still waiting" to the
-- organizer without an extra `cleared_at` column for v1. If/when an organizer
-- clears the flag (post-payout), it resets to 0 and a subsequent request
-- starts a fresh timestamp.
--
-- Applied once by the D1 migration tracker (wrangler d1 migrations apply).
-- SQLite has no ALTER TABLE ADD COLUMN IF NOT EXISTS; re-running manually will
-- error with "duplicate column name", which is the expected signal.

ALTER TABLE contacts ADD COLUMN credit_refund_requested INTEGER NOT NULL DEFAULT 0;
ALTER TABLE contacts ADD COLUMN credit_refund_requested_at TEXT;

-- Index the flag for the admin "refund requested" queue/listing. Partial index
-- (WHERE = 1) keeps it tiny — only rows with an open request are indexed, so
-- the index grows/shrinks with the live queue rather than the contact table.
CREATE INDEX IF NOT EXISTS idx_contacts_credit_refund_requested
    ON contacts(credit_refund_requested, credit_refund_requested_at)
    WHERE credit_refund_requested = 1;
