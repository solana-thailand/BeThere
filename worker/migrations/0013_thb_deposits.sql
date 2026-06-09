-- Phase 3d: THB deposits table.
-- Replaces KV keys: "event:{event_id}:deposit:thb:{attendee_id}", "event:{event_id}:deposit:thb:list".

CREATE TABLE IF NOT EXISTS thb_deposits (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    attendee_id     TEXT NOT NULL,
    event_id        TEXT NOT NULL,
    amount_thb      INTEGER NOT NULL,
    slip_url        TEXT,
    verified        INTEGER NOT NULL DEFAULT 0,
    verified_by     TEXT,
    verified_at     TEXT,
    uploaded_at     TEXT NOT NULL,
    refunded        INTEGER NOT NULL DEFAULT 0,
    refunded_at     TEXT,
    attendee_name   TEXT,
    bank_account    TEXT,
    bank_name       TEXT,
    account_name    TEXT,
    refund_proof_url TEXT
);

CREATE INDEX IF NOT EXISTS idx_thb_deposits_event    ON thb_deposits(event_id);
CREATE INDEX IF NOT EXISTS idx_thb_deposits_attendee ON thb_deposits(event_id, attendee_id);
