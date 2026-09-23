# 147: Public `/api/health` returns D1 row counts and runs six COUNT(*) scans per hit

**Status:** fixed on develop 2026-09-24. Not deployed. After the deploy,
re-run the curl below on prod and confirm `d1` is `{"connected":true}`.
**Found by:** the gist-rs study sub-agent (session `event-checkin-2f`) noticed
it on staging. The prod read and the D1 cost estimate are from this session.
**Severity:** low-medium. Business-volume disclosure (ISO 27001 A.8.12), plus
an unauthenticated way to spend the free-plan D1 read quota (A.8.6).

## What happens

`GET /api/health` (`worker/src/handlers/health.rs`, public router, `no-cache`)
runs one statement with six sub-selects:
`COUNT(*)` over `attendees`, `contacts`, `events`, `staff`, `claim_locks` and
`audit_log`, and returns the numbers. Prod on 2026-09-24:

```
curl -s https://bethere.solana-thailand.workers.dev/api/health
→ "d1":{"connected":true,"counts":{"attendees":521,"audit_log":152,
   "claim_locks":4,"contacts":196,"events":16,"staff":0}}
```

- **Disclosure:** anyone can read the attendee and contact totals and watch
  them change over time (registrations per hour, audit activity).
- **Cost:** D1 bills rows read, and a COUNT(*) reads every row. That is about
  890 rows per hit today, and it grows with the tables. The free plan allows
  5 M rows read per day, so about 5,600 anonymous hits (one request every
  15 s) would use the whole day's quota and stop every D1 read in the app.
  This is an estimate from the counts above; it was not measured with
  `wrangler d1 insights`.

The frontend reads only `cluster` from this endpoint. `dev_mode` and the
Solana readiness block are configuration posture, not data; they stay for
now (see "Not in scope").

## Fix

- Health probes connectivity with `SELECT 1` (0 rows read) and returns
  `"d1":{"connected":bool}` only.
- `worker/tests/public_health_no_counts.rs` fails if `health.rs` regains a
  `COUNT(` or a `counts` field.
- `scripts/d1/validate_d1.sh` (a legacy Issue 046 script) read the counts
  from health. It now reads them with a read-only
  `wrangler d1 execute --remote --json`.

## Not in scope (noted)

- `dev_mode` and the Solana cluster/readiness warnings are still public. The
  frontend needs `cluster`; the rest could move behind admin auth.
- `validate_d1.sh --seed` still `INSERT`s an `events` row with
  `d1 execute`, which the KV-first read path can mask (CLAUDE.md data rules).
  Don't run `--seed` against prod.
