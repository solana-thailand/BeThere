# 086 — Marking an event completed erases every attendee's registration history

**Status:** fixed 2026-09-13, **not yet deployed to production**
**Found:** 2026-09-13, by the repo owner noticing their own profile said they
had never registered for anything
**Severity:** high, user-facing — no data loss, but it is indistinguishable
from data loss to the person looking at it

## What

`GET /api/my-registrations` excluded events whose status is `completed`, on
**both** paths:

- `worker/src/handlers/sql/my_registrations.sql` —
  `AND e.status NOT IN ('completed','archived')`
- `worker/src/handlers/register/my_registration.rs:160` (KV fallback) —
  `matches!(meta.status, EventStatus::Completed | EventStatus::Archived)`

So the moment an organizer marks a past event finished, every attendee who ever
registered for it loses that row. The profile page then renders its empty
state: *"You haven't registered for any events yet."*

## How it surfaced

Twelve production events were marked `completed` in one sitting on 2026-09-13
(to enable post-event registration for the DevRel recap QR codes). Immediately
afterwards the repo owner's own profile showed no registrations at all, despite
11 rows in `attendees` and 89 in `registration_responses`.

Measured on production at the time of the fix:

```
your registrations under the old filter   0
your registrations under the new filter  11

hidden across everyone:  244 people, 475 registrations
```

475 of 477 registrations in the database were invisible.

## Why the filter existed

The docstring on `my_registrations` says it "iterates active events". The
intent was an *upcoming-events* view. But the endpoint backs the profile page's
registration **history**, and it is also how an attendee reaches:

- their ticket (`/ticket/{id}`)
- their claim link (`/claim/{token}`) — which is exactly what a checked-in
  attendee of a *finished* event needs

Excluding finished events removed the one state where those links matter most.

## Fix

Both paths now exclude only `archived` — the hidden/soft-deleted state.
`completed` is kept, and `build_next_step_from_presence` already does the right
thing for a past event: it keys off claimed / checked-in, returning `ticket` or
`claim` rather than anything registration-like.

`worker/tests/registration_history_guard.rs` pins all three properties: the D1
query keeps completed, the KV fallback keeps completed, and **the two agree** —
because they back the same endpoint and a divergence would make an attendee's
history depend on whether D1 happened to be bound.

Negative-controlled: restoring the old filter fails two of the three tests.

One self-inflicted detail worth recording: the first draft of the third test
matched the word `'completed'` inside the SQL *comment* explaining the fix, so
it failed against correct code. The test now strips `--` comments before
reading the filter. A guard that reads prose is not a guard.

## Related

- `.issues/083` — the admin control whose use triggered this.
- `.issues/068` — post-event/retrospective flows, the reason the events were
  marked completed in the first place.
