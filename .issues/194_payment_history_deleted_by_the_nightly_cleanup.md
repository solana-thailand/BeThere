# 194 — Payment history of past events is deleted by the nightly cleanup

**Status:** in progress (2026-10-13, session `event-checkin-11`). Owner
2026-10-13: "the payment history of every past event should stay, for audit,
but it is gone now; is it the cron?" It is. Fixed on a branch (pull below);
prod and the recovery of RTM #3 wait on the owner's go.

## What happened

`worker/src/cleanup.rs` phase 2 runs nightly and, at
`event_end + refund_deadline_hours + 90 days`, deleted from D1:

- every `thb_deposits` row of the event (since 2026-09-19 after copying
  amounts and flags into the PII-free `thb_deposit_archive`, `.issues/126`);
- every `deposit_statuses` row (no archive at all; THB, USDC and credit);
- the event's `slip_proposals`;
- the KV mirrors of all of these.

Read from the prod D1 export of 2026-10-13 20:45 (local copy, read-only):

| Event | Ended | Live deposits | Archived | Note |
|---|---|---|---|---|
| RTM #1 | 26 Apr | 0 | 0 | paid by hand; nothing in the system |
| RTM #2 | 24 May | 0 | 15 | deleted 22 Aug; archive only (amounts, no names) |
| RTM #3 | 20 Jun | 0 | 0 | deleted 2026-09-18/19 before the archive existed: 14 rows, ฿7,000 |
| RTM #4 | 19 Jul | 20 | 0 | **would be deleted 2026-10-17 10:00 UTC** |
| RTM #5 | 23 Aug | 17 | 0 | due 2026-11-21 |
| RTM #6 | 4 Oct | 34 | 0 | due 2027-01-02 |

(`deposit_statuses` on prod hold 20 / 17 / 34 rows for RTM #4–#6.)

## Who this hurts today

`credit_ledger` is never deleted, so balances are intact. But the admin finds
a credit holder through their deposit row (Held as Credit tab), their refund
account (copied from the deposit) or an open request. Two people paid at
RTM #3, held the ฿500 as credit, and still hold it, with none of the three:

- **Chanthawat Kiriyadee**, ฿500 (held from RTM #3)
- **Pond Bangna** ("pangpond"), ฿500 (held from RTM #3 on 2026-08-14, applied to
  RTM #5 (event id `…-4-bangkok-copy`; the RTM ids are offset by one) the same
  day, returned on 2026-09-17 by the ended-event release because they did not
  come)

Neither appears in any admin list. Five RTM #5 holders (฿500 each) are still
visible through their deposit account.

## Fix

1. **Keep payment rows** (this branch): the cleanup no longer deletes
   `thb_deposits`, `deposit_statuses` or `slip_proposals` from D1; it still
   writes the archive and clears the KV mirrors (the read paths are D1-first).
   The event-wide delete functions are removed so nothing can call them again.
   Guard: `worker/tests/cleanup_guards.rs::cleanup_never_deletes_payment_rows`.
   Must reach prod before 2026-10-17 10:00 UTC or RTM #4 goes.
2. **Show every credit holder** (follow-up PR): the payout list should include
   anyone with a positive balance, account or not ("No account on file — ask
   the attendee"), so nobody depends on someone remembering them.
3. **Recover RTM #3** (owner go, prod write): the 14 rows are in
   `~/bethere-backups/bethere-db-pre-0029-0032-20260910T142846Z.sql`
   (14 verified, 11 refunded, 2 held as credit, ฿7,000). Re-insert them into
   `thb_deposits` with their original ids, after a fresh backup.
4. **Retention policy** (owner decision, with `.issues/160`): rows now stay
   indefinitely, bank fields and slip URLs included. If personal data should
   still go after a period, scrub those fields on settled rows instead of
   deleting the payment.

## Not recoverable

RTM #2's deposit rows were deleted on 22 Aug, before the oldest local backup
(10 Sep); the archive keeps their amounts and flags but not names, bank or
slips. RTM #1 was never in the system.
