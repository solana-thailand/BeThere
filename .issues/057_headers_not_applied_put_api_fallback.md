# 057 — `_headers` rules not applied (PUT API fallback treats it as a plain asset)

**Status:** Open (performance-only; correctness is resolved in #057-cache commit)
**Severity:** Low (performance) — NOT a correctness bug after the `spa_fallback` fix
**Area:** `worker/deploy.sh`, Cloudflare Static Assets, caching

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
