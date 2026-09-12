# 071 — Capability tokens travel in URL paths, so the platform logs them

**Status:** Decided and implemented locally — **option 2**. Needs staging validation before merge.
**Priority:** P2 security/privacy (residual from Issue 070, not a regression)
**Created:** 2026-09-12

## Problem

Claim, quiz and adventure capability tokens are carried in the URL path:

```
GET  /api/claim/{token}
POST /api/claim/{token}
GET  /api/quiz/{token}/status
POST /api/quiz/{token}/submit
GET  /api/adventure/{token}/status
POST /api/adventure/{token}/save
```

Issue 064 removed those tokens from the Worker's own tracing fields, and Issue
070 slice 4 stopped `middleware/correlation.rs` from re-adding them by logging
the raw request path. Neither reaches the layer that still records them: the
**platform** request log. Locally that is visible as

```
[wrangler:info] GET /api/claim/0feed070-dead-7eef-8eef-0000000c1a1b 200 OK (11ms)
```

and in production it is Cloudflare's per-request metadata in Workers Logs. Anyone
with read access to Workers Logs can therefore replay a claim within the token's
lifetime. The same applies to `/api/wallet/{address}/nfts`, which exposes a
wallet address rather than a capability.

This is inherent to putting a secret in a URL: it also lands in browser history,
in any `Referer` header the claim page emits, and in intermediate proxy logs.

## What is already true

- The Worker's structured log stream is clean (Issue 070, verified by
  `scripts/verify/pii_log_probe.sh`).
- Claim tokens are single-purpose and bound to one attendee row, and the claim
  path takes a D1/DO lock, so a replay after a successful claim fails.
- There is no evidence of misuse. This is a design residual found during
  verification, not an incident.

## Options

1. **Accept and document.** Treat Workers Logs as a privileged surface with the
   same handling as the D1 audit log, and rely on the token being single-use.
   Cheapest; leaves a live capability in a log with a broader reader set than
   intended.
2. **Shorten exposure.** Give claim tokens an expiry (or make the claim URL
   exchange the path token for a short-lived body token on first load). Keeps
   the existing UX and URL shape, bounds the replay window.
3. **Move the token out of the path.** Serve `/claim` as an unauthenticated page
   that reads the token from the fragment (`#`, never sent to the server) and
   posts it in the request body. Strongest; touches the frontend claim flow, the
   QR payloads already in circulation, and any printed/emailed links.

Recommendation: **2**, with 3 considered for the next claim-flow change. Option 1
alone understates the exposure, because a capability in a log is materially
different from an identifier in a log.

## Scope guard

- Do **not** widen this into another sweep of the Worker log stream; that is
  Issue 070 and it is verified.
- Any change to the claim URL shape must account for QR codes already printed or
  distributed for past events.

## Acceptance criteria

- [ ] An explicit, recorded decision between the three options above.
- [ ] If 2 or 3: the chosen mechanism is implemented, covered by a test, and the
      replay window is stated in `docs/operator-handover.md`.
- [ ] If 1: the operator handover states that Workers Logs contains live claim
      capabilities and who may read it.

## References

- [Issue 064](064_claim_token_log_redaction.md)
- [Issue 070](070_worker_log_pii_redaction.md) — slice 4 documents the residual
- `worker/src/middleware/correlation.rs` — `redact_path`
- `scripts/verify/pii_log_probe.sh`

---

## Decision (2026-09-13) — option 2, with option 3 still the real fix

Owner approved acting on the recommendation on record. Option 1 alone understates
the exposure: a live capability in a log is materially different from an
identifier in a log. Option 3 is the correct end state but touches the frontend
claim flow, QR payloads already printed, and links already emailed — out of scope
for a security fix that should land now.

**Implemented: a bounded replay window, anchored on check-in.**

### Why check-in and not issuance

The token is a UUID v7 minted at *registration* (`handlers/register/signup.rs`)
and reused verbatim at check-in (`handlers/checkin.rs` mints one only if absent).
A TTL measured from issuance — which UUID v7 makes free, since its first 48 bits
are the issuance millisecond — would start ticking weeks before the event and
expire self-registered attendees mid-event. Check-in is when the capability
becomes meaningful: it is when the claim QR is generated, and both the claim and
quiz paths already require a checked-in attendee.

A token whose attendee has not checked in is **not** expired: it has no anchor,
and the existing "you must be checked in first" guards already make it useless.

### How it is enforced

`ClaimTokenPolicy` (`worker/src/claim/ttl.rs`) is a **required parameter** of
both token→attendee resolvers, not an internal default:

- `db::attendees::get_attendee_by_claim_token`
- `db::attendees::get_attendee_with_claim_counts`

So the compiler — not a lint, not review — stops a new resolution path from
skipping the window. Adding the parameter surfaced exactly six call sites; five
take `AppState::claim_token_policy()`, and `handlers/attendee/delete.rs` takes
`ClaimTokenPolicy::unrestricted()` because admin deletion by token must find the
row whatever its age and grants no capability.

The **Sheets fallbacks matter and are covered.** An expired token makes the D1
branch return `None`, which falls through to the Sheets lookup — without a filter
there, the fallback would hand back the very attendee the window just withheld.
Both fallbacks in `sheets/mod.rs` now apply the same policy. This is the
recurring shape in this repo: a guard lands on one path and the sibling keeps the
behaviour.

An expired token is reported as **not found**, identical to an unknown token, so
a replay cannot distinguish a stale capability from a typo.

### Failure modes, chosen deliberately

| case | behaviour | why |
|---|---|---|
| not checked in | never expires | no anchor; existing guards already refuse |
| `checked_in_at` unparseable | fails **open** | a data bug must not lock an attendee out of an NFT they earned; a warning is logged |
| `CLAIM_TOKEN_TTL_SECS = 0` | disabled | operational kill switch |
| negative TTL (misconfiguration) | treated as disabled | must not expire every attendee at once |
| `ttl` + anchor overflow | saturates | cannot panic |

### Covered endpoints

All four token-bearing surfaces resolve through the gated functions: `GET`/`POST
/api/claim/{token}`, `POST /api/quiz/{token}/submit`, `POST
/api/adventure/{token}/save`. `GET /api/quiz/{token}/status` and `GET
/api/adventure/{token}/status` return progress without resolving an attendee at
all — they were already non-capability reads and are unchanged.

### Verification

- 9 unit tests on the pure policy: inside/outside the window, the exact boundary
  (expiry is strictly *after*), no anchor, empty and whitespace anchors,
  unparseable timestamp, zero TTL, negative TTL, overflow.
- New source guard `worker/tests/claim_token_window_guard.rs` pins which files
  may call `unrestricted()`, plus a second test that guards the guard — if
  `unrestricted` is renamed the scan would silently match nothing forever.
  Verified in **both directions**: injecting a compiling bypass into
  `handlers/quiz.rs` makes it fail and name the file; removed, it passes.
  (A first attempt injected a statement at module level, which did not compile,
  so the test never ran and reported nothing — corrected.)
- `cargo clippy --all-targets -D warnings` clean; worker suite 446 passed / 0
  failed (437 before).

### Acceptance criteria

- [x] An explicit, recorded decision between the three options.
- [x] The chosen mechanism is implemented and covered by tests.
- [x] The replay window is stated in `docs/operator-handover.md`.
- [ ] Staging validation: confirm a within-window claim still succeeds end to end.
      Not possible locally — no staging deploy in this session.

### Deploy-day consequence — read before releasing

This is **retroactive**. The window is measured from `checked_in_at`, which is
already on every row, so on the first deploy any attendee who checked in more
than `CLAIM_TOKEN_TTL_SECS` ago loses the ability to claim immediately. There is
no grace period and no backfill that could give one, because the anchor is
historical data, not something the release stamps.

Before releasing, decide which of these applies:

1. **Check the exposure first.** Count rows with `claimed_at IS NULL` and
   `checked_in_at` older than 30 days. If it is zero, ship the default.
2. **Ship wide, tighten later.** Deploy with `CLAIM_TOKEN_TTL_SECS` set past the
   oldest unclaimed check-in, then lower it once those rows are claimed or
   written off. Costs one var change per step, no code change.
3. **Ship disabled.** Deploy with `0`, confirm nothing else regressed, then turn
   it on. Slowest, and leaves the window unbounded in the meantime.

Recommendation: **1**, falling back to **2** if the count is non-zero. The kill
switch (`0`) stays the rollback for either.

### Remaining

- **Option 3 stays open** as the real fix, for the next claim-flow change. This
  bounds the exposure; it does not remove it. Workers Logs retention is days, so
  a 30-day window still leaves a usable replay period for anyone reading them.
- The 30-day default is a judgement call, not a measured one. If claim telemetry
  shows attendees reliably claim within hours, shorten it — the knob is per
  environment and needs no code change.
