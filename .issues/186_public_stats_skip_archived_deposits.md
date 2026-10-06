# 186 · Landing "paid a deposit / came" skips archived deposits

Status: open. Found 2026-10-06 (session `event-checkin-42`) while planning
L8 (`.plans/043`); not fixed in the landing staging release.

## What is wrong

`GET /api/public/stats` (`worker/src/db/sql/public_stats.sql`, CTE `paid`)
counts deposits from `thb_deposits` only. The nightly cleanup
(`worker/src/cleanup.rs`) moves an event's deposits to `thb_deposit_archive`
and deletes them from `thb_deposits` 90 days after the refund window. So
every number built on `paid` (deposits handled, THB, payers, payers who came)
shrinks each time an event ages out, and the landing shows it as "so far".

Read-only on prod, 2026-10-06:

| event (D1 id) | live rows | archived rows | verified | came |
|---|---|---|---|---|
| `…-road-to-mainnet-2` (RTM #2) | 0 | 15 | 15 | 15 |
| `…-3-bangkok-copy` (RTM #4) | 20 | 0 | 20 | 16 |
| `…-4-bangkok-copy` (RTM #5) | 17 | 0 | 17 | 15 |
| `…-5-bangkok-copy` (RTM #6) | 34 | 0 | 34 | 22 |

RTM #2's 15 deposits (moved 4 Oct) are already missing from the landing.
RTM #4 is next to age out, then #5.

## Fix

- Read `thb_deposits` and `thb_deposit_archive` together, de-duplicated by
  `source_deposit_id` (the archive's unique key since 0045; it is the live
  row's id), so a row caught mid-purge is counted once.
- The archive has no `deposit_source`. The live filter is
  `deposit_source IS NULL OR IN ('cash','credit')` plus `amount_thb > 0`; a
  `comp` row has amount 0, so `verified = 1 AND amount_thb > 0` is the same
  rule on the archive. Check that against the live rows before relying on it.
- "Came" joins `attendees` by `(attendee_id, event_id)`; attendee rows are not
  purged, so archived deposits keep their check-in.
- The D1 compound-SELECT cap applies (memory `d1-compound-select-cap`): add
  the archive inside the `paid` CTE with one `UNION ALL`, not as new terms.
- Test with an event whose deposits are all archived and one mid-purge.

## Not this issue

- RTM #1 deposits (taken by hand) and RTM #3 deposits (purged 19 Sep, before
  the archive existed) are not in D1 at all. That is the L8 import, in
  `.plans/043`.
