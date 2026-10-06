"""The landing's deposit figures survive the nightly purge (issue #186).

`GET /api/public/stats` counted `thb_deposits` only, and `cleanup.rs` moves an
event's deposits to `thb_deposit_archive` and deletes them 90 days after the
refund window. RTM #2's 15 deposits left the public figure that way on
2026-10-04. These run the real stats SQL (`src/db/sql/public_stats*.sql`)
and the real archive and delete SQL against the production migrations, and
assert that archiving and purging leave every figure where it was.
"""

import unittest

from test_deposit_archive import ARCHIVE_SQL, DELETE_SQL
from test_person_emails import WORKER

STATS_SQL = (WORKER / "src/db/sql/public_stats.sql").read_text()
TIMINGS_SQL = (WORKER / "src/db/sql/public_stats_timings.sql").read_text()

OLD = "road-to-mainnet-2"
NEW = "road-to-mainnet-6"


class PublicStatsArchiveTests(unittest.TestCase):
    def setUp(self):
        import sqlite3

        self.db = sqlite3.connect(":memory:")
        self.db.row_factory = sqlite3.Row
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        for event in (OLD, NEW):
            self.db.execute(
                """INSERT INTO events (id, name, slug, status, event_start_ms, event_end_ms)
                   VALUES (?, ?, ?, 'completed', 1000, 2000)""",
                (event, event, event),
            )
        # Two events; on each, 3 people paid ฿500 and 2 came. One comp (฿0)
        # and one unverified slip must stay out of every figure.
        for event in (OLD, NEW):
            for i in range(3):
                self.attendee(event, f"{event}-p{i}", came=i < 2)
                self.deposit(event, f"{event}-p{i}", 500)
            self.attendee(event, f"{event}-staff", came=True)
            self.deposit(event, f"{event}-staff", 0, source="comp")
            self.attendee(event, f"{event}-late", came=False)
            self.deposit(event, f"{event}-late", 500, verified=0)

    def tearDown(self):
        self.db.close()

    def attendee(self, event, attendee, came):
        self.db.execute(
            """INSERT INTO attendees (id, event_id, email, participation_type, checked_in_at)
               VALUES (?, ?, ?, 'in_person', ?)""",
            (attendee, event, f"{attendee}@example.com", "2026-06-01T10:00:00Z" if came else None),
        )

    def deposit(self, event, attendee, amount, verified=1, source="cash"):
        self.db.execute(
            """INSERT INTO thb_deposits
                   (attendee_id, event_id, amount_thb, slip_url, verified, verified_at,
                    uploaded_at, refunded, refunded_at, deposit_source)
               VALUES (?, ?, ?, '/slip', ?, '2026-05-30T10:06:00Z',
                       '2026-05-30T10:00:00Z', 1, '1970-01-02T00:00:00Z', ?)""",
            (attendee, event, amount, verified, source),
        )

    def figures(self):
        rows = self.db.execute(STATS_SQL).fetchall()
        paid = sorted(
            (r["pt"], r["n"], r["came"]) for r in rows if r["kind"] == "paid"
        )
        thb = [r["n"] for r in rows if r["kind"] == "thb"]
        timings = sorted(
            (r["slip_s"], r["refund_s"]) for r in self.db.execute(TIMINGS_SQL)
        )
        return paid, thb, timings

    def archive(self, event):
        self.db.executescript(ARCHIVE_SQL.replace("?1", f"'{event}'"))

    def purge(self, event):
        self.db.executescript(DELETE_SQL.replace("?1", f"'{event}'"))

    def test_baseline_counts_paid_verified_money_only(self):
        paid, thb, timings = self.figures()
        self.assertEqual(paid, [("in_person", 6, 4)])
        self.assertEqual(thb, [3000])
        self.assertEqual(len(timings), 6)

    def test_archiving_then_purging_changes_no_figure(self):
        before = self.figures()
        self.archive(OLD)
        self.assertEqual(self.figures(), before, "mid-purge: counted twice?")
        self.purge(OLD)
        self.assertEqual(
            self.db.execute(
                "SELECT COUNT(*) AS n FROM thb_deposits WHERE event_id = ?", (OLD,)
            ).fetchone()["n"],
            0,
        )
        self.assertEqual(self.figures(), before, "after purge: archive not read?")

    def test_archiving_twice_changes_no_figure(self):
        before = self.figures()
        self.archive(OLD)
        self.archive(OLD)
        self.purge(OLD)
        self.archive(NEW)
        self.purge(NEW)
        self.assertEqual(self.figures(), before)

    def test_staff_and_organizers_are_not_payers(self):
        """Owner rule, 6 Oct: the landing's payers exclude the event's own
        staff and organizers (their money is still handled). The match is on
        whole list entries: an address that merely contains a staff address
        is an ordinary payer."""
        self.db.execute(
            "UPDATE events SET staff_emails = ?, organizer_emails = ? WHERE id = ?",
            (f" {NEW}-p0@example.com ,x@example.com", f"{NEW}-p1@EXAMPLE.com", NEW),
        )
        self.attendee(NEW, f"b{NEW}-p0", came=False)
        self.deposit(NEW, f"b{NEW}-p0", 500)
        paid, thb, _ = self.figures()
        self.assertEqual(paid, [("in_person", 5, 2)])
        staff = [
            (r["pt"], r["n"], r["came"])
            for r in self.db.execute(STATS_SQL)
            if r["kind"] == "paid_staff"
        ]
        self.assertEqual(staff, [("in_person", 2, 2)])
        self.assertEqual(thb, [3500])

    def test_staff_payer_stays_out_after_the_purge(self):
        self.db.execute(
            "UPDATE events SET staff_emails = ? WHERE id = ?", (f"{OLD}-p0@example.com", OLD)
        )
        before = self.figures()
        self.archive(OLD)
        self.purge(OLD)
        self.assertEqual(self.figures(), before)
        self.assertEqual(before[0], [("in_person", 5, 3)])


if __name__ == "__main__":
    unittest.main()
