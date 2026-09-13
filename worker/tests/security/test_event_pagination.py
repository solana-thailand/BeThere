"""Execute the production event-page query against migrated SQLite."""

from pathlib import Path
import sqlite3
import unittest


WORKER = Path(__file__).resolve().parents[2]
QUERY = (WORKER / "src/db/sql/list_event_meta_page.sql").read_text()


class EventPaginationTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.db.row_factory = sqlite3.Row
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        rows = [
            ("private-new", "2026-01-03", "owner@example.com", ""),
            ("staff-middle", "2026-01-02", "", "staff@example.com"),
            ("owner-old", "2026-01-01", "owner@example.com", ""),
        ]
        self.db.executemany(
            """INSERT INTO events(
                   id,name,slug,event_start_ms,event_end_ms,created_at,
                   organizer_emails,staff_emails
               ) VALUES (?,? ,?,1,2,?,?,?)""",
            [(event, event, event, created, organizers, staff) for event, created, organizers, staff in rows],
        )

    def page(self, admin, email, created="", event_id="", limit=51):
        return list(self.db.execute(QUERY, (admin, email, created, event_id, limit)))

    def test_authorization_is_applied_before_limit(self):
        rows = self.page(0, "owner@example.com", limit=2)
        self.assertEqual([row["id"] for row in rows], ["private-new", "owner-old"])

    def test_keyset_is_stable_across_equal_timestamps(self):
        self.db.execute(
            """INSERT INTO events(
                   id,name,slug,event_start_ms,event_end_ms,created_at,organizer_emails
               ) VALUES ('private-a','a','a',1,2,'2026-01-03','owner@example.com')"""
        )
        first = self.page(0, "owner@example.com", limit=2)
        second = self.page(0, "owner@example.com", first[-1]["created_at"], first[-1]["id"], 2)
        ids = [row["id"] for row in first + second]
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual(ids, ["private-new", "private-a", "owner-old"])

    def test_super_admin_page_uses_pagination_index(self):
        plan = " ".join(
            str(column)
            for row in self.db.execute("EXPLAIN QUERY PLAN " + QUERY, (1, "", "", "", 2))
            for column in row
        )
        self.assertIn("idx_events_created_id", plan)


if __name__ == "__main__":
    unittest.main()
