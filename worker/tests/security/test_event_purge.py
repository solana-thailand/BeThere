"""Hard-deleting an event takes its own rows with it (issue 187).

`DELETE /api/events/{id}/delete` removed the `events` row only, so attendees,
deposits and answers stayed behind; every write smoke left a ฿500 fixture
deposit in prod. `db/event_purge.rs` now archives the deposits' amounts and
deletes the event's rows in one batch (`sql/event_purge.sql`). These run that
real SQL, split the way the Rust splits it, against the production
migrations.
"""

import re
import sqlite3
import unittest

from test_deposit_archive import ARCHIVE_SQL
from test_person_emails import WORKER

PURGE_SQL = (WORKER / "src/db/sql/event_purge.sql").read_text()
PURGE_RS = (WORKER / "src/db/event_purge.rs").read_text()

GONE = "smoke-1"
KEPT = "rtm-9"


def statements():
    """`purge_statements()` in event_purge.rs: split on a lone `;`, drop comments."""
    out = []
    for chunk in PURGE_SQL.split("\n;\n"):
        body = "\n".join(l for l in chunk.splitlines() if not l.lstrip().startswith("--"))
        if body.strip():
            out.append(body.strip())
    return out


def purged_tables():
    return {re.match(r"DELETE FROM (\w+) WHERE event_id = \?1$", s).group(1) for s in statements()}


def kept_tables():
    block = PURGE_RS[PURGE_RS.index("pub const EVENT_PURGE_KEEPS") :]
    block = block[: block.index("];")]
    return set(re.findall(r'"(\w+)"', block))


class EventPurgeTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.db.row_factory = sqlite3.Row
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        for event in (GONE, KEPT):
            self.db.execute(
                "INSERT INTO events (id, name, slug, status, event_start_ms, event_end_ms) VALUES (?, ?, ?, 'archived', 1000, 2000)",
                (event, event, event),
            )
            att = f"{event}-a"
            self.db.execute(
                "INSERT INTO attendees (id, event_id, email, participation_type) VALUES (?, ?, ?, 'in_person')",
                (att, event, f"{att}@example.com"),
            )
            self.db.execute(
                """INSERT INTO thb_deposits (attendee_id, event_id, amount_thb, slip_url, verified, uploaded_at)
                   VALUES (?, ?, 500, '/slip', 1, '2026-10-06T07:27:00Z')""",
                (att, event),
            )
            self.db.execute(
                """INSERT INTO deposit_statuses (event_id, attendee_id, method, amount, currency, deposited_at)
                   VALUES (?, ?, 'thb', 500, 'THB', '2026-10-06T07:27:00Z')""",
                (event, att),
            )
            self.db.execute(
                "INSERT INTO registration_responses (event_id, developer_email, field_key) VALUES (?, ?, 'role')",
                (event, f"{att}@example.com"),
            )
            self.db.execute(
                "INSERT INTO credit_ledger (email, currency, delta, reason, event_id) VALUES (?, 'THB', 500, 'hold', ?)",
                (f"{att}@example.com", event),
            )
            self.db.execute(
                "INSERT INTO audit_log (event_id, actor, action, target, description) VALUES (?, 'x', 'delete', ?, 'd')",
                (event, event),
            )

    def tearDown(self):
        self.db.close()

    def count(self, table, event):
        return self.db.execute(f"SELECT COUNT(*) AS n FROM {table} WHERE event_id = ?", (event,)).fetchone()["n"]

    def purge(self, event):
        self.db.executescript(ARCHIVE_SQL.replace("?1", f"'{event}'"))
        for sql in statements():
            self.db.execute(sql, (event,))
        self.db.execute("DELETE FROM events WHERE id = ?", (event,))

    def test_every_event_table_is_purged_or_kept_on_purpose(self):
        tables = {
            name
            for (name,) in self.db.execute("SELECT name FROM sqlite_master WHERE type = 'table'")
            if any(col["name"] == "event_id" for col in self.db.execute(f"PRAGMA table_info({name})"))
        }
        purged, kept = purged_tables(), kept_tables()
        self.assertFalse(purged & kept, "a table cannot be both purged and kept")
        self.assertEqual(tables - purged - kept, set(), "classify the new event_id table in event_purge.sql or EVENT_PURGE_KEEPS")
        self.assertEqual((purged | kept) - tables, set(), "a listed table no longer exists")

    def test_delete_leaves_nothing_pointing_at_the_event(self):
        self.purge(GONE)
        # Named, not only read from the SQL: an empty purge must fail here.
        core = {"attendees", "thb_deposits", "deposit_statuses", "registration_responses"}
        for table in sorted(purged_tables() | core):
            self.assertEqual(self.count(table, GONE), 0, table)
        for table in ("attendees", "thb_deposits", "deposit_statuses", "registration_responses"):
            self.assertEqual(self.count(table, KEPT), 1, f"{table}: another event's row went too")

    def test_money_and_trail_outlive_the_event(self):
        self.purge(GONE)
        archived = self.db.execute(
            "SELECT amount_thb, event_slug FROM thb_deposit_archive WHERE event_id = ?", (GONE,)
        ).fetchall()
        self.assertEqual([(r["amount_thb"], r["event_slug"]) for r in archived], [(500, GONE)])
        self.assertEqual(self.count("credit_ledger", GONE), 1, "a person's credit is not the event's")
        self.assertEqual(self.count("audit_log", GONE), 1)


if __name__ == "__main__":
    unittest.main()
