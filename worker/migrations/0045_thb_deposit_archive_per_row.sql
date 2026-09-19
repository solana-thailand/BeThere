-- 0045 — Key the deposit archive on the deposit ROW, not on (event, attendee).
--
-- 0044 shipped with `UNIQUE (event_id, attendee_id)`. `thb_deposits` has no such
-- constraint — `0013` creates `idx_thb_deposits_attendee` as a plain INDEX — and
-- `event_store::write::deposit::save_thb_deposit` is a read-then-insert-or-update
-- on that same pair, so two concurrent uploads for one attendee can both take
-- the insert branch. Two rows, one archived, both deleted:
--
--     deposits before : 2 rows, 1200 THB
--     archived        : 1 row,   500 THB
--     deposits after  : 0 rows          <- 700 THB gone, and the run logged success
--
-- Reproduced against these migrations before this file was written. The old
-- `COUNT(*)`-based return made it invisible: it counted every archived row for
-- the event, not the rows the run actually inserted, so the summary looked right.
--
-- The archive exists to preserve the money, and the money is per deposit row, so
-- that is what idempotency has to key on. `thb_deposits.id` is INTEGER PRIMARY
-- KEY AUTOINCREMENT, so it is unique for the life of the database and is not
-- reused after a delete — re-archiving the same id really is the same deposit.
--
-- Additive on purpose. Both archives were empty when this was written (prod and
-- staging, checked), but a rebuild is not needed either way: pre-existing rows
-- simply carry `source_deposit_id IS NULL`, and SQLite treats NULLs as distinct
-- in a UNIQUE index, so they neither conflict nor block. The index is a plain
-- UNIQUE, not partial, so `ON CONFLICT (source_deposit_id)` needs no repeated
-- WHERE predicate (the trap `credit_ledger` hits with its partial index).
--
-- Root cause is NOT fixed here: `thb_deposits` still allows the duplicate pair,
-- and the save path is still check-then-act. See `.issues/127` — putting a UNIQUE
-- index on a live money table and turning the save into an upsert is a change to
-- the deposit write path, not something to slip in beside an archive fix.

ALTER TABLE thb_deposit_archive ADD COLUMN source_deposit_id INTEGER;

-- The pair is no longer an identity, so it must no longer be a constraint.
DROP INDEX IF EXISTS idx_thb_deposit_archive_unique;

CREATE UNIQUE INDEX IF NOT EXISTS idx_thb_deposit_archive_source
    ON thb_deposit_archive(source_deposit_id);

-- Still the shape every read uses ("what happened to this person's deposit").
CREATE INDEX IF NOT EXISTS idx_thb_deposit_archive_attendee
    ON thb_deposit_archive(event_id, attendee_id);
