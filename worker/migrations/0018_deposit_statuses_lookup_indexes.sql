-- Phase: Deposit double-registration defence (plan 003).
--
-- Adds lookup indexes used by:
--   - `db::deposit_statuses::find_attendee_by_wallet`     (Guard 1: deposit initiation dedup)
--   - `db::deposit_statuses::find_attendee_by_tx_signature` (Guard 2: read-path recovery binding check)
--
-- Both queries scope by (event_id, <column>); a composite index turns them from
-- within-event table scans into index range scans. Idempotent — safe to apply
-- on already-deployed DBs.

CREATE INDEX IF NOT EXISTS idx_deposit_statuses_wallet
    ON deposit_statuses(event_id, wallet_address);

CREATE INDEX IF NOT EXISTS idx_deposit_statuses_tx
    ON deposit_statuses(event_id, tx_signature);
