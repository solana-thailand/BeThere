"""Credit payout hardening (`.issues/190`).

Runs the worker's real SQL — extracted from the Rust sources with the
`test_person_emails` harness — against the full migration chain (0001 … 0059):

- the payout reversal is ONE guarded statement: it writes only when the
  payable balance still equals what the organizer confirmed, so a stale amount
  writes nothing, a spend and a clear can race in either order without the
  balance going negative, and a second clear is a no-op;
- the payout account typed on the card is stored only while a request is
  open; the one already given with the deposit is copied at hold time (and
  backfilled by 0059) from the event-scoped deposit, never over the
  attendee's own choice; the queue and the masked preview pick the same row;
  it is deleted at payout, by PDPA erasure, and by the nightly purge once the
  person has no open request and no held (payable or locked) credit.
"""

import re
import sqlite3
import unittest

from test_person_emails import (
    GMAIL,
    OTHER,
    WORK,
    CreditFixture,
    WORKER,
    concat_after,
    plain_after,
)

TRY_REFUND_SQL = concat_after("db/credit_ledger.rs", "pub(crate) const TRY_REFUND_SQL")
REFUND_RECORDED_SQL = plain_after("db/credit_ledger.rs", "pub(crate) const REFUND_RECORDED_SQL")
POSITIVE_SQL = concat_after("db/credit_ledger.rs", "pub async fn positive_balances(")
TRY_SPEND_SQL = concat_after("db/credit_ledger.rs", "pub async fn try_spend(")
NEGATIVE_SQL = plain_after("db/credit_ledger.rs", "let negative_balances = count_query(")
SET_FLAG_SQL = plain_after("db/contacts.rs", "pub(crate) async fn set_credit_refund_requested(")
SAVE_SQL = concat_after("db/credit_refund_accounts.rs", "pub(crate) const SAVE_SQL")
SNAPSHOT_SQL = plain_after(
    "db/credit_refund_accounts.rs", "pub(crate) const SNAPSHOT_FROM_DEPOSIT_SQL"
)
CHOSEN_SQL = concat_after(
    "db/credit_refund_accounts.rs", "pub(crate) const CHOSEN_FOR_PERSON_SQL"
)
SET_FLAG_SAVED_SQL = concat_after(
    "db/contacts.rs", "pub(crate) const SET_FLAG_WITH_SAVED_ACCOUNT_SQL"
)
DELETE_FOR_PERSON_SQL = concat_after(
    "db/credit_refund_accounts.rs", "pub(crate) const DELETE_FOR_PERSON_SQL"
)
PURGE_SQL = concat_after("db/credit_refund_accounts.rs", "pub(crate) const PURGE_SQL")
HOLDERS_SQL = concat_after(
    "db/credit_refund_accounts.rs", "pub(crate) const DEPOSIT_ACCOUNT_HOLDERS_SQL"
)
OPEN_REQUEST_SQL = concat_after(
    "db/credit_refund_accounts.rs", "pub(crate) const OPEN_REQUEST_FOR_PERSON_SQL"
)


def backfill_sql():
    """The 0059 backfill statement, as the migration runs it."""
    text = (WORKER / "migrations/0059_credit_refund_payout.sql").read_text()
    code = "\n".join(re.sub(r"--.*$", "", line) for line in text.splitlines())
    start = code.index("INSERT INTO credit_refund_accounts")
    return code[start : code.index(";", start) + 1]


BACKFILL_SQL = backfill_sql()
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
DEPOSIT_BANK = ("Kasikornbank (KBANK)", "123-4-56789-0", "Somchai Jaidee")
STORED_DEPOSIT_BANK = ("bank", None, *DEPOSIT_BANK)


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

    def purge(self):
        return self.db.execute(PURGE_SQL).rowcount

    def attendee(self, attendee_id, event_id, email):
        self.db.execute(
            "INSERT INTO attendees (id, event_id, email, name) VALUES (?, ?, ?, 'P')",
            (attendee_id, event_id, email),
        )

    def deposit(
        self,
        event_id,
        attendee_id,
        email=None,
        bank=DEPOSIT_BANK,
        held_at="2026-09-01T10:00:00+00:00",
        uploaded_at="2026-08-30T09:00:00+00:00",
        held=1,
        refunded=0,
    ):
        """A verified THB deposit with the refund account the slip form takes,
        and (with `email`) its attendee row."""
        if email is not None:
            self.attendee(attendee_id, event_id, email)
        bank_name, bank_account, account_name = bank
        self.db.execute(
            """INSERT INTO thb_deposits (attendee_id, event_id, amount_thb, verified,
                   uploaded_at, refunded, held_as_credit, held_as_credit_at,
                   bank_name, bank_account, account_name)
               VALUES (?, ?, 500, 1, ?, ?, ?, ?, ?, ?, ?)""",
            (attendee_id, event_id, uploaded_at, refunded, held, held_at,
             bank_name, bank_account, account_name),
        )

    def snapshot(self, email, event_id, attendee_id):
        """Mirror `credit_refund_accounts::snapshot_from_deposit`."""
        return self.db.execute(SNAPSHOT_SQL, (email, event_id, attendee_id)).rowcount

    def backfill(self):
        return self.db.execute(BACKFILL_SQL).rowcount

    def row(self, email):
        return self.db.execute(
            "SELECT * FROM credit_refund_accounts WHERE email = ?", (email,)
        ).fetchone()

    def chosen(self, email):
        return self.db.execute(CHOSEN_SQL, (email,)).fetchone()

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

    def test_the_purge_keeps_an_account_while_credit_is_held(self):
        for email in (GMAIL, WORK, OTHER):
            self.contact(email)
            self.request(email)
            self.db.execute(
                "UPDATE contacts SET credit_refund_requested = 0 WHERE email = ?", (email,)
            )
        self.hold(GMAIL, 500, "e1:a")  # payable
        self.hold(WORK, 500, "e1:b")
        self.assertEqual(self.spend(WORK, 500, "e2"), 1)  # all of it locked to e2
        self.assertEqual(self.purge(), 1)
        self.assertEqual([a[0] for a in self.accounts()], [GMAIL, WORK])

    def test_the_purge_deletes_at_zero_with_no_open_request(self):
        self.contact(GMAIL)
        self.request(GMAIL)
        self.hold(GMAIL, 500, "e1:a")
        self.assertEqual(self.purge(), 0, "open request and credit")
        self.db.execute(
            "UPDATE contacts SET credit_refund_requested = 0 WHERE email = ?", (GMAIL,)
        )
        self.assertEqual(self.purge(), 0, "credit still held")
        self.assertEqual(self.spend(GMAIL, 500, "e2"), 1)
        self.assertEqual(self.purge(), 0, "credit locked to an event")
        self.db.execute(
            """INSERT INTO credit_ledger (email, organization_id, currency, delta, reason,
                   event_id, deposit_id) VALUES (?, '', 'thb', 0, 'return', 'e2', 'r')""",
            (GMAIL,),
        )
        self.assertEqual(self.purge(), 1, "spent: balance 0, nothing locked")
        self.assertEqual(self.accounts(), [])

    def test_an_open_request_keeps_the_account_at_zero_balance(self):
        self.contact(GMAIL)
        self.request(GMAIL)
        self.assertEqual(self.purge(), 0)
        self.assertEqual(len(self.accounts()), 1)

    def test_the_purge_reads_the_whole_person(self):
        # The account sits on one email, the credit and the request on another.
        self.contact(WORK)
        self.deposit("e1", "a1", GMAIL)
        self.snapshot(GMAIL, "e1", "a1")
        self.link(GMAIL, WORK)
        self.hold(WORK, 500, "e1:x")
        self.assertEqual(self.purge(), 0)
        self.hold(WORK, -500, "e1:y")
        self.db.execute(SET_FLAG_SQL, (WORK,))
        self.assertEqual(self.purge(), 0, "the sibling's open request keeps it")

    def test_pdpa_erasure_deletes_the_account(self):
        self.contact(GMAIL)
        self.request(GMAIL)
        self.db.execute(PII_ACCOUNT_SQL, (GMAIL.upper(),))
        self.assertEqual(self.accounts(), [])


    # -- 6. the account from the deposit -----------------------------------

    def test_the_hold_copies_the_deposit_account(self):
        self.deposit("e1", "a1", GMAIL)
        self.assertEqual(self.snapshot(GMAIL, "e1", "a1"), 1)
        self.assertEqual(self.accounts(), [(GMAIL, *STORED_DEPOSIT_BANK)])
        row = self.row(GMAIL)
        self.assertEqual(row["source"], "deposit")
        self.assertEqual(row["source_deposit_ref"], "e1:a1")
        self.assertEqual(row["captured_at"], "2026-08-30T09:00:00+00:00")
        self.assertEqual(row["replaced_deposit_account"], 0)

    def test_the_hold_reads_the_deposit_of_that_event_only(self):
        # Same attendee id, another event: not this deposit (ids are global,
        # the deposit is scoped by event).
        self.deposit("e2", "a1", bank=("SCB", "999-9-99999-9", "Other Person"))
        self.assertEqual(self.snapshot(GMAIL, "e1", "a1"), 0)
        self.deposit("e1", "a1")
        self.snapshot(GMAIL, "e1", "a1")
        self.assertEqual(self.accounts(), [(GMAIL, *STORED_DEPOSIT_BANK)])

    def test_only_a_held_unrefunded_deposit_is_copied(self):
        self.deposit("e1", "a1", held=0)
        self.deposit("e2", "a2", held=1, refunded=1)
        self.assertEqual(self.snapshot(GMAIL, "e1", "a1"), 0)
        self.assertEqual(self.snapshot(GMAIL, "e2", "a2"), 0)
        self.assertEqual(self.accounts(), [])

    def test_incomplete_deposit_accounts_are_skipped(self):
        for n, bank in enumerate(
            [("", "123", "S"), ("KBank", "   ", "S"), ("KBank", "123", ""), (None, None, None)]
        ):
            self.deposit("e1", f"a{n}", bank=bank)
            self.assertEqual(self.snapshot(GMAIL, "e1", f"a{n}"), 0, bank)
        self.assertEqual(self.accounts(), [])

    def test_a_promptpay_number_in_the_bank_fields_becomes_promptpay(self):
        self.deposit("e1", "a1", bank=(" Prompt Pay ", "081-234-5678", "Somchai"))
        self.snapshot(GMAIL, "e1", "a1")
        self.assertEqual(self.accounts(), [(GMAIL, *PROMPTPAY)])
        self.deposit("e2", "a2", bank=("พร้อมเพย์", "1 2345 67890 12 3", "S"))
        self.snapshot(WORK, "e2", "a2")
        self.assertEqual(self.row(WORK)["promptpay_id"], "1234567890123")
        # Not a valid PromptPay ID: no account rather than a wrong one.
        self.deposit("e3", "a3", bank=("PromptPay", "12345", "S"))
        self.assertEqual(self.snapshot(OTHER, "e3", "a3"), 0)

    def test_a_newer_deposit_replaces_an_older_deposit_account(self):
        self.deposit("e1", "a1")
        self.snapshot(GMAIL, "e1", "a1")
        self.deposit("e2", "a2", bank=("SCB", "222-2-22222-2", "Somchai J"),
                     uploaded_at="2026-09-20T09:00:00+00:00")
        self.assertEqual(self.snapshot(GMAIL, "e2", "a2"), 1)
        row = self.row(GMAIL)
        self.assertEqual((row["bank_name"], row["source_deposit_ref"]), ("SCB", "e2:a2"))
        self.assertEqual(row["captured_at"], "2026-09-20T09:00:00+00:00")

    def test_a_later_hold_never_overwrites_the_attendees_choice(self):
        self.contact(GMAIL)
        self.request(GMAIL, PROMPTPAY)
        self.deposit("e1", "a1")
        self.assertEqual(self.snapshot(GMAIL, "e1", "a1"), 0)
        self.assertEqual(self.accounts(), [(GMAIL, *PROMPTPAY)])
        self.assertEqual(self.row(GMAIL)["source"], "attendee")

    def test_a_different_account_marks_the_deposit_account_replaced(self):
        self.contact(GMAIL)
        self.deposit("e1", "a1")
        self.snapshot(GMAIL, "e1", "a1")
        self.request(GMAIL, PROMPTPAY)
        row = self.row(GMAIL)
        self.assertEqual(
            (row["source"], row["replaced_deposit_account"], row["source_deposit_ref"]),
            ("attendee", 1, None),
        )
        # It stays flagged through a later re-request.
        self.request(GMAIL, BANK)
        self.assertEqual(self.row(GMAIL)["replaced_deposit_account"], 1)

    def test_the_same_account_retyped_is_not_a_replacement(self):
        self.contact(GMAIL)
        self.deposit("e1", "a1")
        self.snapshot(GMAIL, "e1", "a1")
        self.request(GMAIL, STORED_DEPOSIT_BANK)
        row = self.row(GMAIL)
        self.assertEqual((row["source"], row["replaced_deposit_account"]), ("attendee", 0))

    def test_an_attendee_account_with_no_deposit_is_not_a_replacement(self):
        self.contact(GMAIL)
        self.request(GMAIL, BANK)
        self.assertEqual(self.row(GMAIL)["replaced_deposit_account"], 0)

    def test_a_replacement_on_a_linked_email_is_flagged(self):
        self.contact(WORK)
        self.deposit("e1", "a1")
        self.snapshot(GMAIL, "e1", "a1")
        self.link(GMAIL, WORK)
        self.request(WORK, PROMPTPAY)
        self.assertEqual(self.row(WORK)["replaced_deposit_account"], 1)
        # Both read the attendee's choice.
        self.assertEqual(self.chosen(GMAIL)["source"], "attendee")
        self.assertEqual(self.chosen(WORK)["promptpay_id"], "0812345678")

    def test_one_tap_needs_an_account_on_file(self):
        self.contact(GMAIL)
        self.assertEqual(self.db.execute(SET_FLAG_SAVED_SQL, (GMAIL,)).rowcount, 0)
        self.deposit("e1", "a1")
        self.snapshot(GMAIL, "e1", "a1")
        self.assertEqual(self.db.execute(SET_FLAG_SAVED_SQL, (GMAIL,)).rowcount, 1)
        row = self.db.execute(QUEUE_SQL).fetchone()
        self.assertEqual(row["account_method"], "bank")
        self.assertEqual(row["bank_account"], "123-4-56789-0", "staff see the full number")
        self.assertEqual(row["account_source_raw"], "deposit")
        self.assertEqual(row["account_captured_at"], "2026-08-30T09:00:00+00:00")
        self.assertEqual(row["account_replaced_deposit"], 0)

    def test_the_queue_shows_a_replacement(self):
        self.contact(GMAIL)
        self.deposit("e1", "a1")
        self.snapshot(GMAIL, "e1", "a1")
        self.request(GMAIL, PROMPTPAY)
        row = self.db.execute(QUEUE_SQL).fetchone()
        self.assertEqual(row["account_source_raw"], "attendee")
        self.assertEqual(row["account_replaced_deposit"], 1)
        self.assertEqual(row["promptpay_id"], "0812345678")

    def test_the_payout_deletes_the_deposit_account_too(self):
        self.contact(WORK)
        self.deposit("e1", "a1")
        self.snapshot(GMAIL, "e1", "a1")
        self.link(GMAIL, WORK)
        self.db.execute(SET_FLAG_SAVED_SQL, (WORK,))
        self.clear(WORK)
        self.assertEqual(self.accounts(), [])

    # -- 7. the 0059 backfill ----------------------------------------------

    def test_the_backfill_takes_the_latest_held_deposit(self):
        self.deposit("e1", "a1", GMAIL, held_at="2026-09-01T10:00:00+00:00")
        self.deposit("e2", "a2", GMAIL, bank=("SCB", "222-2-22222-2", "Somchai J"),
                     held_at="2026-09-20T10:00:00+00:00")
        # The latest held deposit with an incomplete account does not count.
        self.deposit("e3", "a3", GMAIL, bank=("KTB", "", ""),
                     held_at="2026-09-30T10:00:00+00:00")
        self.hold(GMAIL, 1500, "e1:a1")
        self.assertEqual(self.backfill(), 1)
        row = self.row(GMAIL)
        self.assertEqual((row["bank_name"], row["source"], row["source_deposit_ref"]),
                         ("SCB", "deposit", "e2:a2"))

    def test_the_backfill_matches_the_attendee_on_id_and_event(self):
        # a1 is GMAIL's id in e1. A deposit for a1 in e2 is someone else's
        # (no attendee row there): it must not land on GMAIL.
        self.attendee("a1", "e1", GMAIL)
        self.deposit("e2", "a1", bank=("SCB", "999-9-99999-9", "Other Person"),
                     held_at="2026-09-30T10:00:00+00:00")
        self.deposit("e2", "b1", WORK, held_at="2026-09-02T10:00:00+00:00")
        self.hold(GMAIL, 500, "x")
        self.hold(WORK, 500, "y")
        self.assertEqual(self.backfill(), 1)
        self.assertEqual([a[0] for a in self.accounts()], [WORK])
        self.assertEqual(self.row(WORK)["source_deposit_ref"], "e2:b1")

    def test_the_backfill_skips_people_without_credit(self):
        self.deposit("e1", "a1", GMAIL)
        self.deposit("e1", "a2", WORK)
        self.deposit("e1", "a3", OTHER)
        self.hold(GMAIL, 500, "e1:a1")
        self.hold(WORK, 500, "e1:a2")
        self.assertEqual(self.spend(WORK, 500, "e2"), 1)  # locked, still theirs
        self.hold(OTHER, 500, "e1:a3")
        self.hold(OTHER, -500, "refund:o")  # paid out
        self.assertEqual(self.backfill(), 2)
        self.assertEqual([a[0] for a in self.accounts()], [GMAIL, WORK])
        self.assertEqual(self.purge(), 0, "the purge keeps what the backfill wrote")

    def test_the_backfill_is_idempotent_and_keeps_the_attendees_choice(self):
        self.contact(GMAIL)
        self.request(GMAIL, PROMPTPAY)
        self.deposit("e1", "a1", GMAIL)
        self.deposit("e1", "a2", WORK.upper())
        self.hold(GMAIL, 500, "e1:a1")
        self.hold(WORK, 500, "e1:a2")
        self.assertEqual(self.backfill(), 1)
        self.assertEqual(self.backfill(), 0)
        self.assertEqual(self.accounts(), [(GMAIL, *PROMPTPAY), (WORK, *STORED_DEPOSIT_BANK)])

    def test_the_backfill_and_the_hold_map_deposits_alike(self):
        cases = [
            DEPOSIT_BANK,
            ("  KBank ", " 123 ", " Somchai "),
            ("PromptPay", "081-234-5678", "S"),
            ("prompt pay", "0812345678901", "S"),  # 13 digits
            ("พร้อมเพย์", "1234567890123", "S"),
            ("PromptPay", "1812345678", "S"),  # 10 digits not from 0
            ("PromptPay", "08x2345678", "S"),
            ("PromptPay", "", "S"),
            ("", "123", "S"),
            ("KBank", "123", "   "),
            (None, None, None),
        ]
        for n, bank in enumerate(cases):
            email = f"p{n}@x.example"
            self.deposit(f"e{n}", f"a{n}", email, bank=bank)
            self.hold(email, 500, f"e{n}:a{n}")
        self.backfill()
        from_backfill = self.accounts()
        self.db.execute("DELETE FROM credit_refund_accounts")
        for n, _ in enumerate(cases):
            self.snapshot(f"p{n}@x.example", f"e{n}", f"a{n}")
        self.assertEqual(self.accounts(), from_backfill)
        self.assertEqual(len(from_backfill), 5, from_backfill)

    # -- organizer-initiated payout (.issues/192) --------------------------

    def holders(self):
        return [dict(r) for r in self.db.execute(HOLDERS_SQL)]

    def open_request(self, email):
        return self.db.execute(OPEN_REQUEST_SQL, (email,)).fetchone()["n"] > 0

    def held_deposit(self, email, event_id="e1", attendee_id="a1", amount=500, org=""):
        """A held THB deposit whose account was copied at hold time."""
        self.contact(email)
        self.deposit(event_id, attendee_id, email)
        self.hold(email, amount, f"{event_id}:{attendee_id}", org=org)
        self.snapshot(email, event_id, attendee_id)

    def test_a_deposit_account_holder_is_a_candidate(self):
        self.held_deposit(GMAIL, org="org-a")
        [row] = self.holders()
        self.assertEqual(row["email"], GMAIL)
        self.assertEqual(row["credit_thb"], 500)
        self.assertEqual(row["org_ids"], '["org-a"]')
        self.assertEqual(row["account_source_raw"], "deposit")
        self.assertEqual(
            (row["bank_name"], row["bank_account"], row["account_name"]), DEPOSIT_BANK
        )

    def test_an_open_request_is_paid_from_the_queue_not_here(self):
        self.held_deposit(GMAIL)
        self.contact(WORK)
        self.link(GMAIL, WORK)
        self.request(WORK, PROMPTPAY)
        self.assertTrue(self.open_request(GMAIL), "person-wide, not per email")
        self.assertEqual(self.holders(), [])

    def test_an_account_the_attendee_typed_is_never_a_candidate(self):
        self.held_deposit(GMAIL)
        self.request(GMAIL, PROMPTPAY)
        # The flag closes without a payout (a stale row the organizer dismissed
        # before 0059) — the typed account must still not be paid unasked.
        self.db.execute("UPDATE contacts SET credit_refund_requested = 0")
        self.assertFalse(self.open_request(GMAIL))
        self.assertEqual(self.holders(), [])

    def test_no_payable_credit_is_no_candidate(self):
        self.held_deposit(GMAIL)
        self.assertEqual(self.spend(GMAIL, 500, "e2"), 1)
        self.assertEqual(self.holders(), [])

    def test_linked_emails_are_one_candidate_with_the_person_total(self):
        self.held_deposit(GMAIL)
        self.held_deposit(WORK, "e2", "a2", amount=300)
        self.link(GMAIL, WORK)
        [row] = self.holders()
        self.assertEqual(row["credit_thb"], 800)

    def test_an_organizer_payout_pays_once(self):
        self.held_deposit(GMAIL)
        self.assertEqual(self.refund(GMAIL, 500, requested_at="organizer-1"), ("recorded", 1))
        # A double click is a new key against a zero balance: refused.
        self.assertEqual(self.refund(GMAIL, 500, requested_at="organizer-2"), ("mismatch",))
        self.assertEqual(self.payable(GMAIL), {})
        self.assertEqual(self.holders(), [])


class PiiErasureSqlIsTheSourceSql(unittest.TestCase):
    def test_the_erasure_statement_matches_contacts_rs(self):
        from test_person_emails import SRC

        source = (SRC / "db/contacts.rs").read_text()
        self.assertIn(f'.prepare("{PII_ACCOUNT_SQL}")', source)


if __name__ == "__main__":
    unittest.main()
