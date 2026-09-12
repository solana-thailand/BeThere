# 078 — API error bodies echo raw upstream responses to unauthenticated callers

**Status:** open
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
