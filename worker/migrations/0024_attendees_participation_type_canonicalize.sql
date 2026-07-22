-- Issue #059 Step 3.3: participation_type canonical backfill.
-- Issue #058 follow-up step 3 (same work, two issue references).
--
-- PREREQUISITE (must already be deployed + stable before running this):
--   Issue #059 Step 3.2 — write-path unification (commit 7114c03, deployed
--   to main at b432ac5 via Handover 121). All NEW writes are canonical
--   snake_case (`in_person` / `online`). Without 3.2, this backfill would
--   be partially undone by the next Sheet→D1 sync (upsert_attendee_full
--   overwrites participation_type unconditionally).
--
-- VERIFICATION BEFORE RUNNING:
--   1. Back up bethere-db first:
--        npx wrangler d1 export bethere-db --remote \
--          --output /tmp/bethere-backup-pre-0024-<date>.sql
--   2. Confirm new writes are canonical (legacy mess is frozen):
--        SELECT participation_type, COUNT(*) FROM attendees
--        GROUP BY participation_type ORDER BY 2 DESC;
--      (canonical `in_person`/`online` should dominate; legacy variants
--       should be flat or declining since 2026-06-27.)
--
-- WHAT THIS DOES:
--   Canonicalizes stored participation_type values to match
--   ParticipationType::as_str() in domain/src/models/attendee.rs
--   (`in_person` / `online` / `walkin` / `other`). Display-case
--   ("In-Person") is intentionally NOT stored in D1 — it lives only at
--   the presentation boundary (Sheet cells + UI labels).
--
-- IDEMPOTENT: re-running is safe. Canonical rows match their own LIKE
--   predicates but UPDATE sets the same value (no-op). Legacy rows are
--   the only meaningful change on first run.
--
-- ORDERING: in-person UPDATE runs before online UPDATE. Safe because no
--   prod value contains both tracks (verified pre-backfill).
--
-- PRESERVED (NOT touched by this migration):
--   - `walkin` sentinel — queried literally in db/attendees.rs and
--     claim/mint.rs to detect walk-ins. Canonicalizing it away would
--     break walk-in detection. (No `walkin` rows currently exist in
--     prod as of 2026-06-30, but the predicate below deliberately does
--     NOT match it; future walk-in rows will pass through untouched.)
--   - `test` — manual-review sentinel, excluded by the predicates.
--   - Any unrecognized non-empty value not matching in-person/online
--     patterns — left for manual review rather than guessed.
--
-- EXPECTED EFFECT (verified against prod 2026-06-30 via dry SELECT):
--   21 rows: ''           -> in_person  (legacy default; is_in_person
--                                     already treats empty as in-person)
--   16 rows: 'In-Person'  -> in_person
--   86 rows: 'in_person'  -> in_person  (idempotent, no-op)
--  111 rows: 'Online'     -> online
--  207 rows: 'online'     -> online     (idempotent, no-op)
--    1 row:  'test'       -> untouched
--    0 rows: 'walkin'     -> untouched (none currently exist)
--
-- POST-BACKFILL VERIFICATION:
--   SELECT participation_type, COUNT(*) FROM attendees
--   GROUP BY participation_type ORDER BY 2 DESC;
--   Expected: only `in_person`, `online`, `test` (and `walkin` if any
--   walk-ins have been added since).
--
-- FOLLOW-UP (optional, separate change): once rows are canonical, the
--   three IN_PERSON predicates in db/dashboard.rs, db/attendees.rs, and
--   contacts.rs can collapse from LIKE patterns to
--   `participation_type = 'in_person'`. Do NOT do that in this
--   migration — it belongs in code, with tests, on its own commit.

-- In-person slice: canonicalize empty + display-case + physical variants.
-- Empty -> in_person is a judgment call (legacy default); acceptable
-- because Attendee::is_in_person() already treats empty as in-person.
UPDATE attendees SET participation_type = 'in_person'
 WHERE participation_type IS NULL
    OR TRIM(participation_type) = ''
    OR LOWER(participation_type) LIKE '%in-person%'
    OR LOWER(participation_type) LIKE '%in person%'
    OR LOWER(participation_type) LIKE '%in_person%'
    OR LOWER(participation_type) LIKE '%physical%';

-- Online slice: canonicalize display-case + virtual variants.
-- Runs AFTER the in-person UPDATE (no prod value matches both).
UPDATE attendees SET participation_type = 'online'
 WHERE LOWER(participation_type) LIKE '%online%'
    OR LOWER(participation_type) LIKE '%virtual%';

-- walkin / test / other: deliberately NOT matched above.
