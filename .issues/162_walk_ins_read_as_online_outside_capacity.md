# 162: Walk-ins read as online everywhere except the capacity count

**Status:** deployed (2026-10-10, session `event-checkin-d3`, owner go): prod version `07ea1885` at main `9c3ed7e4` (release pull 182; pull 180 merged into develop as `05ea93ab`), staging `06bdbeb4` ran the same tree first. On staging a fresh walk-in's ticket API returned `participation_type: walkin, is_in_person: true` and the page rendered the in-person view (before the fix it rendered the online view). The fix is `5284959c` (session `event-checkin-1a`), replayed with `stats_track` reading `WalkIn`. Filed 2026-09-28 by session `event-checkin-fa`.

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
  *Correction (2026-10-01):* a walk-in with a D1 row never reaches these
  lines. `ctx.walkin()` returns early at `execute.rs:43` into
  `execute_walkin_claim`. Only the Sheets fallback (D1 unavailable or no
  row) reaches the gate, so the claim symptom is narrower than stated.
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

## Fix as built (2026-10-01, `feature/162-walkin-in-person`)

This takes the first option: a `ParticipationType::WalkIn` variant.
- **Sentinel round trip:** `as_str()` and serde give back `walkin`. Any path
  that canonicalizes through the enum keeps the sentinel, so the walk-in claim
  path, the delete path and the duplicate check still find the row. That is why
  this option beat "parse walkin as InPerson".
- **Exact match:** `parse` recognizes only the exact stored `walkin`. That is
  the same comparison as the SQL `participation_type = 'walkin'` and the claim
  and delete paths, so `Walkin` and ` walkin` stay `Other`, as before.
- **One in-person rule:** `ParticipationType::is_in_person()` covers
  `InPerson | WalkIn`, and `Attendee::is_in_person()` delegates to it.
  `TrackCounts::add` lost its special case.
- **Walk-in stays invalid as a choice:** public signup
  (`resolve_participation_type`, an exhaustive match) and the manual override
  (`normalize_override`) still refuse `walkin`. The existing tests pin both.
- **Left alone on purpose:**
  - `IN_PERSON_PREDICATE` (the no-show denominator). A walk-in is checked in
    on insert, so leaving it out of both counts keeps `no_show_count`
    unchanged.
  - The frontend string helper `utils::is_in_person`, which deposit badges and
    the participation toggle key on.
  - Both are documented where they live.
- **Behaviour that changes for walk-ins:** the ticket JSON
  `is_in_person: true`, the in-person `ticket_note`, and the in-person label on
  the ticket page. On the Sheets fallback of the claim path, the online timing
  gate no longer applies.
- **Deposit-deadline readers** (`read.rs:249`, `usdc/handlers/status.rs:59`,
  `signup.rs:293`) now see a walk-in as in-person. They still skip it, because
  they need a `registration_date`, and D1 walk-in rows have none
  (`db/attendees/reads.rs`). The QR backfill (`read.rs:195`) needs a verified
  deposit, which a walk-in doesn't have.
- **Tests:**
  - New: `domain/tests/participation_walk_in.rs` (5 tests).
  - Updated: `participation_type_parse.rs` (the reference gained the exact
    `walkin` arm), `track_counts.rs` and `register/tests.rs`.
  - Workspace: 117 binaries and 1134 tests green, clippy `-D warnings` clean.
  - Frontend: 39 binaries and 316 tests green, wasm32 clippy clean.
- **Still owed:** a staging check after the merge. Create a walk-in, open its
  ticket page and confirm the in-person note.
