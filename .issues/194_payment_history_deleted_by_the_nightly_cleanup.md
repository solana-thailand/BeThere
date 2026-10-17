# 194 — Payment history of past events is deleted by the nightly cleanup

**Status:** deployed (2026-10-14, session `event-checkin-11`). Fixes 1 and 2
on prod (pull 280 + pull 281, release pull 282, `028fa748`, version
`3f20edad`, 2026-10-13 16:48 UTC, after backup
`backup-prod-20261013-2342.sql`); RTM #4 is safe. Fix 3 (RTM #3 restore) run
on prod 2026-10-13 ~17:00 UTC after backup
`backup-prod-20261013-2352-pre-rtm3-restore.sql`. Fix 4: owner keeps
everything for now. Owner 2026-10-13: "the payment history of every past
event should stay, for audit, but it is gone now; is it the cron?" It is.

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

1. **Keep payment rows** (pull 280, deployed): the cleanup no longer deletes
   `thb_deposits`, `deposit_statuses` or `slip_proposals` from D1; it still
   writes the archive and clears the KV mirrors (the read paths are D1-first).
   The event-wide delete functions are removed so nothing can call them again.
   Guard: `worker/tests/cleanup_guards.rs::cleanup_never_deletes_payment_rows`.
   Must reach prod before 2026-10-17 10:00 UTC or RTM #4 goes.
2. **Show every credit holder** (pull 281, deployed): the payout page also
   lists anyone with a positive balance and no refund account on file ("No
   account on file"), so nobody depends on someone remembering them. Prod
   check after deploy: `no_account` = Chanthawat Kiriyadee and Pond Bangna.
3. **Recover RTM #3** (owner go, prod write): the 14 rows are in
   `~/bethere-backups/bethere-db-pre-0029-0032-20260910T142846Z.sql`
   (14 verified, 11 refunded, 2 held as credit, ฿7,000). Re-insert them into
   `thb_deposits` with their original ids, after a fresh backup and only with
   pull 280 on prod (RTM #3 is past its 90 days; the old cron would delete
   them again). Trial on the 2026-10-13 20:45 prod copy: credit_ledger,
   credit_refund_accounts, contacts, audit_log, event_summaries and every
   other event's rows unchanged; both held rows match their ledger hold key;
   no automatic writer acts on them except the nightly archive copy (14
   amount-only rows). One row comes back verified and not refunded: the
   owner's own ฿500 (checked in 20 Jun).

   **Done 2026-10-13 (owner go).** Prod read-back: 14 rows, ฿7,000, 11
   refunded, 2 held, source `cash`; `credit_ledger` 61 rows before and after.
   Side effect the trial missed (it diffed a fixed table list; wrangler
   reported 57 changes against 42 statements): the `deposit_statuses` trigger
   `notification_deposit_insert` queued 14 `deposit_confirmed` rows in
   `notification_outbox` (ids 2015–2028). Nothing was sent (prod
   `NOTIFICATIONS_ENABLED = "0"`), but they were visible in the in-app inbox,
   and `cancel.sql` only runs inside the disabled dispatch. Cancelled on the
   owner's go (`status='cancelled'`, `NO_LONGER_ELIGIBLE`, 14 changes);
   `notification_inbox_visible` shows 0 for the event.
4. **Retention policy** (owner 2026-10-14: keep everything for now; revisit
   with `.issues/160`): rows stay indefinitely, bank fields and slip URLs included. If personal data should
   still go after a period, scrub those fields on settled rows instead of
   deleting the payment.

## Not recoverable

RTM #2's deposit rows were deleted on 22 Aug, before the oldest local backup
(10 Sep); the archive keeps their amounts and flags but not names, bank or
slips. RTM #1 was never in the system.
