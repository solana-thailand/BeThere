# 187 · Hard-deleting an event leaves its attendees and deposits behind

Status: in progress. Fix on `feature/187-event-delete-rows` (pull 158,
option 1 below, owner's call 2026-10-06), on staging since 2026-10-06 08:36
UTC (`346686f7`, version `366e0b08`); prod waits for the owner's go. Found 2026-10-06
(session `event-checkin-d6`) after the release 1 prod deploy, checking
`thb_deposits` write volume.

## What is wrong

`DELETE /api/events/{id}/delete` (`worker/src/handlers/events/lifecycle.rs`,
`db/events.rs` `DELETE FROM events WHERE id = ?`) removes the event row only.
Its `attendees`, `thb_deposits` (and likely `deposit_statuses`,
`slip_proposals`) rows stay, pointing at an event that no longer exists.

`post_deploy_smoke.sh` creates a fixture event, uploads a ฿500 slip, then
hard-deletes the event, so every write smoke leaves one attendee and one
deposit row:

| env | orphan `thb_deposits` | orphan `attendees` | read |
|---|---|---|---|
| prod | 1 (฿500, 2026-10-06 07:27 UTC, the release 1 smoke) | 1 | 2026-10-06 |
| staging | 47 | 47 | 2026-10-06 |

## Impact

- Public stats: none. `public_stats.sql` joins live events, so orphans are
  not counted (prod figures equal the pre-deploy read: 65 / 61, ฿41,000).
- Money table hygiene: a fixture ฿500 sits in prod `thb_deposits`. The
  nightly purge works per event end, so it never archives or deletes it.
- Any report that reads `thb_deposits` without joining `events` counts it.

## Repro (read-only)

```sql
SELECT COUNT(*) FROM thb_deposits t LEFT JOIN events e ON e.id = t.event_id
 WHERE e.id IS NULL;
```

## Fix options (not chosen)

1. The hard delete also deletes the event's attendees, deposits, statuses
   and proposals in one batch (it is refused unless the event is archived).
2. The smoke cleanup deletes its own fixture rows first.

Option 1 fixes the cause; option 2 only the smoke. Removing the prod orphan
needs an owner go (a write to prod D1).

## Fix (option 1)

`db/event_purge.rs`, called by `sync_delete_event_from_d1` before the events
row goes: archive the deposits' amounts (same rule and SQL as the nightly
purge; nothing is deleted unless the archive covers every live deposit), then
one D1 batch (`sql/event_purge.sql`) deletes the event's rows from 15 tables,
attendees last. If the purge fails the events row is kept too, so nothing
points at a missing event.

Kept on purpose: `audit_log`, `credit_ledger` (a person's balance),
`thb_deposit_archive`, `onchain_events`, `escrow_index`, `nft_mint_jobs`.
`event_summaries` goes with the events row as before.

Side effect: each write smoke now leaves its ฿500 fixture as one row in
`thb_deposit_archive` (`event_slug` = `smoke-…`). Public stats do not read
archive rows whose event is gone.

Tests: `worker/tests/security/test_event_purge.py` (every `event_id` table is
purged or kept on purpose; nothing left for the deleted event, another
event untouched; archive, credit and audit outlive it; both fail on an empty
purge) and `worker/tests/event_purge_statements.rs` (the split).

## Orphans found 2026-10-06 (prod, read-only, after the release 2 smoke)

| table | orphans | from `smoke-*` events |
|---|---|---|
| attendees | 3 | 2 |
| thb_deposits | 2 | 2 |
| deposit_statuses | 2 | 2 |
| registration_responses | 5 | 0 |

The 1 attendee and 5 answers not from smoke events belong to some other
deleted event; the owner limited the cleanup to smoke fixture rows.

## Verified on staging (2026-10-06)

All orphans on staging before the fix deploy: 54 deposits, 67 attendees;
`thb_deposit_archive` smoke rows: 0. After a write smoke on the fixed build:
54 / 67 unchanged, archive smoke rows 1. Before the fix each smoke added one
of each.

## Cleanup counts (read-only, rows tied to deleted `smoke-*` events)

| env | attendees | thb_deposits | deposit_statuses | smoke events | fixture ฿ |
|---|---|---|---|---|---|
| prod | 2 | 2 | 2 | 2 | 1,000 |
| staging | 46 | 46 | 46 | 46 | 23,000 |

Every other purged table has none. Removal waits for the fix on prod, a
backup and the owner's go on these counts.

