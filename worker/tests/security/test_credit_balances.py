"""Plan 028 W5 and issue 163: one-release `balances`, the release indexes and
the per-person release, on migrated SQLite.

The SQL is read out of `src/db/credit_ledger.rs` with the extractors in
`test_person_emails`, so this runs the statements the Worker sends.
"""

import sqlite3
import unittest

from test_person_emails import WORKER, concat_after, person_fragment, release_sql


def balances_sql():
    return concat_after("db/credit_ledger.rs", "pub async fn balances(")


GLOBAL_RELEASE = release_sql("RELEASE_ENDED_APPLIES_SQL")
PERSON_RELEASE = release_sql("RELEASE_PERSON_ENDED_APPLIES_SQL")


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
        plan = [row[3] for row in self.db.execute("EXPLAIN QUERY PLAN " + GLOBAL_RELEASE)]
        self.assertTrue(
            any("idx_credit_ledger_reason_event_email" in step and step.startswith("SEARCH a") for step in plan),
            plan,
        )
        self.assertFalse(any(step == "SCAN a" for step in plan), plan)


    def plan(self, sql, *binds):
        return [row[3] for row in self.db.execute("EXPLAIN QUERY PLAN " + sql, binds)]

    def test_person_release_seeks_by_email_not_by_reason(self):
        # Issue 163: on the reason index the scoped release still walks every
        # apply row in the ledger. It must seek the person's rows by email.
        plan = self.plan(PERSON_RELEASE, "a@x.io")
        self.assertTrue(
            any(step.startswith("SEARCH a USING INDEX idx_credit_ledger_bal (email=?)") for step in plan),
            plan,
        )
        self.assertFalse(any(step.startswith("SEARCH a USING INDEX idx_credit_ledger_reason") for step in plan), plan)
        self.assertFalse(any(step == "SCAN a" for step in plan), plan)

    def ended_applies(self):
        self.db.executemany(
            "INSERT INTO events(id, name, slug, event_start_ms, event_end_ms) VALUES (?,?,?,1,?)",
            [("ended", "Ended", "ended", 1000), ("upcoming", "Upcoming", "upcoming", 4_100_000_000_000)],
        )
        self.db.executemany(
            "INSERT INTO credit_ledger(email, organization_id, currency, delta, reason, event_id, deposit_id) VALUES (?,?,?,?,?,?,?)",
            [
                (email, "", "thb", -100, "apply", event, f"apply:{event}:{email}")
                for email in ("a@x.io", "alt@x.io", "b@x.io")
                for event in ("ended", "upcoming")
            ],
        )

    def returned(self):
        return sorted(self.db.execute("SELECT email, event_id FROM credit_ledger WHERE reason = 'return'").fetchall())

    def test_person_release_returns_only_that_person_ended_applies(self):
        self.ended_applies()
        self.assertEqual(self.db.execute(PERSON_RELEASE, ("alt@x.io",)).rowcount, 2)
        self.assertEqual(self.returned(), [("a@x.io", "ended"), ("alt@x.io", "ended")])
        self.assertEqual(self.db.execute(PERSON_RELEASE, ("a@x.io",)).rowcount, 0)
        # The global release picks up the stranger, and writes the same keys,
        # so the two never double-return.
        self.assertEqual(self.db.execute(GLOBAL_RELEASE).rowcount, 1)
        self.assertEqual(self.db.execute(GLOBAL_RELEASE).rowcount, 0)

    def test_person_release_gives_the_balance_the_global_release_gives(self):
        self.ended_applies()
        scoped = {}
        for email in ("a@x.io", "alt@x.io", "b@x.io", "nobody@x.io"):
            self.db.execute("SAVEPOINT s")
            self.db.execute(PERSON_RELEASE, (email,))
            scoped[email] = self.balances(email, "")
            self.db.execute("ROLLBACK TO s")
            self.db.execute("RELEASE s")
        self.db.execute(GLOBAL_RELEASE)
        for email, got in scoped.items():
            self.assertEqual(got, self.balances(email, ""), email)

if __name__ == "__main__":
    unittest.main()
