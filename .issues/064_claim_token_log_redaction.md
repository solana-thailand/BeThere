# 064 — Remove claim-token capability values from logs

Status: implemented locally — pending full release validation

The 2026-09-10 core-service log scan found direct `claim_token = %token` fields
across claim locking, mint lookup/execution, quiz, check-in, adventure, and
Sheets synchronization. A claim token appears in public claim URLs and is used
to locate an attendee's claim state, so raw values should not cross into Worker
logs even when the log entry is informational.

## Required implementation

1. Add one shared secret-fingerprint helper using SHA-256 and return a short,
   fixed-length hexadecimal correlation value. Do not use reversible masking or
   Rust's non-cryptographic `DefaultHasher` for this security boundary.
2. Replace every structured log field that emits `token` or `claim_token` with
   the fingerprint. Keep raw values only in storage keys, bound SQL parameters,
   route handling, and provider requests where functionally required.
3. Add a source guard that rejects raw claim-token tracing fields across
   `worker/src`, with a narrow allowlist only if a future sink demonstrably
   redacts values itself.
4. Confirm error strings do not interpolate the raw token and run the complete
   claim, quiz, check-in, and integration suites.

Current audit command:

```sh
rg -n 'claim_token\s*=\s*%|token\s*=\s*%' worker/src
```

This change requires no paid service or Cloudflare plan upgrade.

## Implementation status (2026-09-10)

- Added a shared SHA-256 helper that exposes only the first 64 digest bits as
  16 lowercase hexadecimal characters for log correlation.
- Replaced raw claim-token fields and interpolated adventure-handler messages
  with `claim_token_fingerprint` fields.
- Added a recursive source guard plus helper behavior tests.
- Production deployment remains blocked on the broader release gates in #063.
