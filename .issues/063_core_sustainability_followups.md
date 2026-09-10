# 063 — Core service sustainability follow-ups

Status: open

The 2026-09-09 core-path review removed the two largest avoidable request
amplifiers: `my-registrations` now uses one user-scoped D1 join, and each live
dashboard poll uses two concurrent D1 reads instead of five sequential reads.
Hidden or unmounted dashboard tabs no longer poll.

Remaining work, ordered by impact:

1. **Distributed abuse control — implemented.** Native Workers Rate Limiting
   bindings now coordinate counters across isolates within each Cloudflare
   location, with the in-memory limiter retained for local development and
   binding failures. The pre-authentication guard uses the edge-provided IP
   for every route group, ignoring arbitrary credentials and forwarding headers.
   Both webhook routes use a separate per-source budget. The fallback is capped
   at 4096 entries and rejects new sources while full. Cloudflare documents the binding as local,
   asynchronous and without meaningful request latency:
   <https://developers.cloudflare.com/workers/runtime-apis/bindings/rate-limit/>.
   It is intentionally not treated as exact accounting, and authorization plus
   idempotency remain the primary controls.
2. **Event/admin pagination.** `db::events::list_events` and several organizer
   lists are unbounded. Add cursor pagination before platform event volume makes
   those responses or D1 reads material. Preserve current small-data UX with a
   frontend load-more flow.
3. **Measured dashboard cadence — implemented.** Polling remains at 2.5 seconds
   while data changes, backs off to 5 seconds after three unchanged responses,
   and resets immediately after a change. Hidden and unmounted tabs do not poll;
   request epochs prevent late manual/poll responses from replacing newer data.
4. **Dependency future compatibility.** `proc-macro-error2 2.0.1` currently
   emits Cargo's future-incompatibility warning through the frontend dependency
   graph. Track the upstream Leptos/tooling update and remove the warning through
   a normal dependency upgrade after compatibility tests pass.

No paid service or plan upgrade is authorized by this issue.

## Remaining release gates (2026-09-10)

- Verify native binding availability on the existing account without enabling
  billing; confirm namespace IDs do not collide with another Worker.
- Add authenticated principal limits if abuse data justifies them. The outer
  source budgets now allow 120 claim, 60 deposit and 120 webhook requests/minute
  to accommodate shared venue NAT while auth remains 20/minute.
- Finish wallet email provenance (#062).
- Apply migration and deploy only after these release gates are resolved.

Dashboard errors now return failure instead of fabricated zero totals. The
frontend keeps the last successful timestamp on failure and ignores stale or
post-unmount completions. Chromium lifecycle and attendee inbox coverage passes
with mocked APIs. Claim-token logs now use SHA-256 fingerprints (#064).
