"""Plan 028 W4: the landing list's end-time filter, pushed into SQL.

`list_public_events_raw` now reads only events with `event_end_ms > now`. The
handler still applies the full rule in Rust (status, visibility, end time), so
the SQL must keep every row that rule keeps. Run on migrated SQLite with the
statement extracted from the source and `now` bound as a float, as D1 does.
"""

import re
import sqlite3
import unittest

from test_person_emails import SRC, WORKER, unescape

NOW = 1_790_000_000_000


def list_sql():
    source = (SRC / "db/events.rs").read_text()
    rest = source[source.index("pub async fn list_public_events_raw(") :]
    literal = re.search(r'format!\(\s*"((?:[^"\\]|\\[\s\S])*)"', rest).group(1)
    return unescape(literal).replace("{PUBLIC_EVENT_COLUMNS}", "id, status, visibility, event_end_ms")


def rust_retain(status, visibility, end_ms):
    """The handler's filter, after `parse_enum_column` normalised the enums."""
    return status == "active" and end_ms > NOW and visibility == "public"


class PublicEventsFilterTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        self.rows = [
            ("past", NOW - 1),
            ("ends-now", NOW),
            ("next-ms", NOW + 1),
            ("future", NOW + 86_400_000),
            ("zero", 0),
        ]
        self.db.executemany(
            "INSERT INTO events(id, name, slug, event_start_ms, event_end_ms, created_at, status, visibility) "
            "VALUES (?, ?, ?, 1, ?, '2026', 'active', 'public')",
            [(i, i, i, end) for i, end in self.rows],
        )

    def selected(self):
        return {row[0] for row in self.db.execute(list_sql(), (float(NOW),))}

    def test_sql_keeps_exactly_the_rows_the_handler_keeps(self):
        kept = {i for i, end in self.rows if rust_retain("active", "public", end)}
        self.assertEqual(self.selected(), kept)
        self.assertEqual(kept, {"next-ms", "future"})

    def test_the_statement_filters_in_sql(self):
        self.assertIn("WHERE event_end_ms > ?1", list_sql())


if __name__ == "__main__":
    unittest.main()
