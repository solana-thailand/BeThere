"""The attendee's "paid back" receipt (`.issues/192`).

Runs `credit_payout_receipt::SETTLED_PAYOUT_SQL`, extracted from the Rust
source, against the full migration chain. The card shows the receipt in place
of the request button, so the read must return a payout only while it
settled everything: no credit after it, none locked to an event.
"""

import re
import unittest

from test_person_emails import GMAIL, OTHER, WORK, CreditFixture, concat_after

RECEIPT_SQL = concat_after(
    "db/credit_payout_receipt.rs", "pub(crate) const SETTLED_PAYOUT_SQL"
)
TRY_REFUND_SQL = concat_after("db/credit_ledger.rs", "pub(crate) const TRY_REFUND_SQL")
NOTE = "held-credit payout processed by organizer"
ISO_UTC = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")


class CreditPayoutReceiptTests(CreditFixture):
    def hold(self, email, amount, key, currency="thb", org=""):
        self.db.execute(
            """INSERT INTO credit_ledger
                   (email, organization_id, currency, delta, reason, deposit_id)
               VALUES (?, ?, ?, ?, 'hold', ?)""",
            (email, org, currency, amount, key),
        )

    def pay_out(self, email, thb, usdc=0, key="2026-10-09T08:00:00.000Z"):
        """Mirror `credit_ledger::try_refund`; asserts the reversal wrote."""
        written = self.db.execute(
            TRY_REFUND_SQL, (email, key, thb, usdc, NOTE)
        ).rowcount
        self.assertGreater(written, 0, "the payout reversal wrote nothing")

    def receipt(self, email):
        row = self.db.execute(RECEIPT_SQL, (email,)).fetchone()
        return None if row is None else (row["thb"], row["usdc"])

    def test_no_payout_is_no_receipt(self):
        self.hold(GMAIL, 500, "e1:a")
        self.assertIsNone(self.receipt(GMAIL))
        self.assertIsNone(self.receipt(OTHER))

    def test_a_settling_payout_is_the_receipt(self):
        self.hold(GMAIL, 300, "e1:a")
        self.hold(GMAIL, 200, "e2:a")
        self.pay_out(GMAIL, 500)
        self.assertEqual(self.receipt(GMAIL), (500, 0))
        paid_at = self.db.execute(RECEIPT_SQL, (GMAIL,)).fetchone()["paid_at"]
        self.assertRegex(paid_at, ISO_UTC)

    def test_both_currencies_and_orgs_add_up(self):
        self.hold(GMAIL, 500, "e1:a", org="org-a")
        self.hold(GMAIL, 200, "e2:a", org="org-b")
        self.hold(GMAIL, 7, "e3:a", currency="usdc")
        self.pay_out(GMAIL, 700, usdc=7)
        self.assertEqual(self.receipt(GMAIL), (700, 7))

    def test_a_linked_email_sees_the_persons_payout(self):
        self.link(GMAIL, WORK)
        self.hold(WORK, 500, "e1:w")
        self.pay_out(GMAIL, 500)
        self.assertEqual(self.receipt(GMAIL), (500, 0))
        self.assertEqual(self.receipt(WORK), (500, 0))
        self.assertIsNone(self.receipt(OTHER))

    def test_credit_after_the_payout_hides_it(self):
        self.hold(GMAIL, 500, "e1:a")
        self.pay_out(GMAIL, 500)
        self.hold(GMAIL, 300, "e2:a")
        self.assertIsNone(self.receipt(GMAIL))

    def test_locked_credit_hides_it(self):
        self.hold(GMAIL, 1000, "e1:a")
        self.ledger(GMAIL, -500, "apply", event_id="e2", deposit_id="apply:e2:a")
        self.pay_out(GMAIL, 500)
        self.assertIsNone(self.receipt(GMAIL))
        # The event ends and returns the lock: that is credit again, not settled.
        self.ledger(GMAIL, 500, "return", event_id="e2", deposit_id="return:e2:a")
        self.assertIsNone(self.receipt(GMAIL))

    def test_only_the_latest_payout_counts(self):
        self.hold(GMAIL, 500, "e1:a")
        self.pay_out(GMAIL, 500, key="2026-10-01T08:00:00.000Z")
        self.hold(GMAIL, 300, "e2:a")
        self.pay_out(GMAIL, 300, key="2026-10-09T08:00:00.000Z")
        # Same second in this test: the read must not merge the two payouts.
        self.assertEqual(self.receipt(GMAIL), (300, 0))


if __name__ == "__main__":
    unittest.main()
