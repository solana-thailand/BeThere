"""Locked credit — applied to an event that has not ended (issue #120).

The spendable balance and the locked amount are two different numbers, and the
payout path needs both: `positive_balances` is what the organizer can hand back
today, `locked_applies` is what is committed to an upcoming event and comes
back when it ends. Clearing a request against the first while the second is
non-zero pays out nothing and silently drops the request, so these run the
worker's real SQL — extracted from the Rust sources, same harness as
`test_person_emails` — against the production migrations.
"""

import unittest

from test_person_emails import (
    GMAIL,
    OTHER,
    WORK,
    CreditFixture,
    concat_after,
    plain_after,
)

LOCKED_SQL = concat_after("db/credit_ledger.rs", "pub async fn locked_applies(")
POSITIVE_SQL = concat_after("db/credit_ledger.rs", "pub async fn positive_balances(")
QUEUE_SQL = concat_after("db/contacts.rs", "pub async fn credit_refund_requests(")
RELEASE_SQL = plain_after("db/credit_ledger.rs", "const RELEASE_ENDED_APPLIES_SQL")

PAST = 1_600_000_000_000
FUTURE = 4_100_000_000_000


class LockedCreditTests(CreditFixture):
    # -- helpers ---------------------------------------------------------

    def event(self, event_id, name, end_ms):
        self.db.execute(
            """INSERT INTO events (id, name, slug, event_start_ms, event_end_ms)
               VALUES (?, ?, ?, ?, ?)""",
            (event_id, name, event_id, end_ms - 1, end_ms),
        )

    def apply_credit(self, email, amount, event_id):
        self.ledger(
            email, -amount, "apply", event_id=event_id,
            deposit_id=f"apply:{event_id}:{email}",
        )

    def ret(self, email, amount, event_id):
        self.ledger(
            email, amount, "return", event_id=event_id,
            deposit_id=f"return:{event_id}:{email}",
        )

    def locked(self, email):
        return [
            (r["event_id"], r["event_name"], r["amount"], r["event_end_ms"])
            for r in self.db.execute(LOCKED_SQL, (email,)).fetchall()
        ]

    def spendable(self, email):
        rows = self.db.execute(POSITIVE_SQL, (email,)).fetchall()
        return sum(r["balance"] for r in rows)

    def queue(self):
        return [
            (r["email"], r["credit_thb"], r["locked_thb"], r["locked_until"])
            for r in self.db.execute(QUEUE_SQL).fetchall()
        ]

    # -- the balance / locked split --------------------------------------

    def test_credit_applied_to_an_upcoming_event_is_locked_not_spendable(self):
        self.event("rtm6", "RTM #6", FUTURE)
        self.ledger(GMAIL, 500, "hold", deposit_id="rtm5:a")
        self.apply_credit(GMAIL, 500, "rtm6")
        # The whole balance is committed: the payout reversal would write
        # nothing, which is exactly why the clear has to be refused.
        self.assertEqual(self.spendable(GMAIL), 0)
        self.assertEqual(self.locked(GMAIL), [("rtm6", "RTM #6", 500, FUTURE)])

    def test_returned_credit_is_spendable_again_and_no_longer_locked(self):
        self.event("rtm6", "RTM #6", FUTURE)
        self.ledger(GMAIL, 500, "hold", deposit_id="rtm5:a")
        self.apply_credit(GMAIL, 500, "rtm6")
        self.ret(GMAIL, 500, "rtm6")
        self.assertEqual(self.locked(GMAIL), [])
        self.assertEqual(self.spendable(GMAIL), 500)

    def test_an_ended_event_releases_the_lock(self):
        self.event("rtm5", "RTM #5", PAST)
        self.ledger(GMAIL, 500, "hold", deposit_id="rtm4:a")
        self.apply_credit(GMAIL, 500, "rtm5")
        # `locked_applies` runs this first; without it an ended event's credit
        # reads as locked and the organizer can never clear the request.
        self.db.execute(RELEASE_SQL)
        self.assertEqual(self.locked(GMAIL), [])
        self.assertEqual(self.spendable(GMAIL), 500)

    def test_an_event_with_no_row_stays_locked(self):
        # An unknown end is not a past end (same rule as the release SQL), and
        # the breakdown still names the event by id so the organizer can look.
        self.ledger(GMAIL, 500, "hold", deposit_id="rtm5:a")
        self.apply_credit(GMAIL, 500, "ghost")
        self.db.execute(RELEASE_SQL)
        self.assertEqual(self.locked(GMAIL), [("ghost", "", 500, 0)])

    def test_partial_apply_leaves_the_rest_spendable(self):
        self.event("rtm6", "RTM #6", FUTURE)
        self.ledger(GMAIL, 800, "hold", deposit_id="rtm5:a")
        self.apply_credit(GMAIL, 500, "rtm6")
        self.assertEqual(self.spendable(GMAIL), 300)
        self.assertEqual(self.locked(GMAIL), [("rtm6", "RTM #6", 500, FUTURE)])

    def test_two_locked_events_list_the_latest_ending_one_first(self):
        self.event("near", "Near", FUTURE)
        self.event("far", "Far", FUTURE + 1000)
        self.ledger(GMAIL, 1000, "hold", deposit_id="rtm5:a")
        self.apply_credit(GMAIL, 400, "near")
        self.apply_credit(GMAIL, 600, "far")
        # The guard names the first row — the last event to give the money back.
        self.assertEqual(
            self.locked(GMAIL),
            [("far", "Far", 600, FUTURE + 1000), ("near", "Near", 400, FUTURE)],
        )

    # -- person scope ----------------------------------------------------

    def test_a_linked_email_sees_the_person_whole_lock(self):
        self.event("rtm6", "RTM #6", FUTURE)
        self.ledger(WORK, 500, "hold", deposit_id="rtm5:b")
        self.apply_credit(WORK, 500, "rtm6")
        self.link(GMAIL, WORK)
        self.assertEqual(self.locked(GMAIL), [("rtm6", "RTM #6", 500, FUTURE)])

    def test_a_stranger_lock_is_never_counted(self):
        self.event("rtm6", "RTM #6", FUTURE)
        self.ledger(OTHER, 500, "hold", deposit_id="rtm5:c")
        self.apply_credit(OTHER, 500, "rtm6")
        self.assertEqual(self.locked(GMAIL), [])

    def test_one_email_return_does_not_release_the_other(self):
        # `return` is keyed per (event, email), so a linked sibling's lock
        # survives — the breakdown must not collapse them.
        self.event("rtm6", "RTM #6", FUTURE)
        self.ledger(GMAIL, 500, "hold", deposit_id="rtm5:a")
        self.ledger(WORK, 500, "hold", deposit_id="rtm5:b")
        self.apply_credit(GMAIL, 500, "rtm6")
        self.apply_credit(WORK, 500, "rtm6")
        self.ret(GMAIL, 500, "rtm6")
        self.link(GMAIL, WORK)
        self.assertEqual(self.locked(GMAIL), [("rtm6", "RTM #6", 500, FUTURE)])

    # -- the organizer payout queue --------------------------------------

    def test_queue_shows_zero_payable_and_the_locked_amount(self):
        self.event("rtm6", "RTM #6", FUTURE)
        self.ledger(GMAIL, 500, "hold", deposit_id="rtm5:a")
        self.apply_credit(GMAIL, 500, "rtm6")
        self.request_refund(GMAIL, "2026-09-19T00:00:00Z")
        self.assertEqual(self.queue(), [(GMAIL, 0, 500, "RTM #6")])

    def test_queue_locked_is_zero_when_nothing_is_committed(self):
        self.ledger(GMAIL, 500, "hold", deposit_id="rtm5:a")
        self.request_refund(GMAIL, "2026-09-19T00:00:00Z")
        self.assertEqual(self.queue(), [(GMAIL, 500, 0, "")])

    def test_queue_locked_is_person_wide_and_shown_once(self):
        self.event("rtm6", "RTM #6", FUTURE)
        self.ledger(GMAIL, 500, "hold", deposit_id="rtm5:a")
        self.ledger(WORK, 500, "hold", deposit_id="rtm5:b")
        self.apply_credit(GMAIL, 500, "rtm6")
        self.apply_credit(WORK, 500, "rtm6")
        self.link(GMAIL, WORK)
        self.request_refund(GMAIL, "2026-09-19T00:00:00Z")
        self.request_refund(WORK, "2026-09-19T01:00:00Z")
        # One row, ฿1000 locked — two rows here would have the organizer wait
        # on (and later pay) the same money twice.
        self.assertEqual(self.queue(), [(WORK, 0, 1000, "RTM #6")])


if __name__ == "__main__":
    unittest.main()
