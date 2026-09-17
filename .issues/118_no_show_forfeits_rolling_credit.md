# 118 — A no-show (or a failed check-in write) forfeits rolling credit

**Status:** Deployed — prod `8a3d6d9d` (2026-09-17; staging `20bf192d`; migration 0042 applied; rollback `b5269410`; D1 backup `~/bethere-backups/bethere-db-20260917-pre-118-119.sql`)
**Found:** 2026-09-17, owner report while RTM #6 registration was open
**Severity:** High (attendee money silently taken; 2 people, ฿500 each)

## Owner rule

Rolling credit is the attendee's cash the organizer still holds. Checked in or
not, it stays usable until it is actually paid back.

## What the code did

"Model B" (2026-08-15): registering spends the credit (`apply`, −฿500) and
check-in writes it back (`return`, +฿500). Nothing else returned it, so:

- **ปอนด์ บางนา** was registered for RTM #5 as *online* with credit applied.
  Online attendees never check in, so the credit could never come back.
- **PARIPOL TOOPIROH** *did* check in to RTM #5 (2026-09-02 03:54, staff check-in
  handler, audit entry present) and still got no `return`. That write is
  best-effort (`tracing::error!` and continue), and reconcile never looked for
  "checked-in credit user without a return". The exact cause can't be recovered:
  worker logs from that day are gone, and the apply writer was rewritten on
  2026-09-11 (`fde8b76`).

Both showed ฿0, so both were sent to pay again for RTM #6. PARIPOL uploaded a slip.

Note: RTM event ids are offset from their slugs (slug #6 → id
`...-5-bangkok-copy`). Map via `SELECT slug, id FROM events` before reading a
ledger trail.

## Fix

- `credit_ledger::release_ended_applies` (shared SQL const): one set-based,
  idempotent `INSERT … SELECT` that writes the `return` for every `apply` whose
  event has ended (`events.event_end_ms`) and has none yet. It uses the same
  `return:{event_id}:{email}` key as check-in, so the two can never double-return.
  An event with no end time stays locked.
- Runs before **every** balance read in the ledger module (`balance`,
  `positive_balances`, `liability`, `thb_balances_by_email`, `try_spend`,
  `reconcile`/daily cron), in the payout queue (`contacts::credit_refund_requests`,
  so the amount shown matches what the reversal removes), and as the first
  statement of the atomic apply batch. Errors propagate: skipping it would
  under-report credit, and the payout reversal would then pay out less than owed.
- `remove_return` (undo check-in) is a no-op once the event has ended.
- Check-in return kept as an early release. Comments and
  `docs/deposit-refund-flows.md` no longer describe forfeiture.

## Verification

- Ran the release SQL against a local copy of the prod backup
  (`bethere-db-20260917-pre-114-116-117.sql`). First run: 2 rows (PARIPOL ฿0→฿500,
  ปอนด์ บางนา ฿0→฿500). Second run: 0. The 9 other holders were unchanged, both
  RTM #6 locks stayed, and no balance went negative.
- The undo guard deletes nothing for an ended event and still deletes for an
  upcoming one.
- Guards: `worker/tests/credit_release_and_staff_comp.rs`, mutation-checked
  (release removed from `balance`, from the payout queue, the batch index
  reverted, the undo guard removed; each fails).

## PARIPOL RTM #6 (owner chose: use credit, refund the slip)

PARIPOL's RTM #6 registration already has a pending (unverified) slip. The atomic
apply refuses to replace a cash deposit (`ConflictingDeposit`), and no admin
action removes a pending slip. Converting it to credit means removing that
pending cash record and applying credit; if they actually transferred ฿500 for
#6, that transfer then has to be refunded (or held as extra credit).

Also seen: ปอนด์ บางนา's RTM #4 cash deposit (฿500, verified, online) is neither
refunded nor held. Out of scope here.

**Done 2026-09-17:**
1. Saved the pending rows (they carry the refund bank details) to
   `~/bethere-backups/paripol-rtm6-pending-slip-20260917.json` (outside git,
   mode 600). The slip image stays in R2 at
   `/api/storage/slips/…-5-bangkok-copy/01a0aaef-…`.
2. Removed `thb_deposits` id 56 and its `deposit_statuses` row, guarded on
   `verified=0 AND refunded=0 AND held_as_credit=0`.
3. Ran the release statement on prod: 2 rows. All 10 THB credit holders now read ฿500.

**Owner still to do:** press **Apply Credit** for PARIPOL on the RTM #6 roster
(the button shows now), and transfer back the ฿500 they sent for #6.

## Staging verification (`20bf192d`)

Planted a hold plus an apply on an ended event for the dev identity: the raw
ledger read ฿0, `GET /api/deposit/credit-balance` returned ฿500 twice, and
exactly one `event_ended` return row was written. Test rows removed.

