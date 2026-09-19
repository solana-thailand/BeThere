"""The nightly purge keeps the money and drops the people (issue #126).

`cleanup.rs` phase 2 hard-deletes `thb_deposits` 90 days after the refund
window closes. That is right for `bank_account` and the slip URL and wrong for
`amount_thb`: on 2026-09-19 it took RTM#3's 14 deposits (7,000 THB) with it,
one of which was neither refunded nor held as credit. Migration 0044 adds the
archive the delete now runs behind, and these run its real SQL — extracted from
the Rust source, same harness as `test_person_emails` — against the production
migrations.
"""

import re
import unittest

from test_person_emails import LITERAL, WORKER, plain_after, unescape

ARCHIVE_SQL = plain_after(
    "db/thb_deposits.rs", "pub async fn archive_thb_deposits_for_event("
)
DELETE_SQL = plain_after(
    "db/thb_deposits.rs", "pub async fn delete_thb_deposits_for_event("
)


def coverage_sql():
    """The coverage SELECT — the archive fn's *second* statement.

    `plain_after` anchors on a substring and takes the next string literal, but
    every distinctive phrase here lives inside a literal, so anchoring on one
    would return the anchor itself. Slice the function, then take the literal
    after `let stmt = db`.
    """
    source = (WORKER / "src/db/thb_deposits.rs").read_text()
    fn = source[source.index("pub async fn archive_thb_deposits_for_event(") :]
    rest = fn[fn.index("let stmt = db") :]
    return unescape(re.search(LITERAL, rest).group(1))


COVERAGE_SQL = coverage_sql()

EVENT = "road-to-mainnet-2-bangkok-copy"
SLUG = "road-to-mainnet-3-bangkok"

# The personal columns the purge exists to remove. None of them may appear in
# the archive — that is the whole point of archiving instead of not deleting.
PII_COLUMNS = {
    "attendee_name",
    "bank_account",
    "bank_name",
    "account_name",
    "slip_url",
    "refund_proof_url",
    "verified_by",
}


class DepositArchiveTests(unittest.TestCase):
    def setUp(self):
        import sqlite3

        self.db = sqlite3.connect(":memory:")
        self.db.row_factory = sqlite3.Row
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        self.db.execute(
            """INSERT INTO events (id, name, slug, event_start_ms, event_end_ms)
               VALUES (?, 'RTM #3', ?, 1, 2)""",
            (EVENT, SLUG),
        )

    def tearDown(self):
        self.db.close()

    # -- helpers ---------------------------------------------------------

    def deposit(self, attendee, amount, refunded=0, held=0, slip=True, event=EVENT):
        self.db.execute(
            """INSERT INTO thb_deposits
                   (attendee_id, event_id, amount_thb, slip_url, verified,
                    verified_by, verified_at, uploaded_at, refunded, refunded_at,
                    attendee_name, bank_account, bank_name, account_name,
                    held_as_credit)
               VALUES (?, ?, ?, ?, 1, 'staff@example.com', '2026-06-01',
                       '2026-05-30', ?, ?, 'A Name', '1234567890', 'A Bank',
                       'An Account', ?)""",
            (
                attendee,
                event,
                amount,
                f"/api/storage/slips/{event}/{attendee}" if slip else "",
                refunded,
                "2026-06-10" if refunded else None,
                held,
            ),
        )

    def coverage(self):
        """(archived, unarchived) — the real counts the Rust decides on."""
        row = self.db.execute(COVERAGE_SQL, (EVENT,)).fetchone()
        return row["archived"], row["unarchived"]

    def archive(self):
        """Run the real archive SQL, then report coverage like the Rust does."""
        self.db.executescript(ARCHIVE_SQL.replace("?1", f"'{EVENT}'"))
        return self.coverage()

    def rtm3(self):
        """RTM#3's real shape: 11 refunded, 2 held as credit, 1 neither."""
        for i in range(11):
            self.deposit(f"ref-{i}", 500, refunded=1)
        for i in range(2):
            self.deposit(f"held-{i}", 500, held=1)
        self.deposit("stranded", 500)

    # -- the money survives ----------------------------------------------

    def test_every_deposit_is_archived_before_the_delete(self):
        self.rtm3()
        self.assertEqual(self.archive(), (14, 0))
        self.db.executescript(DELETE_SQL.replace("?1", f"'{EVENT}'"))
        self.assertEqual(
            self.db.execute("SELECT COUNT(*) AS n FROM thb_deposits").fetchone()["n"], 0
        )
        # The reconciliation that was impossible after 2026-09-19.
        row = self.db.execute(
            """SELECT COUNT(*) AS rows, SUM(amount_thb) AS thb,
                      SUM(refunded) AS refunded, SUM(held_as_credit) AS held,
                      SUM(CASE WHEN refunded=0 AND held_as_credit=0 THEN 1 ELSE 0 END) AS neither,
                      SUM(CASE WHEN refunded=0 AND held_as_credit=0 THEN amount_thb ELSE 0 END) AS neither_thb
               FROM thb_deposit_archive"""
        ).fetchone()
        self.assertEqual(
            (row["rows"], row["thb"], row["refunded"], row["held"], row["neither"], row["neither_thb"]),
            (14, 7000, 11, 2, 1, 500),
        )

    def test_the_personal_columns_are_not_carried_over(self):
        self.rtm3()
        self.archive()
        columns = {
            r["name"]
            for r in self.db.execute("PRAGMA table_info(thb_deposit_archive)").fetchall()
        }
        leaked = columns & PII_COLUMNS
        self.assertEqual(leaked, set(), f"archive must not hold personal data: {leaked}")
        # And no archived value equals one of the personal values we inserted.
        values = set()
        for r in self.db.execute("SELECT * FROM thb_deposit_archive").fetchall():
            values.update(str(v) for v in tuple(r))
        for secret in ("A Name", "1234567890", "A Bank", "An Account", "staff@example.com"):
            self.assertNotIn(secret, values)

    def test_evidence_survives_as_a_boolean_not_a_pointer(self):
        # A URL to a bank slip is personal data; "there was a slip" is not.
        self.deposit("with-slip", 500, slip=True)
        self.deposit("no-slip", 500, slip=False)
        self.archive()
        rows = {
            r["attendee_id"]: r["had_slip"]
            for r in self.db.execute(
                "SELECT attendee_id, had_slip FROM thb_deposit_archive"
            ).fetchall()
        }
        self.assertEqual(rows, {"with-slip": 1, "no-slip": 0})

    # -- re-running is safe ----------------------------------------------

    def test_archiving_twice_does_not_double_the_money(self):
        self.rtm3()
        self.assertEqual(self.archive(), (14, 0))
        self.assertEqual(self.archive(), (14, 0))
        total = self.db.execute(
            "SELECT SUM(amount_thb) AS thb FROM thb_deposit_archive"
        ).fetchone()["thb"]
        self.assertEqual(total, 7000)

    def test_a_retry_after_a_partial_archive_still_covers_everything(self):
        # The cron can die between the archive and the delete. Tomorrow's run
        # re-archives the survivors and must end up with the same coverage.
        self.rtm3()
        self.archive()
        self.db.execute("DELETE FROM thb_deposit_archive WHERE attendee_id='stranded'")
        self.assertEqual(self.archive(), (14, 0))

    # -- one attendee, two deposits (issue #127) -------------------------

    def test_two_deposits_for_one_attendee_are_both_archived(self):
        # `thb_deposits` has no UNIQUE on (event_id, attendee_id) — 0013 makes it
        # a plain INDEX — and `save_thb_deposit` is read-then-insert-or-update,
        # so two concurrent uploads for one attendee both take the insert branch.
        # 0044 keyed the archive on that pair: one row archived, both deleted.
        self.deposit("att-1", 500)
        self.deposit("att-1", 700)
        self.assertEqual(self.archive(), (2, 0))
        total = self.db.execute(
            "SELECT SUM(amount_thb) AS thb FROM thb_deposit_archive"
        ).fetchone()["thb"]
        self.assertEqual(total, 1200, "both deposits are money; both must survive")

    def test_the_duplicate_pair_is_still_idempotent(self):
        self.deposit("att-1", 500)
        self.deposit("att-1", 700)
        self.archive()
        self.assertEqual(self.archive(), (2, 0))
        self.assertEqual(
            self.db.execute("SELECT COUNT(*) AS n FROM thb_deposit_archive").fetchone()["n"],
            2,
            "re-running keys on the source row, so it adds nothing and loses nothing",
        )

    def test_coverage_is_short_when_a_row_is_not_archived(self):
        # What the delete is gated on: `is_complete()` is `unarchived == 0`.
        self.deposit("att-1", 500)
        self.deposit("att-2", 700)
        self.archive()
        self.db.execute("DELETE FROM thb_deposit_archive WHERE attendee_id='att-2'")
        self.assertEqual(
            self.coverage()[1], 1, "an unarchived live row must be counted as one"
        )

    def test_a_big_archive_does_not_excuse_an_unarchived_row(self):
        """The reason coverage matches ids instead of comparing totals.

        `archived >= live` asks whether the archive is big enough, which is a
        different question: rows that correspond to nothing currently live —
        0044-era rows with a NULL `source_deposit_id`, or rows kept from an
        earlier purge of an event that has since taken new deposits — inflate
        `archived` and paper over a genuinely unarchived row.
        """
        self.deposit("att-1", 500)
        self.db.executemany(
            """INSERT INTO thb_deposit_archive
                   (event_id, event_slug, attendee_id, amount_thb, uploaded_at, source_deposit_id)
               VALUES (?, 'slug', ?, 500, '', NULL)""",
            [(EVENT, "ghost-1"), (EVENT, "ghost-2"), (EVENT, "ghost-3")],
        )
        archived, unarchived = self.coverage()
        self.assertEqual(archived, 3, "three archive rows, none of them the live one")
        self.assertGreaterEqual(
            archived, 1, "the old `archived >= live` gate would have passed here"
        )
        self.assertEqual(
            unarchived, 1, "matching by id sees the live deposit is not archived"
        )

    def test_a_rerun_after_a_successful_purge_stays_safe(self):
        # live = 0 against a full archive. `>=` not `==`, or the retry would
        # refuse forever on an event that is already done.
        self.deposit("att-1", 500)
        self.archive()
        self.db.executescript(DELETE_SQL.replace("?1", f"'{EVENT}'"))
        archived, unarchived = self.archive()
        self.assertEqual((archived, unarchived), (1, 0))
        self.assertEqual(unarchived, 0, "an already-purged event has nothing unmatched")

    # -- the archive outlives the event ----------------------------------

    def test_slug_is_denormalised_so_the_archive_reads_after_the_event_is_gone(self):
        self.deposit("a", 500)
        self.archive()
        self.db.execute("DELETE FROM events WHERE id=?", (EVENT,))
        row = self.db.execute(
            "SELECT event_id, event_slug FROM thb_deposit_archive"
        ).fetchone()
        # id and slug differ on every duplicated event (.issues/079); reading the
        # id alone is how you conclude RTM#4's deposits belong to RTM#3.
        self.assertEqual((row["event_id"], row["event_slug"]), (EVENT, SLUG))

    def test_a_missing_event_row_falls_back_to_the_id_not_an_empty_slug(self):
        self.deposit("a", 500)
        self.db.execute("DELETE FROM events WHERE id=?", (EVENT,))
        self.archive()
        self.assertEqual(
            self.db.execute("SELECT event_slug FROM thb_deposit_archive").fetchone()[
                "event_slug"
            ],
            EVENT,
        )

    def test_other_events_are_untouched(self):
        self.deposit("mine", 500)
        self.deposit("theirs", 900, event="some-other-event")
        self.archive()
        rows = self.db.execute(
            "SELECT event_id, amount_thb FROM thb_deposit_archive"
        ).fetchall()
        self.assertEqual([(r["event_id"], r["amount_thb"]) for r in rows], [(EVENT, 500)])


if __name__ == "__main__":
    unittest.main()
