"""Door clicks (.plans/045 R4.10): the worker's real SQL on the migrations."""

from pathlib import Path
import sqlite3
import unittest

from test_person_emails import plain_after

WORKER = Path(__file__).resolve().parents[2]
COUNT = plain_after("door_clicks.rs", "pub const COUNT_SQL")


class DoorClickTests(unittest.TestCase):
    def test_counts_per_day_page_and_door(self):
        db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            db.executescript(migration.read_text())
        for day, page, door in [
            ("2026-10-11", "home", "events"),
            ("2026-10-11", "home", "events"),
            ("2026-10-11", "home", "try"),
            ("2026-10-12", "home", "events"),
        ]:
            db.execute(COUNT, (day, page, door))
        rows = db.execute("SELECT day, page, door, n FROM door_clicks ORDER BY day, door").fetchall()
        self.assertEqual(
            rows,
            [
                ("2026-10-11", "home", "events", 2),
                ("2026-10-11", "home", "try", 1),
                ("2026-10-12", "home", "events", 1),
            ],
        )


if __name__ == "__main__":
    unittest.main()
