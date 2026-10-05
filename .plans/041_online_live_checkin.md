# Plan 041: online check-in from the live stream (for discussion)

**Created:** 2026-10-05, session `event-checkin-87`.
**Trigger:** owner idea, 2026-10-05: an online attendee checks in by scanning
a QR on the opening slide of the live stream, or by opening the YouTube link
from BeThere. They must be signed in and registered; the check-in then lets
them claim the NFT.
**Status:** parked for discussion. Reopen when: the owner answers the two
questions below, and not before the 8 Oct take.

## What exists today (read from the code, 2026-10-05)

- The only online check-in is the Adventure quest:
  `POST /api/adventure/quest-complete` (`worker/src/handlers/adventure.rs`)
  sets `checked_in_at` once the required levels are passed. It is the single
  writer of a virtual check-in; a new path should reuse it, not add another.
- Claim needs a checked-in attendee with a claim token
  (`domain/src/models/next_step.rs`), and the claim link expires 30 days after
  check-in (`worker/src/claim/ttl.rs`).
- `checked_in_at` also feeds dashboards, event summaries and the new public
  "checked in" count, so a loose rule inflates all of them.

## Risks

1. **A static QR leaks.** A slide QR seen on a public stream can be
   screenshotted into a group chat; a registered attendee who never watched
   can scan it and claim.
2. **A click is not attendance.** Opening the YouTube link proves a click, and
   YouTube does not tell us who is watching.

## Proposal

- **Rotating code:** the organizer opens a display page that shows a QR whose
  code changes every 2–5 minutes (HMAC of event id + time step, with a one-step
  grace). Put it in OBS as a browser source instead of a static slide. A
  forwarded screenshot goes stale in minutes.
- **Window:** accept only between `event_start_ms` and `event_end_ms`, and only
  for attendees registered on the online track.
- **Optional second scan:** a code at the start and one at the end, both
  required, for events where the badge matters.
- **YouTube link:** record "joined" but do not count it as a check-in, unless
  the organizer turns that on for the event.
- **One writer:** the scan lands on the same check-in write the Adventure path
  uses, so no second rule can drift.

Size: medium. A Worker endpoint (verify the rotating code, window, track), an
organizer display page, and an attendee landing page that signs in and confirms.

## Open questions (owner)

1. Static QR in the slides (simple, can leak) or a rotating QR from OBS
   (safer)?
2. Replaces the Adventure quest, or a per-event choice next to it?
