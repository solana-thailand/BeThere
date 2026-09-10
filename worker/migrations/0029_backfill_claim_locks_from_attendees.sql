-- Backfill the finalized columns of `claim_locks` from `attendees`.
--
-- `claim_locks` rows are INSERTed at lock acquisition with `asset_id`,
-- `signature` and `claimed_at` NULL, and filled in by `finalize_claim_lock`
-- once the mint succeeds. Rows predating reliable finalization still carry the
-- three NULLs even though the mint completed, so the table's audit copy of the
-- claim is missing for those events.
--
-- Earlier notes on this backfill assumed it would have to join against Solana to
-- recover the mint signature. It does not: `attendees` already carries
-- `claimed_at`, `claim_asset_id` and `claim_signature`, written by
-- `db::attendees::writes::claim_attendee` in the same claim flow. The backfill
-- is therefore a pure intra-database join and needs no RPC, no key material and
-- no network access.
--
-- Join key: `attendees.claim_token`. That is the key `claim_attendee` itself
-- matches on, and the column is UNIQUE, so it identifies exactly one attendee.
--
-- Scope guards:
--   * only rows where all three target columns are still NULL — an already
--     finalized row is never touched, so this is safe to re-run;
--   * only from attendees where all three source columns are NON-NULL — a
--     half-written attendee row cannot produce a half-written lock row.
--
-- `expires_at` is NOT NULL and moves to the 90-day retention horizon on
-- finalization (`claim::finalized_expires_at`). We reproduce that relative to
-- the *original* claim rather than to migration time, so a backfilled row is
-- indistinguishable from one finalized normally. `strftime` returns NULL on an
-- unparseable timestamp, which against a NOT NULL column would abort the whole
-- statement, so it is wrapped in COALESCE onto the existing value.
--
-- Read impact today: none. `db::claim_locks::get_claim_lock` is the only reader
-- and is `#[allow(dead_code)]` with no call sites; nothing prunes the table on
-- `expires_at` either. This repairs the audit trail, it does not change
-- behaviour.

UPDATE claim_locks
SET asset_id = (
        SELECT a.claim_asset_id FROM attendees a
        WHERE a.claim_token = claim_locks.token
    ),
    signature = (
        SELECT a.claim_signature FROM attendees a
        WHERE a.claim_token = claim_locks.token
    ),
    claimed_at = (
        SELECT a.claimed_at FROM attendees a
        WHERE a.claim_token = claim_locks.token
    ),
    expires_at = COALESCE(
        (
            SELECT strftime('%Y-%m-%dT%H:%M:%SZ', a.claimed_at, '+90 days')
            FROM attendees a
            WHERE a.claim_token = claim_locks.token
        ),
        claim_locks.expires_at
    )
WHERE claim_locks.asset_id IS NULL
  AND claim_locks.signature IS NULL
  AND claim_locks.claimed_at IS NULL
  AND EXISTS (
        SELECT 1 FROM attendees a
        WHERE a.claim_token = claim_locks.token
          AND a.claimed_at IS NOT NULL
          AND a.claim_asset_id IS NOT NULL
          AND a.claim_signature IS NOT NULL
    );
