-- 0055_claim_locks_one_wallet_per_event.sql — `.plans/025` §5.3 / 7.8.
--
-- One badge per recipient wallet per event. `claim_locks` is where the
-- recipient is recorded: both claim paths insert the row before minting and
-- nothing prunes finalized rows (0029), so an index here covers every claim
-- made since 0001. A failed mint deletes its lock row, so a retry to the same
-- wallet is still allowed.
--
-- Exact string, not LOWER(wallet): base58 is case-sensitive, and every writer
-- passes an address through `solana::validate_wallet_address`, whose output
-- is already canonical (plan 025 §6.2 (e)).
--
-- Prod had 0 duplicate (event_id, wallet) pairs on 2026-09-27 (4 rows), so the
-- index builds without cleanup. The only writer that could trip on it is
-- `db::claim_locks::acquire_claim_lock`, which now uses a targetless
-- ON CONFLICT DO NOTHING and reports which constraint held
-- (memory: migration-constraints-break-existing-writers).
CREATE UNIQUE INDEX IF NOT EXISTS idx_claim_locks_event_wallet
    ON claim_locks(event_id, wallet);
