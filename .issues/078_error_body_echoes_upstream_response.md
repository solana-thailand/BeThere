# 078 — API error bodies echo raw upstream responses to unauthenticated callers

**Status:** fixed 2026-09-13 (verified locally; not yet deployed)
**Found:** 2026-09-13, while validating Issue 071 against staging
**Severity:** medium (information disclosure; no auth bypass, no data loss)

## What

`AppError::Internal(msg)` renders via `Display` as `internal error: {msg}`
(`domain/src/models/error.rs`), and that string is serialized straight into the
public JSON error body. When `msg` was built by wrapping an upstream failure,
the upstream's URL and response body are handed to the caller.

Observed on staging, unauthenticated, against `GET /api/claim/{token}` with a
token that does not exist:

```json
{"success":false,"error":"internal error: failed to look up claim: HTTP 404 from
https://sheets.googleapis.com/v4/spreadsheets//values/Attendees%21A2%3AAG:
{\n  \"error\": {\n    \"code\": 404, ... }}"}
```

The empty path segment (`spreadsheets//values`) is staging's unset
`PLATFORM_SHEET_ID`. **On production that segment contains the real spreadsheet
ID.**

## Why it matters

`/api/claim/{token}` is a public, unauthenticated endpoint — it has to be, the
claim link is what attendees open. Anyone who can reach it can trigger the D1
miss path with a junk token.

The disclosure is conditional: with a valid sheet ID the Sheets lookup normally
resolves the miss and returns a clean 404, so the leak surfaces only when the
Sheets call itself errors — a quota trip, a permission change, an outage. That
is exactly when an operator is least likely to be watching response bodies.

What leaks:

- the production Google spreadsheet ID (an access-control boundary, not a
  secret, but it is not public either and it names internal infrastructure)
- the exact upstream API, version and range being queried
- upstream error text, verbatim and unbounded

This is the response-body sibling of Issue 070, which cleaned the Worker's
**log** stream. #070 explicitly could not reach anything outside the log; this
is outside the log.

## Scope to check

`Internal` is not the only carrier. `AppError::External { service, status, body }`
formats as `external service error: {service} returned {status}: {body}` and has
the same shape. Every construction site that interpolates an upstream response
into either variant needs review — the claim lookup is where it was noticed, not
necessarily the only one.

## Proposed fix

1. Split the operator-facing detail from the caller-facing message. Keep the
   full upstream text on the `tracing` event (already redacted per #070) and
   return a stable, opaque body — `{"success":false,"error":"internal error"}`
   plus the existing correlation id, which is what an operator actually needs to
   find the log line.
2. Apply the same to `External`: callers get service and status, not body.
3. Add a test asserting no response body for a 5xx contains `https://` or the
   value of a binding, in the spirit of `worker/tests/log_pii_guard.rs`.

Do not fix this by special-casing the Sheets message; the defect is the general
policy of putting `Display` in the body.

## Verification

Reproduce before fixing (any environment, no auth):

```sh
curl -s https://bethere-staging.solana-thailand.workers.dev/api/claim/does-not-exist
```

After the fix that body must not name an upstream host, and the detail must
still be present in the Worker log for the same correlation id. Check both —
a fix that also drops the operator's detail has traded one problem for another.

## Related

- `.issues/070_worker_log_pii_redaction.md` — same class of leak, log stream.
- `.issues/071_capability_token_in_url_platform_logs.md` — found during its
  staging validation; the 500-vs-404 difference is also why that validation
  asserts indistinguishability rather than a fixed status code.

---

## Fix — 2026-09-13

### What changed

`AppError` now renders twice, and the two renderings are different objects
rather than the same string used in two places:

- **`Display`** — unchanged. Operator-facing detail, goes to the `tracing`
  event. This is deliberate: the Verification section below insists a fix must
  not also drop the operator's detail.
- **`AppError::public_message()`** (new, `domain/src/models/error.rs`) — the
  only rendering allowed to leave the Worker.

The split is **by variant**, per the "do not special-case the Sheets message"
note above:

| variant | body the caller now gets |
|---|---|
| `Internal(_)` | `internal error` |
| `External { service, status, body }` | `external service error: {service} returned {status}` — `body` dropped |
| every 4xx | its `Display` string, passed through `redact_urls` |

4xx messages are kept because they tell the caller what *they* did wrong and are
part of the API contract. They are still scrubbed, because a client error can
also be built by wrapping an upstream failure — `escrow/status.rs:185` renders
an RPC rejection as `Validation(format!("escrow not found on-chain: {e}"))`, and
`SolanaConfig::full_rpc_url()` carries the RPC key in its query string. That is
the same defect class in a 400.

`redact_urls` is shape-driven (`scheme://` … up to whitespace, quote or
backslash) rather than a host allowlist, so an upstream added tomorrow is
covered the day it is added — the same reasoning as
`middleware::correlation::redact_path`.

### Second body path, removed

`worker/src/auth.rs` hand-rolled three `ApiResponse` envelopes (two 401s and the
403 staff rejection) instead of going through `WorkerError`. Those bypassed the
redaction entirely and would have kept bypassing it. All three now return
`WorkerError(AppError::…).into_response()`.

**One behaviour change:** the staff rejection body is now
`"forbidden: user is not in staff allowlist"` instead of
`"user is not in staff allowlist"` — the `Display` prefix. Status stays 403.
Nothing in the repo matches on that string (checked frontend and worker).

### Correlation id

Not added to the body. Every response already carries `x-correlation-id` from
`middleware::correlation`, which is what ties the opaque body to the log line
that still holds the detail. Adding it to the envelope as well would need the
request extensions inside `IntoResponse`, which `WorkerError` does not have.

### Tests

- `domain/tests/error_public_message.rs` (11) — pins the body/detail split per
  variant, including the exact production string from this issue, and covers
  `redact_urls` for multiple URLs in one message, quoted URLs inside a JSON
  body, non-`https` schemes and multi-byte input.
- `worker/tests/error_body_guard.rs` (4) — pins the *wiring*: `src/error.rs`
  builds the body from `public_message` and logs `Display`, no file outside it
  hand-rolls an `ApiResponse` error envelope, and no `AppError` is built from a
  literal URL. The hand-rolled-envelope test is what found the three auth.rs
  sites; it will find the next one too.

Full suite green: worker 26 binaries, domain 9, `cargo clippy --workspace
--all-targets -- -D warnings` clean.

## Verification — local runtime, 2026-09-13

Unit tests prove the function; they do not prove the wiring reaches the leak
site. Verified on the real Worker via `wrangler dev --local`, with a seeded
active event whose `sheet_id` is a bogus spreadsheet ID so the Sheets lookup
404s — the exact path in the original report.

Single request, correlation `01a0970e-eec0-7170-a3ca-61edb7293141`:

| | |
|---|---|
| response | `HTTP/1.1 500` · `{"success":false,"error":"internal error"}` |
| log (`src/error.rs:45`) | `internal error: failed to look up claim: HTTP 404 from https://sheets.googleapis.com/v4/spreadsheets/1AbCd…_PROBE/values/Attendees%21A2%3AAG: {"error":{"code":404,…}}` |

The leak path was genuinely reached — the spreadsheet ID and the upstream body
are right there in the log for the same correlation id — and none of it is in
the response. Both halves checked, so this is not a false clean: a run where the
Sheets call was never made would have produced the same clean body for the wrong
reason, and an earlier attempt in this session did exactly that (it 500'd on
`no such table: events` because the fresh persist dir had no migrations).

### Local-harness note

`wrangler dev --local` against the checked-in `.wrangler/state` fails to boot
with `SENTRY_DO SQLite failed; table _cf_ALARM has 3 columns but 2 values were
supplied` — stale persisted DO state versus wrangler 4.99.0's workerd. Work
around it with `--persist-to <fresh dir>` and re-apply migrations there; moving
`cache/` and `observability/` aside is *not* enough. Pre-existing, unrelated to
this issue.

## Still open after this fix

- Nothing is deployed. The fix is in the working tree / branch only.
- `AppError::External`'s `body` is dropped from the response but is **not**
  logged anywhere by `WorkerError` beyond the `Display` string — that is
  intended, but it means a 502's upstream body is only as redacted as #070's
  log guards make it. No new exposure, noted for completeness.
- The `Validation(format!("escrow not found on-chain: {e}"))` sites are now
  URL-scrubbed, but they still hand the caller an RPC error's prose. Worth a
  look if that endpoint's audience ever widens.
