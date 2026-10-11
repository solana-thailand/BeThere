"""Regular rules (Super GOAT SG1b): the worker's real SQL (src/regular.rs)
on the production migrations. The qualifying rows cover each registrant's
linked emails and only campaign events that ran; invited rows are out; the
table keeps its bounds; the event purge deletes the rule."""

import sqlite3
import unittest

from test_person_emails import WORKER, plain_after

SRC = "regular.rs"
RULE = plain_after(SRC, "pub const RULE_SQL")
UPSERT = plain_after(SRC, "pub const UPSERT_SQL")
DELETE = plain_after(SRC, "pub const DELETE_SQL")
ROWS = plain_after(SRC, "pub const REGISTRANT_DOOR_ROWS_SQL")
PURGE = (WORKER / "src" / "db" / "sql" / "event_purge.sql").read_text()


class RegularTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        self.db.execute("INSERT OR IGNORE INTO campaigns (id, title, status) VALUES ('s', 'S', 'active')")
        self.n = 0

    def tearDown(self):
        self.db.close()

    def event(self, eid, status="completed", campaign=True):
        self.db.execute(
            "INSERT INTO events (id, name, slug, status, visibility, event_start_ms, event_end_ms) VALUES (?, ?, ?, ?, 'public', 1, 2)",
            (eid, eid, eid, status),
        )
        if campaign:
            self.db.execute("INSERT INTO campaign_events (campaign_id, event_id) VALUES ('s', ?)", (eid,))

    def row(self, eid, email, came=True, approval="approved"):
        self.n += 1
        self.db.execute(
            "INSERT INTO attendees (id, event_id, email, participation_type, checked_in_at, checked_in_by, approval_status) VALUES (?, ?, ?, 'in_person', ?, ?, ?)",
            (f"a{self.n}", eid, email, "2026-10-01T10:00:00Z" if came else None, "door" if came else None, approval),
        )

    def test_rows_cover_linked_emails_and_campaign_events_only(self):
        self.event("next", status="active")  # the event being registered for
        self.event("rtm1")
        self.event("rtm2")
        self.event("oneoff", campaign=False)
        self.row("next", "me@x.example", came=False)
        self.row("next", "other@x.example", came=False)
        self.row("next", "staffinvite@x.example", came=False, approval="invited")
        self.row("rtm1", "old@x.example")
        self.row("rtm2", "me@x.example")
        self.row("oneoff", "me@x.example")
        self.row("rtm1", "stranger@x.example")
        for i, e in enumerate(["me@x.example", "old@x.example"]):
            self.db.execute("INSERT INTO person_emails (email, person_id, is_primary, proof) VALUES (?, 'p', ?, 'google')", (e, int(i == 0)))
        rows = sorted((r[0], r[1]) for r in self.db.execute(ROWS, ("next",)))
        self.assertEqual(
            rows,
            [("me@x.example", "next"), ("me@x.example", "rtm1"), ("me@x.example", "rtm2"), ("other@x.example", "next")],
        )

    def test_rule_crud_bounds_and_purge(self):
        self.event("e1")
        self.assertIsNone(self.db.execute(RULE, ("e1",)).fetchone())
        self.db.execute(UPSERT, ("e1", 3, 0, "now"))
        self.db.execute(UPSERT, ("e1", 4, 0, "later"))
        self.assertEqual(self.db.execute(RULE, ("e1",)).fetchone(), (4, 0))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute(UPSERT, ("e1", 0, 0, "now"))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute(UPSERT, ("e1", 3, -1, "now"))
        self.db.executescript(PURGE.replace("?1", "'e1'"))
        self.assertIsNone(self.db.execute(RULE, ("e1",)).fetchone())
        self.db.execute(UPSERT, ("e1", 3, 0, "now"))
        self.db.execute(DELETE, ("e1",))
        self.assertIsNone(self.db.execute(RULE, ("e1",)).fetchone())


if __name__ == "__main__":
    unittest.main()
