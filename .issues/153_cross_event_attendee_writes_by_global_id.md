# 153: Staff of one event can write another event's attendee by id

**Status:** in progress. Part 1 (participation override) is fixed on develop.
Part 2 (the shared lookup) is being worked on.
**Found by:** staging verification of plan 032 (session `event-checkin-2e`).
`PATCH /attendee/{id}/participation-type` returned 500 for an event whose
`sheet_id` is a placeholder. Reading the handler turned up the authz gap
below, and an audit of every id-only `UPDATE attendees` found the same gap in
the shared lookup.

## Root cause

`attendees.id` is a **global** primary key. `resolve_event_with_access`
authorizes the caller for event A, but the attendee lookups and writes that
follow only check the id, not that the id belongs to A.

## Part 1: participation override (fixed on develop)

`worker/src/handlers/attendee/participation.rs`:

1. **500 on an unreadable Sheet.** A failure in `get_attendees_map` became a
   hard 500 before D1 was reached, even though D1 alone can carry the change.
2. **Cross-event write.** If the id was not in A's Sheet, the handler fell
   through to `UPDATE attendees … WHERE id = ?`. A staff member of A could
   move an attendee of event B between tracks, and got a 200 with
   `sheet_row_updated: false` even when nothing matched.

**Fix:**
- `db::attendees::set_participation_type` now takes `event_id`. It runs
  `db/sql/attendee_participation_set.sql` (`WHERE id = ?2 AND event_id = ?1`)
  and returns whether a row changed.
- The handler writes D1 first. With no Sheet row, that scoped write is the
  only proof of membership:
  - no match → 404;
  - D1 error → 500;
  - no match and the Sheet was unreadable → 500 with the Sheet error.
- An unreadable Sheet with a D1 match → 200, D1 only.

**Test:** `worker/tests/security/test_participation_override.py` runs the SQL
against every migration. It fails against the old id-only `WHERE` (verified).

## Part 2: `sheets::get_attendee_by_id` accepts any event's attendee (open)

`sheets::get_attendee_by_id(id, state, sheet_id, sheet_name, kv)` tries D1
first with `SELECT … FROM attendees WHERE id = ?1`
(`db/attendees/reads.rs:126`). It only reads event A's Sheet when D1 misses.
Event B's attendee is therefore "found" while A is authorized. Callers that
then write by id (audit, session `event-checkin-fa`):

| Route | Write | Effect when X is an event B id |
|---|---|---|
| `POST /api/checkin/X?event_id=A` | `check_in_attendee` | X checked in by A's staff, new claim token |
| `POST /api/attendee/X/undo-checkin?event_id=A` | `undo_check_in` | X's check-in and claim token cleared |
| `POST /refund/manual/X` (`event_id` A) | `mark_refund` | X's refund fields set |
| `POST /deposit/thb/admin-upload` (A, X) | `verify_deposit`, `set_qr_url` | X's deposit verified; also creates an (A, X) deposit record that unlocks verify, comp and mark-refund for X |

Routes found safe: `adventure` (email-filtered), claim mint (secret token),
slip verify, comp and refund mark (they need an (A, X) deposit record, except
through the admin-upload chain above), USDC paths (on-chain proof),
`repair-claim-tokens` (super-admin, all events by design). The Durable Object
check-in and `save_deposit_status_to_d1` have no production callers.

The exploit needs a staff account on some event and a victim's attendee id
(ticket QR codes carry it).

**Planned fix:** scope the lookup, not each call site. Its D1 half filters on
`event_id`, so a foreign id misses D1, then misses A's Sheet, and every
caller's existing 404 applies.
