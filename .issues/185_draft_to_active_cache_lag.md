# 185 · Draft → active shows "not open" for 2+ minutes

Status: parked. Not reproduced on staging (2026-10-06, below), and the
proposed fix was wrong. Reopen trigger: a second sighting with timings
(seconds since the flip, colo / `cf-ray`, whether the page was reloaded).

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

## Proposed fix (small) — WITHDRAWN, see the re-check below

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

## Re-check, 2026-10-06 (session `event-checkin-ca`): not reproduced

The cache analysis above does not hold. `get_public_event` answers a
**draft with 404**, and a non-2xx goes out as `no-store`
(`with_public_cache`) and is never edge-stored (`is_edge_storable` wants
200). So neither the 120 s nor the 30 s layer ever holds the "not open"
state, and a short `max-age` on a not-open event would change nothing. The
service worker also fetches `/api/*` with `cache: "no-store"`.

Measured on staging (BKK colo), fixture `e2e-test-event-1790173399`, put
back to draft afterwards:
- curl: three draft reads (404 `no-store`), then `PUT {"status":"active"}`.
  The first poll, 2.6 s later, got 200.
- Reverse: `PUT draft`, and the first poll, 2.1 s later, got 404.
- Browser (puppeteer, 390 px, page under the service worker): loaded
  `/e/{slug}` as a draft ("Event Not Found"), flipped it, and reloaded every
  ~9 s. The first reload, 11.9 s after the PUT, showed the event and
  "Reserve Your Spot".

One real lag was seen: the **admin** read `GET /api/events/{id}` (KV-first)
still returned `active` for a while after `PUT draft` had returned 200,
while the public read already said 404. KV read caching in the colo is the
only layer left that can produce a stale state. It would explain a lag of
up to about 60 s, not 2+ minutes. If it recurs, record which read was stale.
