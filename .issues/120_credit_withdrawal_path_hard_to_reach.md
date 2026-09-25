# 120 — The "return my credit" path is hard to reach for most credit holders

**Status:** §1–§3 built 2026-09-19, deployed (in prod tag `deploy/production/20260925T032335Z`; `issue_ledger.py` 2026-09-25 found every linked commit there) · §4 still open
**Found:** 2026-09-17, owner question ("anyone with credit who wants to withdraw it instead of using it next time needs a way")
**Severity:** Medium (attendee money; a way out exists but most holders can't find it)

## What exists today

End to end, the withdrawal flow works:

1. **Attendee:** the `RequestCreditRefundCard` → `POST /api/deposit/request-credit-refund`
   sets `contacts.credit_refund_requested` (cross-event, on the person, not the event).
2. **Organizer:** the Deposits page's payout queue (`credit_refund_requests`) lists
   each request with the ledger amount (now released first, #118).
3. **Organizer:** transfers the cash, then clears the request. The reversal
   (`positive_balances`) writes a `refund` entry for every positive bucket, fails
   closed, and only then clears the flag.

## Why it's hard to reach

- The card renders only on the ticket page of **the event where the deposit was
  held** (`in_person_view.rs`: `if dep.held_as_credit`). A holder whose credit
  has since been applied to a later event sees no card on that later ticket. They
  would have to find the old event's ticket. Today that's every holder who
  rolled forward (Adeelan, Pakawat, Suprakon, Thanaphon, moo, Chanthawat,
  PARIPOL, ปอนด์ บางนา).
- The balance chip only mounts on a **checked-in** in-person ticket.
- **Credit locked in an upcoming event:** a request made while it's locked
  queues at ฿0. If the organizer clears it, nothing is reversed, and when the
  event ends the credit comes back with no open request. There's also no
  self-service way to cancel a registration and free the lock.

## Proposal

1. Show the balance, and the request button when the balance is > 0, wherever
   the person is signed in rather than per event: **My Registrations** and the
   profile. Reuse `CreditBalanceChip` + `RequestCreditRefundCard`; the backend is
   already cross-event.
2. Show locked credit separately: "฿500 is covering RTM #6; it returns when
   that event ends." Readable from `apply` rows without a `return`.
3. Payout queue: don't let the organizer clear a request whose amount is ฿0 while
   credit is still locked. Show "locked until <event>" and keep the flag open.
4. Optional: let a credit registrant cancel before the event, writing the
   `return` immediately (same key), so withdrawing doesn't mean waiting for the
   event to end.

Until this ships, the organizer can handle a verbal request directly: transfer
the cash, then clear the request from the queue (or ask the attendee to press
the card on their original event's ticket).

## Built (2026-09-19, NOT deployed)

**The missing read: locked credit.** `credit_ledger::locked_applies` returns the
events still holding a person's credit — an `apply` with no `return` yet — with
the amount and the event's end. It is the other half of `positive_balances`:
that answers "what can be paid back now", this answers "what is committed and
when does it come back". Both are person-scoped (plan 025) and both run
`release_ended_applies` first, so an ended event never reads as locked.

One SQL predicate, `unreturned_apply_of!`, is shared by all three readers (the
attendee breakdown, the queue's `locked_thb`, the queue's `locked_until`) — a
drifted copy would either strand a request or clear one against credit that is
about to come back.

- **§1 — reachable.** `CreditWallet` (balance + locked lines + the request card)
  now renders on **My Registrations**, for anyone signed in, independent of any
  event. The backend was already cross-event, so no new endpoint. The ticket
  page keeps its chip, which now also lists the locks.
- **§2 — locked shown separately.** `GET /api/deposit/credit-balance` gains
  `locked: [{event_id, event_name, currency, amount, event_end_ms}]`, rendered
  as "500 THB is covering RTM #6 — it returns when that event ends". It is
  deliberately **not** added into `credit_thb`: that number is what is spendable
  and refundable *now*, and merging the two would let the organizer's reversal
  and the attendee's display disagree.
- **§3 — the clear is refused while locked.** `POST /api/deposit/
  clear-credit-refund-request` returns **409** when the person has no positive
  bucket *and* something is still locked, naming the event. Previously the
  reversal wrote nothing, the flag cleared, and when the event ended the credit
  came back with no open request and no record one was ever made. The payout
  queue shows the locked amount and the event, and the Clear button reads
  "Waiting on event" (a UX hint only — the server is the authority, so a locked
  USDC bucket the THB display does not cover is still caught there).

Tests: `worker/tests/security/test_locked_credit.py` (12) runs the real SQL
against the production migrations — lock vs spendable, ended-event release, a
missing event row staying locked, per-email `return` not releasing a linked
sibling, and the queue's one-row-per-person locked total. Source-scan guard
`payout_clear_refuses_while_credit_is_locked` in `credit_ledger_guards.rs`.
The shared fixture moved to `CreditFixture` so the two SQL modules share
migrations and helpers without re-running each other's tests.

## Still open

- **Partial payouts** still clear the request and drop the locked remainder —
  `.issues/124_partial_payout_drops_the_locked_remainder.md`. The ฿0 case is
  what §3 asked for; the partial one needs a decision about the reversal's
  idempotency key, so it is filed rather than half-built.
- **§4** — let a credit registrant cancel before the event, writing the `return`
  immediately. That is a state transition with its own writers (plan 022), not a
  display change, so it is not in this pass.
- The profile page does not mount `CreditWallet`; My Registrations is the
  signed-in landing surface, so it is the one that closes the dead end. Adding
  the second mount is a one-line change if the owner wants it.
- Nothing above is deployed. It needs migration 0043 (plan 025 §8) in front of
  it, same as #122.
