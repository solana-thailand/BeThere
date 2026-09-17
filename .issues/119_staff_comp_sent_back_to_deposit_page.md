# 119 — Comped staff and organizers are sent back to the deposit page

**Status:** Fixed on `fix/credit-release-and-staff-comp` (2026-09-17); not deployed
**Found:** 2026-09-17, owner report (registered free for RTM #6, then every later visit went to the deposit page)
**Severity:** Medium (organizer UX; with `deposit_deadline_hours` it can also move them to Online)

## What

At registration, the staff/organizer/super-admin waiver wrote only a ฿0
`thb_deposits` row (`STAFF_COMP_WAIVED`) and returned a hard-coded ticket
step. Every later reader routes on `deposit_statuses`: `build_next_step`
(my-registration, duplicate registration), the ticket page's deposit card, and
the deposit-deadline check. With no status, the organizer looked unpaid. Past
the event's `deposit_deadline_hours` (1 h on RTM #6), the deposit page's gating
would also switch them to Online.

## Fix

- `signup::record_staff_comp` is the one comp writer. It saves the `thb_deposits`
  row **and** a verified, non-refundable ฿0 `thb` deposit status. It's used for new
  registrations and to repair on re-registration.
- Migration `0042_staff_comp_deposit_status.sql` backfills existing comps
  (`ON CONFLICT DO NOTHING`, so a real status is never overwritten). Prod had
  exactly one comp without a status (RTM #6). Tested twice on a copy of the prod
  backup: adds that row, leaves the RTM #5 comp's existing status alone.
- ฿0 statuses don't affect money readers: dashboards count `usdc` only, and
  `max_refundable_deposits` (40) equals the in-person cap, so the extra count
  can't push anyone out of the refundable tier.

## Guard

`worker/tests/credit_release_and_staff_comp.rs`
`staff_comp_writes_the_status_the_router_reads`, mutation-checked (dropping the
status save fails it).
