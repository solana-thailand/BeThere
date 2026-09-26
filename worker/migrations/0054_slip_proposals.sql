-- 0054_slip_proposals.sql — `.plans/033` W1, the slip agent in shadow mode.
--
-- One row per uploaded THB slip: what was read off it, where the reading came
-- from, what each deterministic check said, and the verdict the checker
-- proposes. Nothing reads this table to move money. The organizer's approve /
-- reject in `slip_verify.rs` is still the only decision; this row is shown next
-- to the slip so the two can be compared, and that comparison is the number the
-- plan publishes (false accepts must be 0).
--
-- A NEW TABLE, so no existing writer can trip on its constraints
-- (memory: migration-constraints-break-existing-writers).
--
-- claimed_ref vs bank_ref:
--   claimed_ref  every reference any slip carried, reused or not. Evidence.
--   bank_ref     set only when this row was the FIRST to claim the reference,
--                and UNIQUE. The uniqueness lives in the schema, not in code
--                (`.issues/127` is what a code-only uniqueness check cost).
--   A second slip with the same reference keeps claimed_ref, gets bank_ref
--   NULL, and fails the ref_new check. A single UNIQUE column holding both
--   would have to refuse the second insert and lose the evidence of the reuse.
--
-- The reference is a bank transaction id, not a person. No image, no sender
-- name and no account number are stored here; receiver_account is the masked
-- string printed on the slip (it is the event's own PromptPay account).
--
-- Rows are deleted with their deposit (`db::thb_deposits` delete paths), so
-- the 90-day retention of `cleanup.rs` covers them too.
CREATE TABLE IF NOT EXISTS slip_proposals (
    event_id         TEXT NOT NULL,
    attendee_id      TEXT NOT NULL,
    source           TEXT NOT NULL CHECK (source IN ('qr', 'vision')),
    -- The model id for 'vision'; NULL for 'qr', where no model ran.
    model            TEXT,
    claimed_ref      TEXT,
    bank_ref         TEXT UNIQUE,
    amount_satang    INTEGER CHECK (amount_satang IS NULL OR amount_satang >= 0),
    transferred_at   TEXT,
    receiver_account TEXT,
    -- JSON array of [check, outcome] pairs, `slip_proposal::Evaluation::checks`.
    checks           TEXT NOT NULL,
    verdict          TEXT NOT NULL CHECK (verdict IN ('accepted', 'needs_review', 'rejected')),
    created_at       TEXT NOT NULL,
    PRIMARY KEY (event_id, attendee_id)
);

-- "Has anyone else claimed this reference?" is asked on every upload.
CREATE INDEX IF NOT EXISTS idx_slip_proposals_claimed_ref
    ON slip_proposals(claimed_ref);
