# 188: Scanner QR and id lookup read the active event, not the selected one

**Status:** deployed (2026-10-09, `event-checkin-8a`): prod version `4d0203f0` at main `71720705` (tree = develop `9d37fb8f`), migrations 0058 + 0059 applied to prod and staging; staging `b5ca12fd`. Earlier: in progress. Fixed on the branch `feature/178-display-code`
(2026-10-08, session `event-checkin-8a`), not merged to `develop` yet. Found while
building `.issues/178`.

## Defect

`frontend-leptos/src/pages/scanner/logic.rs` `process_attendee_id` called
`api::get_attendee(&attendee_id, None)`. With no `event_id`, the Worker's
`GET /api/attendee/{id}` resolves the **active** event
(`resolve_event_with_access(…, None)` in `worker/src/handlers/attendee/read.rs`).
The confirm step (`page.rs`) writes the check-in with the event selected in
the scanner. So with a non-active event selected:

- a valid ticket for the selected event shows "Attendee not found", or
- the status shown (checked in, approved, in person) is another event's.

All three entry points had it: the camera QR loop, the `?scan=` redirect and
manual id entry. Access control still held (the Worker checks the staff
member's access to the resolved event), so this is a correctness defect, not
a leak. It is the `.issues/153` class: the attendee id is global, so every
by-id lookup has to name its event.

## Fix

`process_attendee_id` takes the scanner's `active_event_id` and forwards it to
`lookup_and_classify`, the same tail the `.issues/178` code path uses. When no
event is selected yet (the `?scan=` effect can run before the event list
loads), it sends `None` and the Worker falls back to the active event, as
before.

Guard: `frontend-leptos/tests/scanner_event_scope.rs` fails if any
`process_attendee_id` call stops passing the selected event, or if the lookup
goes back to `None`.

## Verify after merge

On staging with two events: select the non-active one in the scanner, scan a
ticket of that event, and see "Ready to Check In" with its status. Then scan a
ticket of the active event and see "not found".
