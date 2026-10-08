"""Credit payout hardening (`.issues/190`).

Runs the worker's real SQL — extracted from the Rust sources with the
`test_person_emails` harness — against the full migration chain (0001 … 0059):

- the payout reversal is ONE guarded statement: it writes only when the
  payable balance still equals what the organizer confirmed, so a stale amount
  writes nothing, a spend and a clear can race in either order without the
  balance going negative, and a second clear is a no-op;
- the payout account is stored only while a request is open, read by the
  queue, and deleted at payout, by the nightly purge and by PDPA erasure.
"""

import sqlite3
import unittest

from test_person_emails import (
    GMAIL,
    OTHER,
    WORK,
    CreditFixture,
    concat_after,
    plain_after,
)

TRY_REFUND_SQL = concat_after("db/credit_ledger.rs", "pub(crate) const TRY_REFUND_SQL")
REFUND_RECORDED_SQL = plain_after("db/credit_ledger.rs", "pub(crate) const REFUND_RECORDED_SQL")
POSITIVE_SQL = concat_after("db/credit_ledger.rs", "pub async fn positive_balances(")
TRY_SPEND_SQL = concat_after("db/credit_ledger.rs", "pub async fn try_spend(")
NEGATIVE_SQL = plain_after("db/credit_ledger.rs", "let negative_balances = count_query(")
SET_FLAG_SQL = plain_after("db/contacts.rs", "pub(crate) async fn set_credit_refund_requested(")
SAVE_SQL = plain_after("db/credit_refund_accounts.rs", "pub(crate) const SAVE_SQL")
DELETE_FOR_PERSON_SQL = concat_after(
    "db/credit_refund_accounts.rs", "pub(crate) const DELETE_FOR_PERSON_SQL"
)
PURGE_SQL = plain_after("db/credit_refund_accounts.rs", "pub(crate) const PURGE_SQL")
CLEAR_FLAG_SQL = concat_after(
    "db/contacts.rs", "pub(crate) async fn clear_credit_refund_requested(",
    opener=".prepare(concat!(",
)
# Pinned to the source by `PiiErasureSqlIsTheSourceSql` below.
PII_ACCOUNT_SQL = "DELETE FROM credit_refund_accounts WHERE email = LOWER(?1)"
QUEUE_SQL = concat_after("db/contacts.rs", "pub async fn credit_refund_requests(")

REQUESTED_AT = "2026-10-01 09:00:00"
NOTE = "held-credit payout processed by organizer"
BANK = ("bank", None, "KBank", "123-4-56789-0", "Somchai")
PROMPTPAY = ("promptpay", "0812345678", None, None, None)


class CreditPayoutTests(CreditFixture):
    # -- helpers ---------------------------------------------------------

    def hold(self, email, amount, key, org="", currency="thb"):
        self.db.execute(
            """INSERT INTO credit_ledger
                   (email, organization_id, currency, delta, reason, deposit_id)
               VALUES (?, ?, ?, ?, 'hold', ?)""",
            (email, org, currency, amount, key),
        )

    def refund(self, email, thb, usdc=0, requested_at=REQUESTED_AT):
        """Mirror `credit_ledger::try_refund`: rows written, or the outcome."""
        written = self.db.execute(
            TRY_REFUND_SQL, (email, requested_at, thb, usdc, NOTE)
        ).rowcount
        if written:
            return ("recorded", written)
        prefix = f"refund:{email}:{requested_at}:"
        n = self.db.execute(REFUND_RECORDED_SQL, (prefix,)).fetchone()["n"]
        return ("already",) if n else ("mismatch",)

    def spend(self, email, amount, event_id, org=""):
        return self.db.execute(
            TRY_SPEND_SQL,
            (email, org, "thb", amount, event_id, f"apply:{event_id}:{email}"),
        ).rowcount

    def payable(self, email):
        return {
            (r["organization_id"], r["currency"]): r["balance"]
            for r in self.db.execute(POSITIVE_SQL, (email,))
        }

    def negatives(self):
        return self.db.execute(NEGATIVE_SQL).fetchone()["n"]

    def contact(self, email):
        self.db.execute(
            "INSERT INTO contacts (email, name) VALUES (?, 'P')", (email,)
        )

    def request(self, email, account=BANK):
        """Mirror `contacts::set_credit_refund_requested`'s batch."""
        with self.db:
            flagged = self.db.execute(SET_FLAG_SQL, (email,)).rowcount
            self.db.execute(SAVE_SQL, (email, *account))
        return flagged

    def accounts(self):
        return [
            tuple(r)
            for r in self.db.execute(
                "SELECT email, method, promptpay_id, bank_name, bank_account, "
                "account_name FROM credit_refund_accounts ORDER BY email"
            )
        ]

    def clear(self, email):
        with self.db:
            self.db.execute(CLEAR_FLAG_SQL, (email,))
            self.db.execute(DELETE_FOR_PERSON_SQL, (email,))

    # -- 1. stale amount -------------------------------------------------

    def test_a_stale_amount_writes_nothing(self):
        self.hold(GMAIL, 800, "e1:a")
        self.assertEqual(self.refund(GMAIL, 500), ("mismatch",))
        self.assertEqual(self.payable(GMAIL), {("", "thb"): 800})

    def test_the_confirmed_amount_reverses_the_whole_balance(self):
        self.hold(GMAIL, 800, "e1:a")
        self.assertEqual(self.refund(GMAIL, 800), ("recorded", 1))
        self.assertEqual(self.payable(GMAIL), {})
        row = self.db.execute(
            "SELECT delta, reason, deposit_id, note FROM credit_ledger WHERE reason='refund'"
        ).fetchone()
        # The key format predates this change, so an old reversal still dedups.
        self.assertEqual(
            tuple(row),
            (-800, "refund", f"refund:{GMAIL}:{REQUESTED_AT}::thb", NOTE),
        )

    def test_an_unconfirmed_usdc_bucket_blocks_the_payout(self):
        self.hold(GMAIL, 500, "e1:a")
        self.hold(GMAIL, 5, "e1:b", currency="usdc")
        self.assertEqual(self.refund(GMAIL, 500), ("mismatch",))
        self.assertEqual(self.refund(GMAIL, 500, usdc=5), ("recorded", 2))
        self.assertEqual(self.payable(GMAIL), {})

    def test_a_currency_that_cannot_be_confirmed_is_never_reversed(self):
        self.hold(GMAIL, 500, "e1:a")
        self.hold(GMAIL, 7, "e1:b", currency="eur")
        self.assertEqual(self.refund(GMAIL, 500), ("mismatch",))
        self.assertEqual(self.payable(GMAIL), {("", "eur"): 7, ("", "thb"): 500})

    # -- 2. races: the balance never goes negative -----------------------

    def test_spend_then_clear_refuses_the_clear(self):
        self.hold(GMAIL, 500, "e1:a")
        # The organizer saw ฿500; a registration spends it before the clear.
        self.assertEqual(self.spend(GMAIL, 500, "rtm7"), 1)
        self.assertEqual(self.refund(GMAIL, 500), ("mismatch",))
        self.assertEqual(self.balance(GMAIL), 0)
        self.assertEqual(self.negatives(), 0)

    def test_clear_then_spend_refuses_the_spend(self):
        self.hold(GMAIL, 500, "e1:a")
        self.assertEqual(self.refund(GMAIL, 500), ("recorded", 1))
        self.assertEqual(self.spend(GMAIL, 500, "rtm7"), 0)
        self.assertEqual(self.balance(GMAIL), 0)
        self.assertEqual(self.negatives(), 0)

    def test_partial_spend_then_clear_needs_the_new_amount(self):
        self.hold(GMAIL, 800, "e1:a")
        self.assertEqual(self.spend(GMAIL, 500, "rtm7"), 1)
        self.assertEqual(self.refund(GMAIL, 800), ("mismatch",))
        self.assertEqual(self.refund(GMAIL, 300), ("recorded", 1))
        self.assertEqual(self.balance(GMAIL), 0)
        self.assertEqual(self.negatives(), 0)

    # -- 3. double clear -------------------------------------------------

    def test_a_second_clear_is_idempotent(self):
        self.hold(GMAIL, 500, "e1:a")
        self.assertEqual(self.refund(GMAIL, 500), ("recorded", 1))
        self.assertEqual(self.refund(GMAIL, 500), ("already",))
        refunds = self.db.execute(
            "SELECT COUNT(*) AS n, SUM(delta) AS s FROM credit_ledger WHERE reason='refund'"
        ).fetchone()
        self.assertEqual((refunds["n"], refunds["s"]), (1, -500))

    def test_a_new_request_is_a_new_key(self):
        self.hold(GMAIL, 500, "e1:a")
        self.assertEqual(self.refund(GMAIL, 500), ("recorded", 1))
        self.hold(GMAIL, 300, "e2:a")
        self.assertEqual(
            self.refund(GMAIL, 300, requested_at="2026-10-05 10:00:00"), ("recorded", 1)
        )
        self.assertEqual(self.balance(GMAIL), 0)

    def test_an_email_with_wildcards_cannot_match_another_request(self):
        # `substr`, not LIKE: `_` in one email must not stand for a character
        # of another, or a mismatch would read as "already recorded".
        lookalike = "a_b@x.example"
        self.hold("axb@x.example", 500, "e1:x")
        self.assertEqual(self.refund("axb@x.example", 500), ("recorded", 1))
        self.hold(lookalike, 500, "e1:y")
        self.assertEqual(self.refund(lookalike, 400), ("mismatch",))

    # -- 4. isolation ----------------------------------------------------

    def test_two_people_are_isolated(self):
        self.hold(GMAIL, 500, "e1:a")
        self.hold(OTHER, 700, "e1:b")
        self.assertEqual(self.refund(GMAIL, 500), ("recorded", 1))
        self.assertEqual(self.payable(OTHER), {("", "thb"): 700})
        self.assertEqual(self.refund(OTHER, 500), ("mismatch",))

    def test_every_org_bucket_is_reversed_against_one_total(self):
        self.hold(GMAIL, 500, "e1:a", org="")
        self.hold(GMAIL, 300, "e1:b", org="org-b")
        self.assertEqual(self.refund(GMAIL, 500), ("mismatch",))
        self.assertEqual(self.refund(GMAIL, 800), ("recorded", 2))
        self.assertEqual(self.payable(GMAIL), {})

    def test_a_linked_person_is_paid_once(self):
        self.hold(WORK, 500, "e1:a")
        self.link(GMAIL, WORK)
        self.assertEqual(self.refund(GMAIL, 500), ("recorded", 1))
        self.assertEqual(self.payable(WORK), {})
        self.assertEqual(self.refund(WORK, 500), ("mismatch",))

    # -- 5. payout account -----------------------------------------------

    def test_the_account_is_saved_only_with_an_open_request(self):
        # No contact row: the flag matches nothing and neither does the save.
        self.assertEqual(self.request(GMAIL), 0)
        self.assertEqual(self.accounts(), [])
        self.contact(GMAIL)
        self.assertEqual(self.request(GMAIL), 1)
        self.assertEqual(self.accounts(), [(GMAIL, *BANK)])

    def test_a_re_request_replaces_the_account(self):
        self.contact(GMAIL)
        self.request(GMAIL, BANK)
        self.request(GMAIL, PROMPTPAY)
        self.assertEqual(self.accounts(), [(GMAIL, *PROMPTPAY)])

    def test_the_schema_refuses_a_mixed_or_unknown_account(self):
        self.contact(GMAIL)
        self.db.execute(SET_FLAG_SQL, (GMAIL,))
        for bad in [
            ("promptpay", "0812345678", "KBank", None, None),
            ("bank", None, "KBank", None, "Somchai"),
            ("cash", None, None, None, None),
        ]:
            with self.assertRaises(sqlite3.IntegrityError, msg=str(bad)):
                self.db.execute(SAVE_SQL, (GMAIL, *bad))

    def test_the_queue_shows_the_account_age_and_orgs(self):
        self.contact(GMAIL)
        self.hold(GMAIL, 500, "e1:a", org="org-a")
        self.request(GMAIL, PROMPTPAY)
        self.db.execute(
            "UPDATE contacts SET credit_refund_requested_at = datetime('now', '-8 days') "
            "WHERE email = ?",
            (GMAIL,),
        )
        row = self.db.execute(QUEUE_SQL).fetchone()
        self.assertEqual(row["account_method"], "promptpay")
        self.assertEqual(row["promptpay_id"], "0812345678")
        self.assertIsNone(row["bank_account"])
        self.assertIn(row["age_hours"], (191, 192))
        self.assertEqual(row["org_ids"], '["org-a"]')
        self.assertEqual(row["credit_thb"], 500)

    def test_a_request_without_an_account_still_queues(self):
        # Requests made before 0059 have no account row.
        self.db.execute(
            """INSERT INTO contacts (email, name, credit_refund_requested,
                   credit_refund_requested_at) VALUES (?, 'P', 1, ?)""",
            (GMAIL, REQUESTED_AT),
        )
        row = self.db.execute(QUEUE_SQL).fetchone()
        self.assertIsNone(row["account_method"])
        self.assertEqual(row["org_ids"], "[]")

    def test_the_payout_deletes_the_account(self):
        self.contact(GMAIL)
        self.request(GMAIL)
        self.clear(GMAIL)
        self.assertEqual(self.accounts(), [])

    def test_the_payout_deletes_a_linked_sibling_account(self):
        self.contact(GMAIL)
        self.contact(WORK)
        self.request(GMAIL)
        self.request(WORK, PROMPTPAY)
        self.link(GMAIL, WORK)
        self.clear(GMAIL)
        self.assertEqual(self.accounts(), [])

    def test_the_purge_deletes_old_and_closed_rows_only(self):
        for email in (GMAIL, WORK, OTHER):
            self.contact(email)
            self.request(email)
        # OTHER's request was cleared but its delete did not land.
        self.db.execute(
            "UPDATE contacts SET credit_refund_requested = 0 WHERE email = ?", (OTHER,)
        )
        # WORK's request is open but older than the retention.
        self.db.execute(
            "UPDATE credit_refund_accounts SET updated_at = datetime('now', '-91 days') "
            "WHERE email = ?",
            (WORK,),
        )
        deleted = self.db.execute(PURGE_SQL, ("-90 days",)).rowcount
        self.assertEqual(deleted, 2)
        self.assertEqual([a[0] for a in self.accounts()], [GMAIL])

    def test_pdpa_erasure_deletes_the_account(self):
        self.contact(GMAIL)
        self.request(GMAIL)
        self.db.execute(PII_ACCOUNT_SQL, (GMAIL.upper(),))
        self.assertEqual(self.accounts(), [])


class PiiErasureSqlIsTheSourceSql(unittest.TestCase):
    def test_the_erasure_statement_matches_contacts_rs(self):
        from test_person_emails import SRC

        source = (SRC / "db/contacts.rs").read_text()
        self.assertIn(f'.prepare("{PII_ACCOUNT_SQL}")', source)


if __name__ == "__main__":
    unittest.main()
