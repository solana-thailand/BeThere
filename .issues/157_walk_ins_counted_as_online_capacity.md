# 157: Walk-ins take an online spot, and some capacity checks ignore them

**Status:** in progress — fixed on branch `feature/028-w3-track-counts` (commit 2e44acd1, session `event-checkin-f2`, 2026-09-28), deliberately not merged: it rides with plan 028 W3, which §5 holds off `develop` until after RTM #6 on 2026-10-04. Merge after that, with a staging rehearsal. Found while building W3.

## What happens

A staff walk-in is stored in D1 `attendees` with `participation_type = 'walkin'`
(`db::attendees::try_insert_walkin`). The capacity counts read the full D1
attendee list and classify each row with `ParticipationType::parse`, which
returns `Other` for `"walkin"`. `Other` counts toward the online track
(`counts_toward_online_track`, the legacy catch-all). So:

- **Public event page** (`public_event.rs::count_attendees_by_track`) and
  **registration** (`register/capacity.rs::enforce_capacity`): each walk-in is
  counted as online from the list, then added to in-person by the separate
  `count_walkin_attendees` query. One walk-in uses one online spot and one
  in-person spot. On a hybrid event with an online cap, walk-ins shrink the
  "online spots remaining" figure and can close online registration early.
- **Deposit-deadline reclaim** (`attendee/read.rs`, `slip_upload.rs`,
  `slip_admin_upload.rs`, and the duplicate-registration path in
  `register/signup.rs`): count in-person with `is_in_person()`, which is false
  for `"walkin"`, and never add the walk-in count. A reclaim can be offered on
  an event whose room is already full of walk-ins. `deposit/usdc/gating.rs`
  did count them, so the same question had two answers.

The test comment in `register/tests.rs` says "walk-in sentinel stays out of
both tracks", which is not what the counting code did.

## Evidence

Code reading, then pinned by `domain/tests/track_counts.rs`: the pre-fix loop
(`per_attendee`) puts `"walkin"` in the online bucket; `TrackCounts` puts it in
in-person. The sheet mirror writes walk-ins as `In-Person`
(`sheets/write/append.rs::append_walkin_row`), so events whose count came from
the sheet were not affected; only D1-served counts were.

**Not measured:** how many prod events have walk-ins together with an online
cap, i.e. whether any real online registration was closed early. That needs a
read-only prod D1 query, not run.

**RTM #6 exposure checked 2026-09-28** (public API, no D1 query): the event is
hybrid, in-person 25/40, online 32/100 (68 remaining), `online_open_mode =
always`. Walk-ins arrive on the day (2026-10-04). Closing online registration
through this bug would take more than 68 walk-ins on an event whose whole
in-person cap is 40, so no hotfix before RTM #6; the fix merges with W3 after
it. The online remaining figure on that page will read low by one per walk-in
on the day.

## Fix (on the branch)

`handlers::capacity::count_tracks` is the one counter: a D1
`GROUP BY participation_type`, classified by `TrackCounts::add` (walk-in →
in-person, otherwise `ParticipationType::parse`). All eight sites use it, or
tally a list already in hand with `TrackCounts`. The source guard
`worker/tests/capacity_count_single_home.rs` bans counting by filtering a
fetched attendee list anywhere in `src/`.

## Known leftover

With D1 erroring, the count falls back to the sheet plus the D1 walk-in count.
Mirrored walk-ins are `In-Person` sheet rows, so that path can count a walk-in
twice. It errs toward "full" and only runs during a D1 failure; unchanged from
before.
