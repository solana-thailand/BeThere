# 050: "On now / Next" on the event page, live from Ontime

Status: **built and checked locally, not pushed (2026-11-26).** Branch
`feature/050-ontime-now-card`. Needs migration 0067 before the code deploys.

## Why

Organisers run the show in Ontime (ontime-rs). Plan 019 imports its CSV into
the description: a copy, which goes stale the moment the show is edited or
runs late. Ontime's attendee feed (ontime-rs plan 023, live in production
since 2026-11-26) gives the same agenda without a token and without notes,
and a listen-only socket that sends it again whenever it changes. Attendees
on the event page should see what is on now, what is next and whether the
show runs late, from that feed, with nothing copied into BeThere.

## What

- **Event field `ontime_feed_url`** (migration 0067, `''` = none). The
  organiser pastes the link Ontime shows under Settings → Network → Attendee
  page; it is stored as the feed URL, `https://<host>/r/<room>/public`.
- **`domain::models::event::ontime_feed`**: `normalize_ontime_feed_url` at the
  write, `safe_ontime_feed` at the read. Refused: anything but https, a host
  with user info or odd characters, a room name Ontime would not accept, any
  other path, a `#`, and **any link with `token=`** (a room token lets its
  holder change the show). Error text carries no `://` (the redactor).
- **`domain::models::live_agenda`**: merges `public-data` packets (or the
  `GET /public` body), and works out the card at a given time: the session on
  now and its chosen fields, time left counted from the last packet, progress,
  next, late / ahead / on time in whole minutes (under a minute is on time),
  and the expected or planned finish. Pure, tested natively.
- **`public_event::live_card::LiveCard`** under the event name. Rendered only
  while the show plays or pauses. One loop owns the socket: it opens only
  from 3 h before the start to 3 h after the end, retries with backoff up to
  30 s, and ends with the card (`try_set`), dropping the socket and detaching
  its handlers. When the room turns its feed off (close 1000 with a reason)
  the card forgets the show. A link to the room's own attendee page in the
  page's language.
- **Event form**: "Live agenda from Ontime" under the CSV import, checked as
  it is typed. Duplicating an event keeps the link (a series often runs from
  one room).
- English and Thai strings follow the Ontime attendee page's words.

GPL boundary: BeThere only reads Ontime's public JSON over HTTP and a socket;
no Ontime code is vendored.

## Checks

- `domain/tests/ontime_feed.rs`: 18 tests (links taken and refused, the token
  link, error text, packets with nulls, counting, pause, relative offset,
  planned finish, other messages ignored, clock text).
- `frontend-leptos/tests/listener_cleanup.rs`: the card's loop stops on
  unmount, `Connection::drop` detaches and closes, no `.forget()`.
- Workspace fmt, clippy `-D warnings`, tests (154 binaries); frontend fmt,
  wasm clippy `-D warnings`, tests (77 binaries); floors ratcheted.
- Locally (`wrangler dev --local`, `DEV_MODE`) against a throwaway room on the
  production Ontime Worker, deleted afterwards: the pasted page link is stored
  as the feed; token and http links get a 400. In a 390 px Chromium: the card
  appears while the show plays, shows the session, presenter, next, a
  countdown that moves by itself, the pace, and a token-free agenda link; no
  note anywhere; an edit in Ontime and the operator moving on reach the card;
  Thai works; turning the feed off removes the card. No page errors besides
  the local sign-in 401 and the analytics beacon.

## Deploy (owner's call)

1. Apply `0067_event_ontime_feed_url.sql` (staging, then prod), read the
   schema back.
2. Deploy staging, paste a real room's link on a staging event, check the
   card during a short test show.
3. Prod per the gradual deploy runbook.

## Later

- The same card on the ticket page (attendees at the venue open the ticket).
- Per-session times as Ontime expects them, once ontime-rs sends them.
- The rotating check-in QR (plan 041) can use the same room link.
