# 057 — `_headers` rules not applied (PUT API fallback treats it as a plain asset)

**Status:** RESOLVED (2026-07-27)
**Severity:** Low (performance) — NOT a correctness bug after the `spa_fallback` fix
**Area:** `worker/deploy.sh`, Cloudflare Static Assets, caching

## Resolution (2026-07-27)

Two parts:

1. **Primary path fixed itself (Option 1).** The Cloudflare `/versions` `10013` bug
   recovered, so deploys now use standard `wrangler deploy`, which parses `_headers`
   natively. Verified on prod (Version `7b7d0df0`): `/` and `/sw.js` → `no-store`,
   hashed `*.js/.wasm/.css` → `public, max-age=31536000, immutable`, and `/_headers`
   is NOT a fetchable asset. The original symptom is gone on the live deploy path.

2. **PUT fallback hardened (defense-in-depth).** The manual fallback is still a latent
   path if the versions bug returns. `worker/deploy.sh` now:
   - **Skips `_headers`/`_redirects`** from the uploaded manifest (they were served as
     octet-stream blobs before — the `GET /_headers` symptom).
   - **Sets each asset's real Content-Type** on upload (per-extension MIME + `charset=utf-8`
     on text/*, `application/null` sniff-fallback for unmapped types), mirroring wrangler's
     `syncAssets`. This also closes the content-type *poison* vector behind the 2026-07-26
     octet-stream incident (see handover 132).
   - Backstopped by `verify_content_types()` (added earlier) which fails any deploy that
     serves `/` or the JS bundle as octet-stream.
   - **Known remaining limitation:** the raw PUT API has no field for `_headers` rules, so
     a *fallback* deploy still gets Cloudflare's default `max-age=0, must-revalidate` on
     static assets (correct — always revalidates, never stale — just not immutable-cached).
     Perf-only, fallback-only; documented inline at the `asset_config` in deploy.sh.

Reviewed adversarially (2 lenses: Python correctness + Cloudflare API contract) — both sound.

## Symptom

Content-hashed static assets served by the Cloudflare Static Assets binding
return the **default** `Cache-Control: public, max-age=0, must-revalidate`
instead of the custom rules in `frontend-leptos/_headers`:

```
GET /                  → max-age=0, must-revalidate   (wanted: no-store)
GET /sw.js             → max-age=0, must-revalidate   (wanted: no-store)
GET /event-checkin-frontend-*.js → max-age=0, must-revalidate (wanted: immutable)
```

Confirmed by `curl -sI` against production (2026-06-24). Additionally,
`GET /_headers` returns HTTP 200 with `content-type: application/octet-stream`,
proving the file was uploaded as a **regular asset** rather than parsed as config.

## Root cause

Cloudflare parses `_headers`/`_redirects` as special config files only during
`wrangler deploy` (via the `/versions` API). The `/versions` API has a recurring
`10013` bug, so `worker/deploy.sh` falls back to the manual PUT
`/workers/scripts/{name}` API. That fallback walks `dist/` and uploads **every**
file as a plain static asset — including `_headers`, which is then stored as an
asset blob and never parsed for header rules.

Cloudflare docs confirm `_headers` rules are applied to Static Asset responses
only; Worker-generated responses are exempt regardless.

## Why this is NOT a correctness bug anymore

The blank-page-after-deploy bug had two surfaces, both involving the version-
critical `index.html` shell:

| Route | Served by | Before | After |
|-------|-----------|--------|-------|
| `/` | Static Asset | `max-age=0, must-revalidate`, **no ETag** | unchanged (always re-fetches → always fresh) |
| `/sw.js` | Static Asset | `max-age=0, must-revalidate`, ETag | unchanged (revalidates → picks up new version) |
| `/claim/*`, `/staff`, `/admin` | Worker `spa_fallback()` | **no cache-control at all** → heuristic-caching danger | **`no-store` + security headers** (fixed in commit `55f6acd`) |

The dangerous surface — Worker SPA routes with no cache-control — is fixed.
Asset routes already force revalidation/re-fetch, so they cannot serve a stale
shell. Remaining gap is purely that hashed assets revalidate every load instead
of caching immutably (extra conditional requests, no stale content).

## Fix options (when prioritized)

1. **Preferred — restore `wrangler deploy`.** When the Cloudflare `/versions`
   `10013` bug recovers, `wrangler deploy` will parse `_headers` natively and
   immutable caching returns automatically. No code change needed; just deploy.
2. **Alternative — `run_worker_first = true`.** Route all requests through the
   Worker; fetch assets via `env.ASSETS.fetch()` and set `Cache-Control` per
   extension (immutable for hashed `*.js/.wasm/.css`, `no-store` for the shell).
   Decouples caching from `_headers` entirely. Larger refactor; validates that
   the worker fetches assets correctly. Add to wrangler.toml `[assets]`
   `run_worker_first = true` and re-add `ASSETS` binding usage in `lib.rs`.
3. **Accept the overhead.** Current default is correct (no stale content), just
   not maximally efficient. Defer until the versions API recovers.

## Verification commands

```sh
# Asset route (should show no-store when fixed):
curl -sI https://bethere.solana-thailand.workers.dev/ | grep -i cache-control

# Worker SPA route (ALREADY fixed → no-store):
curl -sI https://bethere.solana-thailand.workers.dev/claim/<token> | grep -i cache-control

# _headers should NOT be a fetchable asset once parsed as config:
curl -sI https://bethere.solana-thailand.workers.dev/_headers | grep HTTP
```

## Related

- Commit `55f6acd` — `fix(cache): set no-store + security headers on worker SPA fallback responses`
- Commit `58cc8bb` — added `_headers` + trunk `copy-file` directive (works for the build; blocked only by the deploy path)
- `worker/deploy.sh` — PUT API fallback (asset manifest walks `dist/` naively)
