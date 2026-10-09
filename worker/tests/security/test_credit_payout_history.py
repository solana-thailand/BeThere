"""The payout history query (`db/audit.rs`, GLOBAL_AUDIT_BY_ACTION_SQL).

Runs the worker's real SQL against the production migrations: only global
rows of the asked action, newest first, capped by the limit.
"""

import sqlite3
import unittest

from test_person_emails import WORKER, plain_after

SQL = plain_after("db/audit.rs", "pub(crate) const GLOBAL_AUDIT_BY_ACTION_SQL: &str =")
INSERT = "INSERT INTO audit_log (event_id, timestamp, actor, action, target, description, metadata) VALUES (?,?,?,?,?,?,?)"


class PayoutHistoryQueryTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        rows = [
            ("__global__", "2026-10-09 07:00:00", "s", "credit_refund_paid_out", "a@x", "d", "{}"),
            ("__global__", "2026-10-09 08:00:00", "s", "credit_refund_paid_out", "b@x", "d", "{}"),
            ("__global__", "2026-10-09 09:00:00", "s", "emails_linked", "c@x", "d", None),
            ("rtm-6", "2026-10-09 10:00:00", "s", "credit_refund_paid_out", "d@x", "d", "{}"),
        ]
        self.db.executemany(INSERT, rows)

    def tearDown(self):
        self.db.close()

    def targets(self, limit):
        return [r[3] for r in self.db.execute(SQL, ("credit_refund_paid_out", limit))]

    def test_only_global_rows_of_the_action_newest_first(self):
        self.assertEqual(self.targets(10), ["b@x", "a@x"])

    def test_limit_caps_the_page(self):
        self.assertEqual(self.targets(1), ["b@x"])


if __name__ == "__main__":
    unittest.main()
