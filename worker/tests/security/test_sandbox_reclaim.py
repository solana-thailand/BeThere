"""Sandbox rent reclaim (SG3, `sandbox/reclaim.rs`): the worker's real SQL on
the production migrations. An event is due only when its run ended a margin
ago (the quota key lives 24 h from creation), its key is still live, and it
was not closed already; faucet and per-connection keys are never events."""

import sqlite3
import unittest

from test_person_emails import WORKER, plain_after

SRC = "sandbox/reclaim.rs"
DUE = plain_after(SRC, "pub const DUE_SQL: &str =")
MARK = plain_after(SRC, "pub const MARK_SQL: &str =")
AFTER = "+420 seconds"  # EVENT_SECONDS (120) + RECLAIM_MARGIN_SECONDS (300)


class ReclaimTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())

    def tearDown(self):
        self.db.close()

    def key(self, key, created_seconds_ago):
        # a quota key expires 24 h after it was claimed
        self.db.execute(
            "INSERT INTO advisory_locks (lock_key, expires_at) VALUES (?, datetime('now', ?))",
            (key, f"{86400 - created_seconds_ago:+d} seconds"),
        )

    def due(self, limit=5):
        return [r[0] for r in self.db.execute(DUE, (AFTER, limit))]

    def test_only_ended_live_unclosed_event_keys(self):
        self.key("sandbox:event:111", 3600)   # an hour old: due
        self.key("sandbox:event:222", 60)     # running: not yet
        self.key("sandbox:event:333", 90000)  # older than a day: key expired
        self.key("sandbox:faucet:wallet", 3600)
        self.key("sandbox:ip-event:abcd:444", 3600)
        self.assertEqual(self.due(), ["111"])

    def test_closed_is_not_tried_again_and_limit_holds(self):
        for i in range(7):
            self.key(f"sandbox:event:{i}00", 3600 + i)
        self.db.execute(MARK, ("000",))
        due = self.due(limit=5)
        self.assertEqual(len(due), 5)
        self.assertNotIn("000", due)
        # marking twice keeps one row
        self.db.execute(MARK, ("000",))
        self.assertEqual(
            self.db.execute("SELECT COUNT(*) FROM advisory_locks WHERE lock_key = 'sandbox:closed:000'").fetchone()[0], 1
        )


if __name__ == "__main__":
    unittest.main()
