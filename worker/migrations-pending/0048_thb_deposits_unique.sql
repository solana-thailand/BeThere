-- 0048_thb_deposits_unique.sql — `.issues/127`
--
-- ⚠️  DO NOT APPLY BEFORE 2026-09-28.
-- RTM #6 is on 2026-09-27. This rebuilds the live deposits table, and a rebuild
-- of the table the door runs on, days before the door opens, is the wrong trade
-- however careful the SQL is. Apply it the week after.
--
-- ── What is wrong ──────────────────────────────────────────────────────────
-- `thb_deposits` has no UNIQUE (event_id, attendee_id). One attendee can
-- therefore hold two deposit rows for one event, and the refund path pays per
-- row. That is how ฿700 went out twice. Every writer "checks first" in Rust —
-- `upload_thb_slip_handler` rejects an attendee who "already has a deposit" —
-- but a check in application code is not a constraint: two concurrent requests
-- both pass it, and `save_thb_deposit` then inserts twice because its
-- get-then-insert is not atomic either.
--
-- ── Current state of production, measured 2026-09-22 (read-only) ───────────
--   54 rows, 54 distinct (event_id, attendee_id) pairs, across 3 events.
-- So there is NOTHING TO DEDUPE: this migration applies cleanly as written.
-- Re-run `bash scripts/verify/thb_duplicate_report.sh` immediately before
-- applying — that number is a fact about a Tuesday, not a property of the
-- table, and RTM #6 will add ~24 rows before this is applied.
--
-- If the report is NOT clean when you run it, STOP. Deduping is not mechanical:
-- keep the row whose slip the organizer actually accepted, by hand, and only
-- then apply this.
--
-- ── Why a rebuild ──────────────────────────────────────────────────────────
-- SQLite cannot add a constraint to an existing table. The 0035 pattern is
-- followed: create the replacement, copy, drop, rename, recreate every index.
-- Unlike 0035 there are no triggers or views to drop first — verified by
-- reading sqlite_master for anything whose SQL names `thb_deposits`: none.
--
-- The column list is spelled out rather than `SELECT *` on purpose. An
-- `INSERT INTO … SELECT *` silently depends on column ORDER matching, and this
-- table has grown twice in the last week (0046 `slip_blake3`, 0047
-- `deposit_source`); naming them means a future column addition makes this fail
-- loudly instead of shifting every value one place left.

CREATE TABLE thb_deposits_v2 (
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
    refund_proof_url TEXT,
    held_as_credit  INTEGER NOT NULL DEFAULT 0,
    held_as_credit_at TEXT,
    slip_blake3     TEXT,
    deposit_source  TEXT
        CHECK (deposit_source IS NULL OR deposit_source IN ('cash', 'credit', 'comp')),
    -- The whole point of this migration.
    UNIQUE (event_id, attendee_id)
);

INSERT INTO thb_deposits_v2
    (id, attendee_id, event_id, amount_thb, slip_url, verified, verified_by,
     verified_at, uploaded_at, refunded, refunded_at, attendee_name,
     bank_account, bank_name, account_name, refund_proof_url, held_as_credit,
     held_as_credit_at, slip_blake3, deposit_source)
SELECT
     id, attendee_id, event_id, amount_thb, slip_url, verified, verified_by,
     verified_at, uploaded_at, refunded, refunded_at, attendee_name,
     bank_account, bank_name, account_name, refund_proof_url, held_as_credit,
     held_as_credit_at, slip_blake3, deposit_source
FROM thb_deposits;

DROP TABLE thb_deposits;
ALTER TABLE thb_deposits_v2 RENAME TO thb_deposits;

-- Every index from 0013, 0046 and 0047, recreated. A rebuild drops them with
-- the old table, and losing `idx_thb_deposits_event` would turn the admin
-- deposit screen into a full scan on every load.
CREATE INDEX IF NOT EXISTS idx_thb_deposits_event    ON thb_deposits(event_id);
CREATE INDEX IF NOT EXISTS idx_thb_deposits_attendee ON thb_deposits(event_id, attendee_id);
CREATE INDEX IF NOT EXISTS idx_thb_deposits_slip_hash
    ON thb_deposits(slip_blake3) WHERE slip_blake3 IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_thb_deposits_source
    ON thb_deposits(event_id, deposit_source);
