"""Execute the per-track head-count query (plan 028 W3) against migrated SQLite.

The Rust side classifies each returned `participation_type` with
`TrackCounts::add`; this pins what the SQL hands it: one row per distinct
stored value for that event only, with exact counts. The column is NOT NULL
(default ''), which `parse` reads as in-person.
"""

from pathlib import Path
import sqlite3
import unittest


WORKER = Path(__file__).resolve().parents[2]
QUERY = (WORKER / "src/db/sql/attendee_counts_by_participation.sql").read_text()


class AttendeeTrackCountTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        rows = [
            ("e1", "a@x.io", "In-Person"),
            ("e1", "b@x.io", "In-Person"),
            ("e1", "c@x.io", "Online"),
            ("e1", "d@x.io", "walkin"),
            ("e1", "f@x.io", ""),
            ("e1", "g@x.io", "retrospective"),
            ("e2", "a@x.io", "Online"),
        ]
        self.db.executemany(
            "INSERT INTO attendees(id, event_id, email, participation_type) VALUES (?,?,?,?)",
            [(f"{e}-{m}", e, m, t) for e, m, t in rows],
        )

    def counts(self, event_id):
        return dict(self.db.execute(QUERY, (event_id,)).fetchall())

    def test_one_row_per_stored_value(self):
        self.assertEqual(
            self.counts("e1"),
            {"In-Person": 2, "Online": 1, "walkin": 1, "": 1, "retrospective": 1},
        )

    def test_counts_only_the_requested_event(self):
        self.assertEqual(self.counts("e2"), {"Online": 1})

    def test_total_equals_the_row_count_the_list_path_read(self):
        listed = self.db.execute("SELECT COUNT(*) FROM attendees WHERE event_id = ?", ("e1",)).fetchone()[0]
        self.assertEqual(sum(self.counts("e1").values()), listed)

    def test_an_event_with_no_rows_returns_nothing(self):
        # `count_tracks_by_event` maps this to None, the "ask the sheet" signal.
        self.assertEqual(self.counts("missing"), {})


if __name__ == "__main__":
    unittest.main()
