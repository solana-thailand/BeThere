# 153: Staff of one event can write another event's attendee by id

**Status:** fixed on develop (both parts, session `event-checkin-fa`), not
deployed. It ships with the next worker deploy and needs no migration.
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

## Part 2: `sheets::get_attendee_by_id` accepted any event's attendee (fixed on develop)

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

The same unscoped D1 lookup also reached `DELETE /api/attendee/X`. The D1
delete itself is scoped by `event.id` and email, but when A's Sheet missed,
the handler fell back to the D1 row's `sheet_row_index`. That deletes an
unrelated row of **A's** Sheet.

**Fix:** scope the lookup, not each call site.
- `db::attendees::get_attendee_by_id(db, event_id, id)` runs
  `db/sql/attendee_by_id.sql` (`WHERE id = ?1 AND event_id = ?2`).
- `sheets::get_attendee_by_id` takes `&EventConfig` instead of a loose
  `sheet_id`/`sheet_name` pair, so the D1 half and the Sheet half cannot name
  different events. All 18 callers are updated.
- `get_attendee_by_id_from_d1` (the public W7 path) and
  `resolve_claim_token_from_d1` take the event id.

A foreign id now misses D1 and then misses A's Sheet, so every caller's
existing 404 applies. The rollover handler (VULN-009) passes the source event,
which is the event the id must belong to.

**Tests:**
- `test_participation_override.py` also runs `attendee_by_id.sql`. The
  cross-event miss fails against an unscoped `WHERE` (verified).
- `ticket_name_guards.rs` follows the SELECT into the `.sql` file.

**Data check (read-only, 2026-09-26):** attendee rows whose `event_id` is not
an `events.id` would now miss D1 and fall back to the Sheet.
- Prod: 1 of 533, `solana-x-ai-builders-2`, a removed event that matches no
  id or slug.
- Staging: 5 of 16, from deleted smoke fixtures.
- No live event is affected.

## Not done

- The other id-only `UPDATE attendees … WHERE id = ?` writers (`writes.rs`,
  `deposit.rs`) are safe now that the lookup before them is scoped, but they
  are not scoped themselves. A new caller that skips the lookup would reopen
  this. Scope them if one of them is touched.
- `delete_attendee_by_id` (empty-email fallback in `delete.rs`) is also
  id-only. It is only reached after the scoped lookup.
