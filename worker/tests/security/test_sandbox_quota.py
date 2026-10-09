"""Sandbox rolling-day caps (plan 042 0.4, `sandbox/quota.rs`).

Runs the worker's real claim SQL against the production migrations. One
statement claims a per-item key and counts the live keys under its prefix, so
this pins: a first claim is granted, a repeat is held, the cap counts only its
own prefix and only live keys, and an expired key can be claimed again.
"""

import sqlite3
import unittest

from test_person_emails import plain_after

QUOTA = "sandbox/quota.rs"
CLAIM = plain_after(QUOTA, "pub const CLAIM_SQL: &str =")
HELD = plain_after(QUOTA, "const HELD_SQL: &str =")
DAY = "+86400 seconds"


def prefix_end(prefix):
    """Mirror of `quota::prefix_end`."""
    return prefix.rstrip(":") + ";"


class SandboxQuotaTests(unittest.TestCase):
    def setUp(self):
        from test_person_emails import WORKER

        self.db = sqlite3.connect(":memory:")
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())

    def tearDown(self):
        self.db.close()

    def claim(self, prefix, item, cap):
        key = f"{prefix}{item}"
        cur = self.db.execute(CLAIM, (key, DAY, prefix, prefix_end(prefix), cap))
        if cur.rowcount > 0:
            return "granted"
        held = self.db.execute(HELD, (key,)).fetchone()
        return "held" if held else "cap"

    def expire(self, key):
        self.db.execute(
            "UPDATE advisory_locks SET expires_at = datetime('now', '-1 seconds') WHERE lock_key = ?",
            (key,),
        )

    def test_first_claim_is_granted(self):
        self.assertEqual(self.claim("sandbox:faucet:", "w1", 2), "granted")

    def test_same_wallet_twice_is_held(self):
        self.claim("sandbox:faucet:", "w1", 2)
        self.assertEqual(self.claim("sandbox:faucet:", "w1", 2), "held")

    def test_cap_stops_a_new_wallet(self):
        self.claim("sandbox:faucet:", "w1", 2)
        self.claim("sandbox:faucet:", "w2", 2)
        self.assertEqual(self.claim("sandbox:faucet:", "w3", 2), "cap")

    def test_cap_counts_only_its_own_prefix(self):
        self.claim("sandbox:event:", "1", 1)
        self.db.execute(
            "INSERT INTO advisory_locks (lock_key, expires_at) VALUES ('sandbox:faucetx', datetime('now', '+1 day'))"
        )
        self.assertEqual(self.claim("sandbox:faucet:", "w1", 1), "granted")

    def test_expired_keys_free_the_cap(self):
        self.claim("sandbox:faucet:", "w1", 1)
        self.expire("sandbox:faucet:w1")
        self.assertEqual(self.claim("sandbox:faucet:", "w2", 1), "granted")

    def test_expired_key_can_be_claimed_again(self):
        self.claim("sandbox:faucet:", "w1", 2)
        self.expire("sandbox:faucet:w1")
        self.assertEqual(self.claim("sandbox:faucet:", "w1", 2), "granted")


if __name__ == "__main__":
    unittest.main()
