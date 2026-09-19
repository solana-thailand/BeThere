-- 0044 — Keep the money facts when the personal data is purged.
--
-- The nightly cron (`worker/src/cleanup.rs` phase 2) hard-deletes every
-- `thb_deposits` row for an event at `event_end + refund_deadline + 90 days`:
--
--     DELETE FROM thb_deposits WHERE event_id = ?1
--
-- That is the right thing to do to the *personal* columns — `attendee_name`,
-- `bank_account`, `bank_name`, `account_name` and the slip URL are bank details
-- and images of bank slips. It is the wrong thing to do to the *amounts*. On
-- 2026-09-19 it took RTM#3's 14 deposits (7,000 THB) with it, including one row
-- of 500 THB that was neither refunded nor held as credit — money whose only
-- surviving trace was a local backup file. RTM#4 (20 rows, 10,000 THB) is due
-- 2026-10-17 17:00 +07 and RTM#5 (17 rows, 8,000 THB) 2026-11-21.
--
-- The two obligations do not actually conflict: keep the amounts, drop the
-- people. This table is what phase 2 writes immediately before it deletes, and
-- it is never itself cleaned up.
--
-- What is deliberately NOT here: attendee_name, bank_account, bank_name,
-- account_name, slip_url, refund_proof_url, verified_by. `attendee_id` IS kept
-- — it is an opaque uuid that `attendees`, `deposit_statuses` and the audit log
-- all retain anyway (the cron does not delete `attendees`), and without it the
-- archive can answer "was every deposit resolved" but not "what happened to
-- mine". `event_id` is the events **id**, which for a duplicated event is the
-- slug of the event it was copied from (`.issues/079`) — `event_slug` is
-- denormalised so the archive still reads correctly once the events row is gone.
--
-- `(event_id, attendee_id)` is UNIQUE so re-running the cron, or archiving an
-- event twice, cannot double-count the money.

CREATE TABLE IF NOT EXISTS thb_deposit_archive (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id          TEXT    NOT NULL,
    event_slug        TEXT    NOT NULL DEFAULT '',
    attendee_id       TEXT    NOT NULL,
    amount_thb        INTEGER NOT NULL,
    verified          INTEGER NOT NULL DEFAULT 0,
    verified_at       TEXT,
    uploaded_at       TEXT    NOT NULL DEFAULT '',
    refunded          INTEGER NOT NULL DEFAULT 0,
    refunded_at       TEXT,
    held_as_credit    INTEGER NOT NULL DEFAULT 0,
    held_as_credit_at TEXT,
    -- Whether a slip / refund proof existed at purge time. The object itself is
    -- NOT referenced: the URL is personal data and the R2 object may or may not
    -- still be there (see `.issues/126` — RTM#3's 13 slip images outlived their
    -- rows). A boolean answers "was there evidence" without keeping a pointer
    -- to a bank slip.
    had_slip          INTEGER NOT NULL DEFAULT 0,
    had_refund_proof  INTEGER NOT NULL DEFAULT 0,
    archived_at       TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_thb_deposit_archive_unique
    ON thb_deposit_archive(event_id, attendee_id);

CREATE INDEX IF NOT EXISTS idx_thb_deposit_archive_event
    ON thb_deposit_archive(event_id);
