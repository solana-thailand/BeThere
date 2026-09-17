# 120 — The "return my credit" path is hard to reach for most credit holders

**Status:** Open — proposal, not built
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
