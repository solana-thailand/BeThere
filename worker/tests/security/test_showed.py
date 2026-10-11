"""'I showed up' links (Super GOAT SG4): the worker's real SQL (src/showed.rs)
on the production migrations. One live link per (email, event); only the
owner revokes; a revoked link, a draft or a private event has no page; the
page carries no person data; erasure and the event purge delete the rows."""

import sqlite3
import unittest

from test_person_emails import WORKER, plain_after

SRC = "showed.rs"
LIVE = plain_after(SRC, "pub const LIVE_SQL")
CREATE = plain_after(SRC, "pub const CREATE_SQL")
REVOKE = plain_after(SRC, "pub const REVOKE_SQL")
MINE = plain_after(SRC, "pub const MINE_SQL")
PAGE = plain_after(SRC, "pub const PAGE_SQL")
ERASE = plain_after(SRC, "pub const ERASE_SQL")
PURGE = (WORKER / "src" / "db" / "sql" / "event_purge.sql").read_text()
T1, T2 = "a" * 32, "b" * 32


class ShowedTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        for eid, status, vis in [("e1", "completed", "public"), ("e2", "draft", "public"), ("e3", "completed", "private")]:
            self.db.execute(
                "INSERT INTO events (id, name, slug, status, visibility, event_start_ms, event_end_ms) VALUES (?, ?, ?, ?, ?, 1, 2)",
                (eid, eid.upper(), f"slug-{eid}", status, vis),
            )

    def tearDown(self):
        self.db.close()

    def create(self, token, email, event):
        self.db.execute(CREATE, (token, email, event, "2026-10-11T00:00:00Z"))

    def test_live_link_is_reused_and_page_has_no_person(self):
        self.create(T1, "me@x.example", "e1")
        self.assertEqual(self.db.execute(LIVE, ("me@x.example", "e1")).fetchone()[0], T1)
        page = self.db.execute(PAGE, (T1,)).fetchone()
        self.assertEqual(page, ("e1", "E1", "slug-e1", 1))
        cols = [d[0] for d in self.db.execute(PAGE, (T1,)).description]
        self.assertNotIn("email", cols)
        self.assertEqual(self.db.execute(MINE, ("me@x.example",)).fetchall(), [("slug-e1", T1)])

    def test_only_the_owner_revokes_and_revoked_is_gone(self):
        self.create(T1, "me@x.example", "e1")
        self.assertEqual(self.db.execute(REVOKE, (T1, "other@x.example", "now")).rowcount, 0)
        self.assertIsNotNone(self.db.execute(PAGE, (T1,)).fetchone())
        self.assertEqual(self.db.execute(REVOKE, (T1, "me@x.example", "now")).rowcount, 1)
        self.assertIsNone(self.db.execute(PAGE, (T1,)).fetchone())
        self.assertIsNone(self.db.execute(LIVE, ("me@x.example", "e1")).fetchone())
        self.assertEqual(self.db.execute(MINE, ("me@x.example",)).fetchall(), [])

    def test_draft_and_private_events_have_no_page(self):
        self.create(T1, "me@x.example", "e2")
        self.create(T2, "me@x.example", "e3")
        self.assertIsNone(self.db.execute(PAGE, (T1,)).fetchone())
        self.assertIsNone(self.db.execute(PAGE, (T2,)).fetchone())

    def test_erasure_and_purge(self):
        self.create(T1, "me@x.example", "e1")
        self.create(T2, "other@x.example", "e1")
        self.db.execute(ERASE, ("me@x.example",))
        self.assertEqual(self.db.execute("SELECT token FROM showed_links").fetchall(), [(T2,)])
        self.db.executescript(PURGE.replace("?1", "'e1'"))
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM showed_links").fetchone()[0], 0)


if __name__ == "__main__":
    unittest.main()
