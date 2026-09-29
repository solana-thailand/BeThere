# 178: Design: a short booking display code on tickets

**Status:** open (design only; filed 2026-09-29 by session `event-checkin-ba`). P2-a shipped the inline ticket as QR-only on purpose.

## Why not derive it

The handoff is explicit: never derive a short code from `claim_token` or the
attendee UUID. `claim_token` is a capability. It goes in the claim URL
(`.issues/071` bounds its replay window), and a prefix of it is a partial
secret. The attendee id is global across events (`.issues/153`).

## Proposal to decide

- **Storage:** a new `attendees.display_code` column. 6 characters from an
  unambiguous alphabet (no 0/O/1/I/L). It is unique per event (a partial
  unique index on `(event_id, display_code)`), random, and assigned at
  registration with a retry on collision.
- **Use:**
  - read aloud or typed at the door ("Enter manually" in the scanner looks
    it up by event and code);
  - shown under the QR on the ticket and in the P2-a inline ticket.
- **Scope:** the code is not a credential. The lookup must stay staff-only
  and event-scoped, and it must never unlock claim or deposit actions.
- **Backfill:** existing rows get codes in a migration. Check write volume
  after deploy (the `migration-constraints-break-existing-writers` memory).
