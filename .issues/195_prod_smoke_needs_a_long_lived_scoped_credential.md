# Issue 195: The prod write smoke needs a credential that does not expire every day

**Status:** open. Design only; nothing built. Owner asked for a design (2026-10-25) after the prod
write smoke was skipped twice in one day because the token had expired.

## What happens today

- `scripts/verify/post_deploy_smoke.sh --url <prod> --i-know-this-writes` creates a `smoke-<ts>`
  event, walks a walk-in in, uploads a THB slip, reads the roster, then archives and deletes the
  fixture. On prod it needs `SMOKE_TOKEN`: a real admin session JWT the owner copies from
  `localStorage` into `~/.bethere-smoke-token` (memory `prod-smoke-needs-smoke-token`).
- `Claims::new` (`domain/src/models/auth.rs`) gives every session a **24-hour** expiry. The token
  is a full super-admin session: if it leaks it can do anything the owner can, for a day.
- Result on 2026-10-25: two prod releases (pull 288, pull 294) shipped with the write smoke
  skipped (401). Staging smoke passed both times, and neither release touched the money paths,
  but the gate that exists because of `.issues/138` did not run on prod.

## What a fix has to satisfy

1. **Lives longer than a day** without the owner re-copying anything per release.
2. **Can do only what the smoke does.** Create an event whose id starts with `smoke-`, add a
   walk-in, upload and verify a slip, read that event's roster, archive and delete it. Nothing on
   any other event, no exports, no contacts, no credit, no payouts.
3. **Revocable** without rotating `JWT_SECRET` (which would log everyone out).
4. **Visible:** every write it makes is attributable in the audit log as the smoke principal, not
   as the owner.
5. **Leaves no trace on the organizer's screens** (already true: the fixture is deleted; keep it).

## Options

| | How | For | Against |
|---|---|---|---|
| A | A long-lived JWT for a dedicated `smoke@…` identity, plus a new `UserRole::Smoke` checked by a guard that allows only `smoke-*` event ids | Reuses the JWT path; one guard | A new role touches every role check (`UserRole` is ordered; `Smoke` must sit below `Staff` and never satisfy `>=` comparisons). Revocation still needs a denylist |
| B | A separate `SMOKE_KEY` Worker secret, accepted only on the handful of routes the smoke calls, only for `smoke-*` ids, never as a session | Revoke = rotate one secret; no change to `Claims` or roles; easy to grep | A second auth path. Every route it touches must check the id prefix itself, and that is exactly the class of bug `.issues/153` was |
| C | Keep the 24 h session, but make `deploy.sh` stop and ask for it before a prod deploy | No new auth surface | Still a full super-admin token on disk; the owner has to be present every release |

**Recommendation: B**, scoped tightly. A single middleware decides "is this the smoke key and is
the path one of N allowed routes and does the event id start with `smoke-`", and returns a
principal that every handler already treats as "not a user". Tests: a guard test listing the
allowed routes (adding one is a deliberate act, like `ALLOWED_INTERPOLATIONS`), and one negative
test per route with a non-`smoke-` id. Store the key in the macOS keychain, not a dotfile.

## Open questions for the owner

- Is a second auth path acceptable at all, or should prod writes stay owner-present (option C)?
- Should the smoke principal be allowed on prod only, or staging too (staging has `dev-token`)?
- Where does the key live on the owner's machine (keychain vs. `~/.bethere-smoke-token`)?

## Reopen / build trigger

Build only after the owner picks an option. Until then, the release runbook says: refresh
`~/.bethere-smoke-token` before saying "go prod", or accept that the prod write smoke is skipped.
