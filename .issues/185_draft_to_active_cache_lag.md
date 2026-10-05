# 185 · Draft → active shows "not open" for 2+ minutes

Status: open. Parked behind the 8 Oct take (`.plans/039` freeze: the public
event page is demo-facing); reopen trigger: the take is done.

## What was seen

After an organizer flipped an event from draft to active, the public event
page kept saying registration was not open for more than two minutes
(session `event-checkin-87`, 2026-10-05, staging).

## Why (read from the code, 2026-10-06; not yet reproduced with timings)

Three caches stack on `GET /api/public/event/{slug}`:

1. `cache_public_120_layer` (`worker/src/middleware/cache.rs`): the browser
   keeps the body for `max-age=120`.
2. `edge_cache_layer` (`worker/src/middleware/edge_cache.rs`): the colo
   keeps it for 30 s.
3. KV (the event read path is KV-first): eventual consistency, up to about
   60 s in other colos.

A viewer who loaded the draft page can wait up to 120 + 30 s, plus the KV lag.
That matches "over two minutes". The moment of the flip is exactly when
people are refreshing, so the 120 s window falls in the worst place.

## Proposed fix (small)

In `get_public_event` (`worker/src/handlers/public_event.rs`), when the
event is not open for registration (draft / not yet open), set the handler's
own `Cache-Control: public, max-age=10`. A handler-chosen `Cache-Control`
already does both things needed: `is_edge_storable` refuses it, so the edge
copy is skipped, and `with_public_cache` leaves it alone, so the 120 s is
skipped. An open event keeps today's caching. KV lag is not addressed; the
write colo usually sees the write at once.

## Repro / done when

- On staging: load `/e/{slug}` for a draft, flip it to active with
  `PUT /api/events/{id}`, reload every 5 s, and record the seconds until the
  register button shows. Today: expected around 120 s or more. After: 10 s or
  less in the organizer's colo.
- A native test pins that a not-open event's response carries the
  short `Cache-Control` (and so is not edge-storable).
