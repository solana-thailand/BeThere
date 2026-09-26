# 152: Changing an event's start date never reached D1

**Status:** deployed 2026-09-26. Migration 0051 was applied to prod at about
07:17 UTC, with owner go in session `event-checkin-b9`. There is no code
change. Before applying it: a staging rehearsal, a D1 backup
(`worker/backup-prod-20260926-1417.sql`, gitignored), and the trigger read back
from `sqlite_master`. **Verified live:** the owner re-saved RTM#6 at 08:18 UTC,
and D1 now reads 1791093600000 / 1791104400000 (4 Oct, 13:00–16:00 Bangkok
time). `/api/public/events` shows 4 Oct. RTM#6 has 0 credit `return` rows, so
nothing was released early.
**Found by:** the owner postponing RTM#6 (27 Sep → 4 Oct 2026, flooding). A
read-only check afterwards showed the public page on 4 Oct and D1 still on
27 Sep.

## What happens

`PUT /api/events/{id}` writes KV, then `sync_event_to_d1` →
`db::events::upsert_event`, an `INSERT … ON CONFLICT (id) DO UPDATE`. When
`event_start_ms` changes, that fires the `notification_reschedule` trigger
(migration 0030), whose `INSERT OR IGNORE` re-creates each enrolled
attendee's reminder.

SQLite gives the statement that fired a trigger the last word on conflict
handling: under the upsert, the trigger's `OR IGNORE` is dropped. The first
reminder that already exists aborts the whole write with
`UNIQUE constraint failed: notification_outbox.dedup_key`.
`sync_event_to_d1` only logs a warning, and the Worker keeps no logs, so the
failure left no trace. KV had the new date and D1 kept the old one.

Every existing test moved the date with a plain `UPDATE`, which does not trip
it.

**Scope:** any start-date (or `time_tba`) change on an event that has at least
one notification enrollment. On 2026-09-26 only RTM#6 had drifted: D1 and KV
agree for every event that is active, draft or ended in the last 60 days.

## Why it matters

These read `events.event_end_ms` from **D1**, not KV:

- the credit ledger's `RELEASE_ENDED_APPLIES_SQL`. RTM#6 has 8 `apply` rows,
  and their credit would be returned on the old end, **27 Sep 09:00 UTC**,
  a week before the event;
- notification reminder scheduling (notifications are disabled in prod, so
  there is no mail impact today);
- anything else in SQL that joins `events` for dates.

## Reproduction

Prod schema (from `sqlite_master`) and the RTM#6 `events`, `notification_*`
rows loaded into local SQLite; no attendee PII.

- The upsert with the new dates → `UNIQUE constraint failed:
  notification_outbox.dedup_key`.
- A plain `UPDATE` of the same dates → OK.
- After migration 0051 → the upsert succeeds, and the 56 pending reminders
  move to the new start minus 24 h.

## Fix (develop)

- `worker/migrations/0051_reschedule_trigger_upsert_safe.sql` recreates the
  trigger with the dedup in the SELECT (`NOT EXISTS`), which no outer policy
  can override. Behaviour is otherwise unchanged.
- The test `test_reschedule_through_the_event_upsert` in
  `worker/tests/notifications/test_outbox.py` moves the date through the
  upsert shape and pins that shape in `db/events.rs`. It fails without 0051
  and passes with it.

## Prod now (owner-gated)

1. Back up D1, apply migration 0051 to prod, then read the trigger back from
   `sqlite_master`.
2. Save RTM#6 once more in the admin event form. Nothing needs changing; the
   save re-runs the D1 dual-write.
3. Check that D1 `events.event_start_ms` / `event_end_ms` for
   `solana-x-ai-builders-the-road-to-mainnet-5-bangkok-copy` read
   1791093600000 / 1791104400000 (4 Oct 13:00–16:00 Bangkok time).

All of this must happen before **27 Sep 09:00 UTC** (16:00 Bangkok time).
Once that time passes, the ledger writes `return` rows for the 8 credit
holders. Nothing removes them automatically when the date is fixed later
(`remove_return` runs only on undo-check-in).

## Not done

- `sync_event_to_d1` still swallows errors. A failed dual-write should be
  visible to the admin who saved, for example as a warning in the response.
  Separate change.
