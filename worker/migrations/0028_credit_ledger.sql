-- Credit ledger: append-only, ORG-SCOPED rolling-deposit-credit movements.
--
-- Replaces the mutable `deposit_credit_thb` cell in the Google "Contacts" sheet,
-- which (a) lost credit on non-atomic best-effort writes and (b) shadowed it
-- behind duplicate contact rows read first-match by find_contact_row
-- (credit incident 2026-08-14: 6 balances lost, 4 shadowed).
--
-- Balance = SUM(delta) over (email, organization_id, currency).
--   +delta  grant  (hold, refund-in, backfill)
--   -delta  spend  (apply to a new event's deposit)
-- Append-only => auditable + race-free. Org-scoped => Org A's credit can never
-- be spent at Org B (Issue #029 multi-org isolation). organization_id '' is the
-- current single/default org (events carry an empty organization_id today).
CREATE TABLE IF NOT EXISTS credit_ledger (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    email           TEXT    NOT NULL,               -- lowercased
    organization_id TEXT    NOT NULL DEFAULT '',    -- '' = default/single org
    currency        TEXT    NOT NULL,               -- 'thb' | 'usdc'
    delta           INTEGER NOT NULL,               -- signed, whole THB / whole USDC
    reason          TEXT    NOT NULL,               -- hold | apply | refund | backfill | adjust
    event_id        TEXT,                            -- source (hold) / target (apply) event
    deposit_id      TEXT,                            -- links thb_deposits.id (idempotency key)
    note            TEXT,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);

-- Cheap balance aggregation.
CREATE INDEX IF NOT EXISTS idx_credit_ledger_bal
    ON credit_ledger (email, organization_id, currency);

-- Idempotency: at most ONE entry per (deposit_id, reason). A re-fired hold or
-- apply for the same deposit is a no-op (ON CONFLICT DO NOTHING) => credit can
-- never double-move. Manual adjusts use NULL deposit_id and are exempt (SQLite
-- treats NULLs as distinct in unique indexes).
CREATE UNIQUE INDEX IF NOT EXISTS idx_credit_ledger_once
    ON credit_ledger (deposit_id, reason)
    WHERE deposit_id IS NOT NULL;
