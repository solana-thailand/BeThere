-- 0058_attendee_display_code.sql — `.issues/178` (owner-approved 2026-10-08).
--
-- A short booking code printed under the ticket QR as `Nº XXXXXX`, read aloud
-- or typed at the door. 6 characters from 23456789ABCDEFGHJKMNPQRSTUVWXYZ
-- (no 0/O/1/I/L), random, unique per event. It is NOT a credential: only the
-- staff, event-scoped lookup accepts it. It is never derived from
-- `claim_token` (a capability, `.issues/071`) or from `attendees.id` (global
-- across events, `.issues/153`). The alphabet and length live in
-- `event_checkin_domain::models::attendee::DisplayCode`; the literal below
-- must match it (worker/tests/attendee_display_code.rs asserts that).
--
-- Backfill design — why this cannot fail on a collision
-- ------------------------------------------------------
-- 31^6 ≈ 8.9e8 codes per event. At 500 attendees in one event the chance of
-- at least one random collision is ~n²/2N ≈ 1.4e-4: small, but not small
-- enough to let `CREATE UNIQUE INDEX` decide whether the migration applies.
-- Pure SQL has no loop, so the backfill is bounded and ends in a pass that
-- makes a violation impossible instead of improbable:
--
--   1. every row gets a random code (SQLite `random()`, one draw per char;
--      not a CSPRNG, which is fine because the code is not a secret);
--   2. two repair passes redraw the code of any row that shares
--      (event_id, display_code) with a row of lower rowid — after them a
--      remaining duplicate needs ~1e-12 luck;
--   3. a last pass sets any duplicate that survived to NULL, keeping the
--      lowest-rowid holder. NULL rows are outside the partial index.
--   4. only then the partial UNIQUE index is built. It cannot fail.
--
-- A NULL code is a supported state, not a bug: the Worker assigns codes at
-- every attendee insert and lazily, with a bounded collision retry, the first
-- time a ticket with a NULL code is read
-- (`db::attendees::ensure_display_code`). That same path covers rows that old
-- code inserts between this migration and the code deploy.
--
-- Existing writers: none of them names `display_code`, and the index only
-- constrains non-NULL values, so no current INSERT/UPDATE can start failing
-- (memory: migration-constraints-break-existing-writers). Upserts
-- (`ON CONFLICT ... DO UPDATE`) leave the column untouched. `INSERT OR
-- REPLACE` (seed scripts only) drops the old row, and its replacement gets a
-- new code lazily.
--
-- Apply order: this migration first, read the schema back
-- (`PRAGMA table_info(attendees)`, `PRAGMA index_list(attendees)`), then the
-- code deploy. Rollback of the code is safe: old code ignores the column.

ALTER TABLE attendees ADD COLUMN display_code TEXT;

-- Non-unique helper index so the repair passes are not O(n²). Dropped below.
CREATE INDEX IF NOT EXISTS idx_attendees_display_code_backfill
    ON attendees(event_id, display_code);

UPDATE attendees SET display_code =
       substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
WHERE display_code IS NULL;

-- Repair pass 1.
UPDATE attendees SET display_code =
       substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
WHERE EXISTS (
    SELECT 1 FROM attendees b
    WHERE b.event_id = attendees.event_id
      AND b.display_code = attendees.display_code
      AND b.rowid < attendees.rowid
);

-- Repair pass 2.
UPDATE attendees SET display_code =
       substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
    || substr('23456789ABCDEFGHJKMNPQRSTUVWXYZ', 1 + ((random() % 31) + 31) % 31, 1)
WHERE EXISTS (
    SELECT 1 FROM attendees b
    WHERE b.event_id = attendees.event_id
      AND b.display_code = attendees.display_code
      AND b.rowid < attendees.rowid
);

-- Last resort: no duplicate can reach the unique index. The Worker fills
-- these lazily.
UPDATE attendees SET display_code = NULL
WHERE EXISTS (
    SELECT 1 FROM attendees b
    WHERE b.event_id = attendees.event_id
      AND b.display_code = attendees.display_code
      AND b.rowid < attendees.rowid
);

DROP INDEX IF EXISTS idx_attendees_display_code_backfill;

CREATE UNIQUE INDEX IF NOT EXISTS idx_attendees_event_display_code
    ON attendees(event_id, display_code)
    WHERE display_code IS NOT NULL;
