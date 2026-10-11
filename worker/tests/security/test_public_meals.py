"""Meals ordered (`public_stats_meals.sql`, .plans/045 R4.11).

Runs the real statement against the production migrations: one row per
(event with a count, participation type), the registration rule of the totals
(`invited` out), drafts and archived events out, a count of 0 out, and an event
with a count but no registrations still listed once. The event purge deletes
the count.
"""

import sqlite3
import unittest

from test_person_emails import WORKER

SQL_DIR = WORKER / "src" / "db" / "sql"
MEALS = (SQL_DIR / "public_stats_meals.sql").read_text()
PURGE = (SQL_DIR / "event_purge.sql").read_text()


class PublicMealsTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        self.n = 0

    def tearDown(self):
        self.db.close()

    def event(self, eid, status="completed", ordered=None):
        self.db.execute(
            "INSERT INTO events (id, name, slug, status, visibility, event_start_ms, event_end_ms) "
            "VALUES (?, ?, ?, ?, 'public', 1, 2)",
            (eid, f"Event {eid}", eid, status),
        )
        if ordered is not None:
            self.db.execute(
                "INSERT INTO event_meals (event_id, ordered, updated_at) VALUES (?, ?, 'now')",
                (eid, ordered),
            )

    def attendee(self, eid, pt="In-Person", came=False, approval="approved"):
        self.n += 1
        self.db.execute(
            "INSERT INTO attendees (id, event_id, email, participation_type, checked_in_at, approval_status) "
            "VALUES (?, ?, ?, ?, ?, ?)",
            (f"a{self.n}", eid, f"p{self.n}@x.example", pt, "2026-10-01T10:00:00Z" if came else None, approval),
        )

    def rows(self):
        return [tuple(r) for r in self.db.execute(MEALS)]

    def test_groups_per_event_and_type_with_the_totals_rule(self):
        self.event("e1", ordered=3)
        self.attendee("e1", came=True)
        self.attendee("e1", came=False)
        self.attendee("e1", pt="Online")
        self.attendee("e1", approval="invited")
        self.assertEqual(
            sorted(self.rows()),
            [("e1", 3, "In-Person", 2, 1), ("e1", 3, "Online", 1, 0)],
        )

    def test_no_count_draft_and_zero_are_out_and_empty_event_is_in(self):
        self.event("none")
        self.attendee("none")
        self.event("draft", status="draft", ordered=10)
        self.attendee("draft")
        self.event("zero", ordered=0)
        self.event("empty", ordered=12)
        self.assertEqual(self.rows(), [("empty", 12, "", 0, 0)])

    def test_count_is_bounded(self):
        self.event("e1")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO event_meals VALUES ('e1', -1, 'now')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO event_meals VALUES ('e1', 100001, 'now')")

    def test_purge_deletes_the_count(self):
        self.event("e1", ordered=5)
        self.db.executescript(PURGE.replace("?1", "'e1'"))
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM event_meals").fetchone()[0], 0)


if __name__ == "__main__":
    unittest.main()
