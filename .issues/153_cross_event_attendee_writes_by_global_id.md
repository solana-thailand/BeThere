# 153: Staff of one event can write another event's attendee by id

**Status:** fixed on develop (parts 1–2 session `event-checkin-fa`, part 3
session `event-checkin-90`). Not in
prod yet: it ships with the next worker release and needs no migration.
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

## Staging verification (2026-09-26, deploy `73140039`, git `14642b3c`)

- `post_deploy_smoke.sh`: Content-Type, headers and writes green. Its admin
  slip upload exercises the scoped lookup on the positive path.
- Own event (`flow-test-event`, placeholder Sheet): participation → Online →
  In-Person, both 200 with `d1_updated: true, sheet_row_updated: false`. This
  was a 500 before.
- Cross-event (event `flow-deposit-20260912`, attendee of `flow-test-event`):
  participation, read, check-in and undo-checkin all refused. D1 read back
  afterwards shows no change: `checked_in_at` kept, `claim_token` still null.
- The refusals are 500, not 404, on staging. After the scoped D1 miss the
  lookup falls back to the Sheet, and staging Sheets are placeholders, so
  absence cannot be proven. With a readable Sheet (prod) the same case is a
  404. Not verified on prod.

## Part 3: the writes themselves (fixed on develop, session `event-checkin-90`)

Parts 1 and 2 left the D1 writers id-only, safe only because the scoped
lookup ran first. They are now scoped themselves, with `AND event_id = ?`:
`check_in_attendee`, `undo_check_in`, `verify_deposit`, `mark_refund`,
`set_qr_url` / `set_qr_urls_batch` (`db/attendees/writes.rs`),
`delete_attendee_by_id` (`management.rs`), and the two unused helpers in
`deposit.rs`. Every caller already had `event` resolved, so each passes
`&event.id`. No route or response changes.

Deliberately still id-only (listed with reasons in the guard's allowlist):
`clear_attendee_pii` (PDPA erasure is cross-event by design), the claim-token
repair (ids read from the same table in the same call), and the Durable
Object's own per-event SQLite.

**Guard:** `worker/tests/attendee_event_scope_guard.rs` lexes every string
literal and `.sql` file under `worker/src` and fails on an `UPDATE attendees`
/ `DELETE FROM attendees` with a bare `id = ?` and no `event_id = ?`. A stale
allowlist entry also fails. Proven both ways: against the Part 2 tree it
flags exactly the 7 writes above; with the fix it passes. Workspace: 88
binaries, 976 tests, 0 failures; clippy `-D warnings` clean.

## Not done

- Nothing in code. Prod verification (cross-event case is 404 with a
  readable Sheet) waits for the next owner-approved prod deploy.
