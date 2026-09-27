"""One badge per recipient wallet per event (plan 025 §5.3 / 7.8, migration 0055).

Runs the worker's real claim-lock SQL, extracted from `db/claim_locks.rs`,
against the production migrations. Before 0055 the insert named its conflict
target `(event_id, token)`; with a second unique index that form raises on a
wallet clash instead of doing nothing, so this pins both halves: the insert
never raises, and the holder lookup says which constraint held.
"""

import sqlite3
import unittest

from test_person_emails import WORKER, plain_after

LOCKS = "db/claim_locks.rs"
INSERT = plain_after(LOCKS, "pub(crate) async fn acquire_claim_lock(")
HOLDER = plain_after(LOCKS, "let holder = db")
RELEASE = plain_after(LOCKS, "pub(crate) async fn release_claim_lock(")

WALLET_A = "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU"
WALLET_B = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"


class ClaimLockWalletTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())

    def tearDown(self):
        self.db.close()

    def acquire(self, event, token, wallet):
        """`acquire_claim_lock`: insert, then classify like `classify_holder`."""
        cur = self.db.execute(
            INSERT, (f"lock-{token}", event, token, wallet, "2026-09-27T00:05:00Z")
        )
        if cur.rowcount > 0:
            return "acquired"
        row = self.db.execute(HOLDER, (event, token, wallet)).fetchone()
        if row is None or row[0]:
            return "token_held"
        return "wallet_used"

    def test_first_claim_acquires(self):
        self.assertEqual(self.acquire("e1", "t1", WALLET_A), "acquired")

    def test_second_token_to_the_same_wallet_is_refused(self):
        self.acquire("e1", "t1", WALLET_A)
        self.assertEqual(self.acquire("e1", "t2", WALLET_A), "wallet_used")

    def test_retry_of_the_same_claim_reads_as_in_flight(self):
        self.acquire("e1", "t1", WALLET_A)
        self.assertEqual(self.acquire("e1", "t1", WALLET_A), "token_held")

    def test_same_token_other_wallet_is_still_the_token_lock(self):
        self.acquire("e1", "t1", WALLET_A)
        self.assertEqual(self.acquire("e1", "t1", WALLET_B), "token_held")

    def test_same_wallet_in_another_event_is_allowed(self):
        self.acquire("e1", "t1", WALLET_A)
        self.assertEqual(self.acquire("e2", "t9", WALLET_A), "acquired")

    def test_a_failed_mint_releases_the_wallet_for_a_retry(self):
        self.acquire("e1", "t1", WALLET_A)
        self.db.execute(RELEASE, ("e1", "t1"))
        self.assertEqual(self.acquire("e1", "t2", WALLET_A), "acquired")

    def test_the_index_is_exact_string_not_case_folded(self):
        # Base58 is case-sensitive: these are two different wallets.
        self.acquire("e1", "t1", WALLET_A)
        self.assertEqual(self.acquire("e1", "t2", WALLET_A.lower()), "acquired")

    def test_the_old_targeted_insert_would_have_raised(self):
        # Proves the test can fail: the pre-0055 statement meets the new index.
        old = INSERT.replace("ON CONFLICT DO NOTHING", "ON CONFLICT (event_id, token) DO NOTHING")
        self.assertNotEqual(old, INSERT)
        self.acquire("e1", "t1", WALLET_A)
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute(old, ("lock-t2", "e1", "t2", WALLET_A, "x"))


if __name__ == "__main__":
    unittest.main()
