# 144: Asset-first pages ship without security headers (CSP, HSTS, X-Frame-Options)

**Status:** deployed 2026-09-24 (prod version `36eae0db`, git `897aa07`). Fixed on `develop` 2026-09-23 and verified on staging.
**Found by:** the `.issues/142`/`143` staging verification (session 5, 2026-09-23).
**Severity:** high if it had shipped. Every HTML page would lose its CSP and
`X-Frame-Options: DENY`, so clickjacking protection and XSS containment would
be off on the ticket, login and admin pages. It never reached prod.

## What happened

`7ed1c07` (perf: serve the wasm pre-compressed) changed `[assets]
run_worker_first` from the default to the array
`["/api/*", "/event-checkin-frontend-*_bg.wasm"]`. Once it is an array, every
path it does not match is served **asset-first**. That includes `/`, `/ticket/*`,
`/login` and every other SPA navigation. Those responses never pass through
`security_headers_layer` (`worker/src/middleware/headers.rs`), and
`frontend-leptos/_headers` only set `Cache-Control`.

This is the same trap as `[[run-worker-first-array-swallows-api]]`: an array
silently re-routes everything that is not in it. The earlier fix covered
`/api/*`; the headers were the second casualty. `deploy.sh`'s smoke test checks
Content-Type, not security headers, so it stayed green.

Evidence, 2026-09-23 ~16:00Z, from `curl -D -`:

| Path | Prod (old version) | Staging (`develop`) |
|---|---|---|
| `/ticket/x` | CSP, HSTS, XFO, nosniff | none |
| `/` | none (asset-first on prod too) | none |
| `/api/health` | all | all |

Prod's `/` has lacked them for longer. It was already asset-first before the
array, because `index.html` is a real asset.

## Fix

- `SECURITY_HEADERS` in `worker/src/middleware/headers.rs` is now one
  `(name, value)` table. `add_security_headers` loops over it. It is
  re-exported from the crate root.
- `frontend-leptos/_headers` mirrors it under `/*`. Matching `_headers` rules
  merge, so the existing `Cache-Control` rules still apply.
- `worker/tests/security_headers_parity.rs` fails if the `/*` block and the
  table differ. I checked it can fail by changing `X-Frame-Options` to
  `SAMEORIGIN`, which made it go red.

## Verification (staging, deploy tag `deploy/staging/20260923T160500Z`)

- `/`, `/ticket/x`, `/login` and `/events` all carry CSP, HSTS, XFO and COOP.
  I sampled 6 times per path, and also with query strings.
- The first probe right after the deploy returned none. Later probes were
  consistent, so this was edge propagation.
- The landing page renders under headless Chrome with the CSP in place.

## Remaining

- [ ] Prod: after the next prod deploy, re-run the `curl -D -` table above.
      `/` should now have headers too.
- [x] `deploy.sh` now runs `verify_security_headers` after the Content-Type
      check: `/` (asset), `/ticket/_smoke` (SPA fallback), `/api/health`
      (Worker) must carry CSP + X-Frame-Options + HSTS. Strict on the wrangler
      path, warn-only on the PUT fallback (cannot upload `_headers`, #057).
      Checked both directions 2026-09-23: staging → pass (rc 0); prod → fails
      on `/` (rc 1), the live pre-existing gap above.
