"""Execute the staff-candidate query (plan 028 W2) against migrated SQLite."""

from pathlib import Path
import sqlite3
import unittest


WORKER = Path(__file__).resolve().parents[2]
QUERY = (WORKER / "src/db/sql/event_ids_for_staff.sql").read_text()


class EventStaffLookupTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        rows = [
            ("listed-new", "2026-01-03", "", "a@x.io, Vol@Example.com"),
            ("owner-only", "2026-01-02", "vol@example.com", ""),
            ("listed-old", "2026-01-01", "", "vol@example.com"),
            ("prefix", "2026-01-04", "", "xvol@example.com,vol@example.co"),
        ]
        self.db.executemany(
            """INSERT INTO events(
                   id,name,slug,event_start_ms,event_end_ms,created_at,
                   organizer_emails,staff_emails
               ) VALUES (?,?,?,1,2,?,?,?)""",
            [(i, i, i, created, org, staff) for i, created, org, staff in rows],
        )

    def ids(self, needle, limit=20):
        return [row[0] for row in self.db.execute(QUERY, (needle, limit))]

    def test_matches_whole_entries_case_and_space_insensitive(self):
        self.assertEqual(self.ids("vol@example.com"), ["listed-new", "listed-old"])

    def test_organizer_list_is_not_a_staff_match(self):
        self.assertNotIn("owner-only", self.ids("vol@example.com"))

    def test_limit_bounds_the_candidates(self):
        self.assertEqual(self.ids("vol@example.com", 1), ["listed-new"])

    def test_unknown_email_matches_nothing(self):
        self.assertEqual(self.ids("nobody@example.com"), [])


if __name__ == "__main__":
    unittest.main()
