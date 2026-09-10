"""Exercise wallet/email provenance rules against the production migrations."""

from pathlib import Path
import sqlite3
import unittest


WORKER = Path(__file__).resolve().parents[2]


class WalletEmailProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.db.row_factory = sqlite3.Row
        for migration in sorted((WORKER / "migrations").glob("*.sql")):
            self.db.executescript(migration.read_text())

    def tearDown(self):
        self.db.close()

    def verified_email(self, wallet):
        row = self.db.execute(
            """SELECT email FROM developer_profiles
               WHERE lower(wallet_address)=lower(?)
                 AND wallet_email_verified=1""",
            (wallet,),
        ).fetchone()
        return None if row is None else row["email"]

    def test_legacy_wallet_is_unknown_and_cannot_resolve_email_identity(self):
        self.db.execute(
            "INSERT INTO developer_profiles(email,wallet_address) VALUES (?,?)",
            ("legacy@example.com", "LegacyWallet"),
        )
        row = self.db.execute(
            """SELECT wallet_email_verified,wallet_email_verified_at,wallet_email_issuer
               FROM developer_profiles WHERE email='legacy@example.com'"""
        ).fetchone()
        self.assertEqual(row["wallet_email_verified"], 0)
        self.assertIsNone(row["wallet_email_verified_at"])
        self.assertIsNone(row["wallet_email_issuer"])
        self.assertIsNone(self.verified_email("LegacyWallet"))

    def test_deposit_recipient_wallet_is_not_an_identity_binding(self):
        self.db.execute(
            """INSERT INTO events(id,name,slug,status,event_start_ms,event_end_ms)
               VALUES ('event','Event','event','active',1000,2000)"""
        )
        self.db.execute(
            """INSERT INTO attendees(id,event_id,email)
               VALUES ('attendee','event','victim@example.com')"""
        )
        self.db.execute(
            """INSERT INTO deposit_statuses(
                   attendee_id,event_id,method,amount,currency,deposited_at,wallet_address
               ) VALUES ('attendee','event','usdc',100,'USDC','now','RecipientWallet')"""
        )
        self.assertIsNone(self.verified_email("RecipientWallet"))

    def test_verified_binding_resolves_case_insensitively_and_is_exclusive(self):
        self.db.execute(
            """INSERT INTO developer_profiles(
                   email,wallet_address,wallet_email_verified,
                   wallet_email_verified_at,wallet_email_issuer
               ) VALUES ('owner@example.com','VerifiedWallet',1,datetime('now'),'google')"""
        )
        self.assertEqual(
            self.verified_email("verifiedwallet"), "owner@example.com"
        )
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute(
                """INSERT INTO developer_profiles(
                       email,wallet_address,wallet_email_verified,
                       wallet_email_verified_at,wallet_email_issuer
                   ) VALUES ('attacker@example.com','VERIFIEDWALLET',1,datetime('now'),'google')"""
            )


if __name__ == "__main__":
    unittest.main()
