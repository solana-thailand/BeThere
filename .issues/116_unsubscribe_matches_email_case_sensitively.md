# 116 — Marketing unsubscribe matches email case-sensitively

**Status:** Open — latent, not live
**Found:** 2026-09-17, `.handovers/137` §4.7 (DevRel), confirmed against the code
**Severity:** Medium (PDPA s.19 withdrawal) if it ever fires; nothing fires it today

## What

`set_marketing_consent` (`worker/src/db/attendees/management.rs`), behind
`POST /api/privacy/unsubscribe-marketing`, withdraws consent with

```sql
UPDATE attendees SET consent_marketing = 0, ... WHERE email = ?
```

Every other attendee-by-email path matches `LOWER(email)`, and migration 0030
added `idx_attendees_email_nocase ON attendees(LOWER(email))` for them —
`upsert_post_event_attendee` among them.

## Why it is correct today, and why that is not enough

Stored emails are lowercase and so is the signed-in identity, so `=` and
`LOWER()` agree on every row that exists now. But the two paths answer
"is this the same person?" differently. A row that ever arrives mixed-case
(a Sheets import, a future writer that skips normalisation) can be *written*
by the case-insensitive paths and then **silently missed by the withdrawal**:
the handler reports success with `changes = 0` while the opt-in stays on.
A withdrawal that misses is the failure PDPA cares most about, and the handler
cannot tell it apart from "nothing to withdraw".

## Proposed fix (not applied)

- `WHERE LOWER(email) = LOWER(?)` — uses the existing 0030 index, so no scan.
- Before shipping, count prod rows with `email <> LOWER(email)`; if any exist,
  the audit in `.issues/115` (DevRel `recipients.py --audit`) must be re-run
  after, since withdrawals that previously missed would start landing.
- Guard in `worker/tests/marketing_consent_guard.rs`: no `WHERE email = ?` in
  a consent write.

## Not in scope

Normalising stored emails (a data migration) — the match should be robust
without it.
