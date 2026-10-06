# 187 · Hard-deleting an event leaves its attendees and deposits behind

Status: open. Found 2026-10-06 (session `event-checkin-d6`) after the
release 1 prod deploy, checking `thb_deposits` write volume.

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
