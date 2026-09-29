# 176: Design: a waitlist queue for full events, with email auto-promotion

**Status:** open (design only; filed 2026-09-29 by session `event-checkin-ba` as the GOAT-hardening handoff requires; nothing is built).

## Context

When an event is full, the page says "เต็มแล้ว / FULL" (P2-b) and offers
nothing else. The handoff asks for a queue that promotes people by email when
someone cancels. The landing's `/api/waitlist` is a different thing: an
organizer-interest list in a Google Sheet.

## Questions to decide before any code

1. **Ordering:** join time only, or a priority (returning attendees, comp
   list)?
2. **Promotion trigger:**
   - a cancellation;
   - a no-deposit timeout (`deposit_deadline_hours`);
   - an organizer raising capacity;
   - which run inline, and which on the cron?
3. **Offer window:** how long a promoted person has to confirm (and pay the
   deposit) before the seat passes to the next. The deposit rules come from
   `.memory/deposit-policy-decisions`: nothing forfeited; THB refund ≤ 7
   days.
4. **Notification:** email through `notifications/` (`NOTIFICATIONS_ENABLED`
   is 0 in prod today). The message kinds and max ages follow `.issues/128`.
5. **The full-state CTA on `/e/{slug}`:** "Join the waitlist" (it needs
   sign-in, like registration), and how a queued person sees their position.
6. **Capacity accounting:** promotion must be one D1 transaction against the
   same count `enforce_capacity` uses. Check every writer of the
   registration transition (the `duplicated-state-transition-paths`
   memory).
7. **PDPA:** a queued person's data and its retention if never promoted.

## Out of scope until decided

Schema (a `waitlist` table or a `registration_phase` value), endpoints, UI.
