# 162: Walk-ins read as online everywhere except the capacity count

**Status:** open (2026-09-28). Found by session `event-checkin-fa` during the `/simplify` altitude review of `feature/028-w3-track-counts`. Nothing is changed yet: the fix changes how the badge claim and the ticket page treat walk-ins, and it should land after RTM #6 (4 Oct).

## What happens

A staff walk-in is stored with `participation_type = 'walkin'`
(`worker/src/db/attendees/walkin.rs`). `ParticipationType::parse("walkin")`
returns `Other`, so `Attendee::is_in_person()` is false for someone who is
physically in the room. A unit test pins it this way on purpose
(`worker/src/handlers/register/tests.rs`: "walk-in sentinel stays out of both
tracks").

W3 (.issues/157) fixes this for **capacity only**: `TrackCounts::add`
special-cases `PARTICIPATION_WALK_IN`. Every other reader still calls
`is_in_person()`:

- `worker/src/claim/mint/execute.rs:113`: `is_online_attendee = !is_in_person()`.
  A walk-in who claims the badge hits the **online claim timing gate** and
  cannot claim until the event ends. Line 137 then takes the online virtual
  check-in branch when the event format has an online track.
- `worker/src/handlers/attendee/read.rs:100,346`: the `is_in_person` JSON field
  is false, and `:384` picks the online `ticket_note`.
- `worker/src/handlers/deposit/usdc/handlers/status.rs:61`.

## Fix (the root cause, not another special case)

Classify at the parser:
- either add `ParticipationType::WalkIn`, with `is_in_person()`-equivalent
  semantics and `as_str() == "walkin"`;
- or have `parse` map `"walkin"` to `InPerson`.

Then:
- drop the special case in `TrackCounts::add`;
- keep public signup rejecting it: `resolve_participation_type` in
  `register/signup.rs` must still refuse a user-supplied "walkin"; add that as
  an explicit check.

## Repro

`rg -n "is_in_person\(\)" worker/src/claim worker/src/handlers` on `develop`,
plus `ParticipationType::parse("walkin") == Other` in `register/tests.rs`.
