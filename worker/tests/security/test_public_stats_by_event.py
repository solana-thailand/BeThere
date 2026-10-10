"""Per-event payers (`public_stats_by_event.sql`, .plans/045 R4.7).

Runs the real statements against the production migrations: public events
only, staff payers out, the archive counted once, and the per-event payers
summing to the totals' `paid` rows over the same (public) events.
"""

import sqlite3
import unittest

from test_person_emails import WORKER
from test_deposit_archive import ARCHIVE_SQL, DELETE_SQL

SQL_DIR = WORKER / "src" / "db" / "sql"
BY_EVENT = (SQL_DIR / "public_stats_by_event.sql").read_text()
TOTALS = (SQL_DIR / "public_stats.sql").read_text()


class PublicStatsByEventTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        self.n = 0

    def tearDown(self):
        self.db.close()

    def event(self, eid, start, visibility="public", staff=""):
        self.db.execute(
            "INSERT INTO events (id, name, slug, status, visibility, event_start_ms, event_end_ms, staff_emails) "
            "VALUES (?, ?, ?, 'completed', ?, ?, ?, ?)",
            (eid, f"Event {eid}", eid, visibility, start, start + 1, staff),
        )

    def payer(self, eid, email, pt="In-Person", came=True, source="cash", amount=500):
        self.n += 1
        aid = f"att-{self.n}"
        self.db.execute(
            "INSERT INTO attendees (id, event_id, email, participation_type, checked_in_at) VALUES (?, ?, ?, ?, ?)",
            (aid, eid, email, pt, "2026-10-01T10:00:00Z" if came else None),
        )
        self.db.execute(
            """INSERT INTO thb_deposits
                   (attendee_id, event_id, amount_thb, slip_url, verified, verified_at,
                    uploaded_at, deposit_source)
               VALUES (?, ?, ?, '/slip', 1, '2026-09-30T10:06:00Z', '2026-09-30T10:00:00Z', ?)""",
            (aid, eid, amount, source),
        )

    def archive_and_purge(self, eid):
        """The nightly purge's real statements: archive, then delete."""
        self.db.executescript(ARCHIVE_SQL.replace("?1", f"'{eid}'"))
        self.db.executescript(DELETE_SQL.replace("?1", f"'{eid}'"))

    def by_event(self):
        return [(r[2], r[4], r[5], r[6]) for r in self.db.execute(BY_EVENT)]

    def test_counts_on_site_payers_per_event_and_drops_staff_and_comp(self):
        self.event("e1", 1, staff="crew@x.example")
        self.payer("e1", "a@x.example")
        self.payer("e1", "b@x.example", came=False)
        self.payer("e1", "crew@x.example")  # staff: out
        self.payer("e1", "c@x.example", source="comp", amount=0)  # comp: out
        self.assertEqual(self.by_event(), [("e1", "In-Person", 2, 1)])
        self.archive_and_purge("e1")
        self.assertEqual(self.by_event(), [("e1", "In-Person", 2, 1)], "the archive keeps the count")

    def test_private_events_are_not_listed(self):
        self.event("pub", 1)
        self.event("priv", 2, visibility="private")
        self.payer("pub", "a@x.example")
        self.payer("priv", "b@x.example")
        self.assertEqual([r[0] for r in self.by_event()], ["pub"])

    def test_rows_are_oldest_first(self):
        self.event("late", 20)
        self.event("early", 10)
        self.payer("late", "a@x.example")
        self.payer("early", "b@x.example")
        self.assertEqual([r[0] for r in self.by_event()], ["early", "late"])

    def test_per_event_payers_sum_to_the_totals_paid_rows(self):
        self.event("e1", 1, staff="crew@x.example")
        self.event("e2", 2)
        for i in range(4):
            self.payer("e1", f"p{i}@x.example", came=i != 0)
        self.payer("e1", "crew@x.example")
        self.payer("e2", "o@x.example", pt="Online")
        self.payer("e2", "w@x.example", pt="walkin")
        per_event = {}
        for _, pt, n, came in self.by_event():
            per_event[pt] = tuple(a + b for a, b in zip(per_event.get(pt, (0, 0)), (n, came)))
        totals = {r[1]: (r[2], r[3]) for r in self.db.execute(TOTALS) if r[0] == "paid"}
        self.assertEqual(per_event, totals)


if __name__ == "__main__":
    unittest.main()
