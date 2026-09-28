"""Plan 028 W5: one-release `balances` and the release index, on migrated SQLite.

The SQL is read out of `src/db/credit_ledger.rs` with the extractors in
`test_person_emails`, so this runs the statements the Worker sends.
"""

import sqlite3
import unittest

from test_person_emails import WORKER, concat_after, person_fragment, plain_after


def balances_sql():
    return concat_after("db/credit_ledger.rs", "pub async fn balances(")


def release_sql():
    return plain_after("db/credit_ledger.rs", "RELEASE_ENDED_APPLIES_SQL: &str = ")


OLD_BALANCE = (
    "SELECT COALESCE(SUM(delta), 0) AS bal FROM credit_ledger WHERE email IN "
    + person_fragment("?1")
    + " AND organization_id = ?2 AND currency = ?3"
)


class CreditBalancesTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())
        self.db.execute("INSERT INTO person_emails(person_id, email, proof) VALUES ('p1','a@x.io','google'), ('p1','alt@x.io','google')")
        self.db.executemany(
            "INSERT INTO credit_ledger(email, organization_id, currency, delta, reason) VALUES (?,?,?,?,?)",
            [
                ("a@x.io", "", "thb", 500, "hold"),
                ("alt@x.io", "", "thb", 300, "hold"),
                ("a@x.io", "", "thb", -500, "apply"),
                ("a@x.io", "", "usdc", 5, "hold"),
                ("a@x.io", "org-b", "thb", 900, "hold"),
                ("b@x.io", "", "thb", 700, "hold"),
            ],
        )

    def balances(self, email, org):
        return dict(self.db.execute(balances_sql(), (email, org)).fetchall())

    def old(self, email, org, currency):
        return self.db.execute(OLD_BALANCE, (email, org, currency)).fetchone()[0]

    def test_one_query_equals_the_two_it_replaces(self):
        for email in ("a@x.io", "alt@x.io", "b@x.io", "nobody@x.io"):
            for org in ("", "org-b"):
                got = self.balances(email, org)
                for currency in ("thb", "usdc"):
                    self.assertEqual(got.get(currency, 0), self.old(email, org, currency), (email, org, currency))

    def test_linked_emails_share_one_balance(self):
        self.assertEqual(self.balances("alt@x.io", ""), {"thb": 300, "usdc": 5})

    def test_release_searches_the_new_index(self):
        plan = [row[3] for row in self.db.execute("EXPLAIN QUERY PLAN " + release_sql())]
        self.assertTrue(
            any("idx_credit_ledger_reason_event_email" in step and step.startswith("SEARCH a") for step in plan),
            plan,
        )
        self.assertFalse(any(step == "SCAN a" for step in plan), plan)


if __name__ == "__main__":
    unittest.main()
