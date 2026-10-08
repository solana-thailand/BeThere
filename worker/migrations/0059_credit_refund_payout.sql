-- 0059_credit_refund_payout.sql — .issues/190 (owner option (a), 2026-10-08).
--
-- Where an attendee wants their held rolling credit paid back to, captured
-- with the "request credit refund" action and shown to the organizer in the
-- payout queue. One row per contact email (the request is on the contact, like
-- `contacts.credit_refund_requested`); a re-request replaces it.
--
-- A table of its own rather than columns on `contacts`:
--   * the row IS the personal data — deleting it at payout, in the nightly
--     purge, and on PDPA erasure is one DELETE, with nothing left behind in a
--     table that has six other writers (`upsert_contact`, `clear_contact_pii`,
--     the Sheets sync, the flag set/clear);
--   * the CHECKs below only bind this table's own writer
--     (`db::credit_refund_accounts::save`). On `contacts` they would bind every
--     existing writer too (memory: migration-constraints-break-existing-writers);
--   * no `event_id` column, so `event_purge` (which must cover every table with
--     one — worker/tests/security/test_event_purge.py) is unaffected.
--
-- Account numbers are never logged; the admin queue is the only reader.
--
-- Apply BEFORE the code deploy that writes it, then read the schema back:
--   PRAGMA table_info(credit_refund_accounts);
CREATE TABLE IF NOT EXISTS credit_refund_accounts (
    email        TEXT PRIMARY KEY,               -- lowercased contacts.email
    method       TEXT NOT NULL CHECK (method IN ('promptpay', 'bank')),
    promptpay_id TEXT,                           -- digits only
    bank_name    TEXT,
    bank_account TEXT,
    account_name TEXT,
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
    CHECK (
        (method = 'promptpay' AND promptpay_id IS NOT NULL
            AND bank_name IS NULL AND bank_account IS NULL AND account_name IS NULL)
        OR
        (method = 'bank' AND promptpay_id IS NULL
            AND bank_name IS NOT NULL AND bank_account IS NOT NULL
            AND account_name IS NOT NULL)
    )
);

-- The nightly purge deletes by age.
CREATE INDEX IF NOT EXISTS idx_credit_refund_accounts_updated
    ON credit_refund_accounts (updated_at);
