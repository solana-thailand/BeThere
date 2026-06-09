-- Phase 3e: Deposit statuses table.
-- Replaces KV key: "event:{event_id}:deposit:status:{attendee_id}" → DepositStatus JSON.

CREATE TABLE IF NOT EXISTS deposit_statuses (
    attendee_id     TEXT NOT NULL,
    event_id        TEXT NOT NULL,
    method          TEXT NOT NULL,   -- DepositMethod enum: usdc, thb, credit_thb, credit_usdc
    amount          INTEGER NOT NULL,
    currency        TEXT NOT NULL,
    tx_signature    TEXT,
    verified        INTEGER NOT NULL DEFAULT 0,
    deposited_at    TEXT NOT NULL,
    wallet_address  TEXT,
    deposit_order   INTEGER NOT NULL DEFAULT 0,
    refundable      INTEGER NOT NULL DEFAULT 1,
    rejected        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (event_id, attendee_id)
);

CREATE INDEX IF NOT EXISTS idx_deposit_statuses_event    ON deposit_statuses(event_id);
CREATE INDEX IF NOT EXISTS idx_deposit_statuses_attendee ON deposit_statuses(event_id, attendee_id);
