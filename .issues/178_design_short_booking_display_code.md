# 178: Design: a short booking display code on tickets

**Status:** deployed (2026-10-09, `event-checkin-8a`): prod version `4d0203f0` at main `71720705` (tree = develop `9d37fb8f`), migrations 0058 + 0059 applied to prod and staging; staging `b5ca12fd`. Earlier: in progress — built on `feature/178-display-code` (2026-10-08, session `event-checkin-8a`), not merged, migration not applied anywhere remote. Owner approved the design as written on 2026-10-08. Filed 2026-09-29 by session `event-checkin-ba` (design only). P2-a shipped the inline ticket as QR-only on purpose.

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

## Built on feature/178-display-code (2026-10-08, `event-checkin-8a`)

Commits: `5bafa9cf` (domain `DisplayCode`), `9ff4244e` (migration 0058,
worker writers, ticket field, staff lookup, e2e seed codes), `9e347ccd`
(ticket `Nº` row, scanner Enter-code path, EN+TH strings), `4418ad03`
(lowercase `nº` prefix), plus the docs/floors commit.

- **Backfill:** SQL `random()` per character, two redraw passes for
  per-event duplicates, then a last pass that NULLs any survivor, and only
  then the partial UNIQUE index — the index build cannot fail. A NULL code is
  assigned lazily on the first ticket read (`ensure_display_code`), which
  also covers rows old code inserts between the migration and the deploy.
- **Writers:** `upsert_attendee`, `upsert_post_event_attendee`,
  `try_insert_walkin`, the DO→D1 mirror (conditional UPDATE after insert,
  best-effort) and `upsert_attendee_full` (inline, `COALESCE` on conflict).
  5 attempts on a unique collision. Guard: `worker/tests/attendee_display_code.rs`.
- **Lookup:** `GET /api/attendees/by-code/{code}?event_id=` (staff router,
  `resolve_event_with_access` first, returns only `attendee_id`).
- **Not done:** the landing inline ticket (P2-a, `pages/landing/registrations.rs`)
  does not show the code yet — that file belongs to a peer; `TicketCode` in
  `pages/ticket/qr_section.rs` is public for it, and `/api/my-registrations`
  does not return the code yet.
- **Visual baselines:** the ticket snapshots change (a new row under the QR);
  the e2e seed pins fixed codes so they stay stable once re-taken from CI.

**Apply order:** `npx wrangler d1 migrations apply bethere-db --remote`
(staging first) **before** the code deploy, then read the schema back
(`PRAGMA table_info(attendees)`, `PRAGMA index_list(attendees)` shows
`idx_attendees_event_display_code`, unique, partial) and count NULL codes.
After deploy, check write volume on `attendees`.
