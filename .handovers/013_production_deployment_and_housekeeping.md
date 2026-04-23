# Handover 013: Production Deployment & Housekeeping

**Date**: 2025-06-28
**Branch**: `main`
**Scope**: Merge PR #2, delete stale branches, fix Yarn PnP workaround, document production deployment

---

## What Happened

Completed the final housekeeping items after the Cloudflare Workers migration. The production deployment at `https://bethere.solana-thailand.workers.dev` is live and fully operational. All stale branches were cleaned up, the README was updated, and a `deploy.sh` script was created to permanently fix the Yarn PnP conflict.

---

## Task Summary

| # | Task | Status |
|---|------|--------|
| 1 | Merge PR #2 (README update for Workers-only setup) | ✅ Done |
| 2 | Delete stale remote branches | ✅ Done |
| 3 | Fix Yarn PnP workaround with `deploy.sh` | ✅ Done |
| 4 | Create Handover 013 | ✅ This document |

---

## 1. Merge PR #2

Merged `develop/feature/02_update_readme` into `main` with `--no-ff` merge commit.

**Changes**: README rewritten to remove all Axum/Docker references, add Google Sheet layout table, document `checked_in_by` and sheet-based staff features, update workspace structure, correct test count (27 not 52).

**Commit**: `787a6a7` on `main`.

---

## 2. Branch Cleanup

Deleted stale feature branches (both local and remote):

| Branch | Status |
|--------|--------|
| `develop/feature/01_workers_migration` | ✅ Deleted (merged via PR #1) |
| `develop/feature/02_update_readme` | ✅ Deleted (merged via PR #2) |

Only `main` branch remains on origin.

---

## 3. Yarn PnP Fix — `deploy.sh`

### The Problem

Yarn Plug'n'Play installs a global manifest at `~/.pnp.cjs`. When wrangler's esbuild bundler resolves modules, it reads this manifest, which forbids importing `@cloudflare/unenv-preset` because it's not listed as a dependency of the *global* PnP workspace. This causes:

```
Could not resolve "@cloudflare/unenv-preset/node/process"
The Yarn Plug'n'Play manifest forbids importing "@cloudflare/unenv-preset" here
```

### Root Cause

esbuild has built-in Yarn PnP resolution support. It walks up from `node_modules/wrangler/` and finds `~/.pnp.cjs`, treating it as the project's PnP manifest. The global manifest has no knowledge of `@cloudflare/unenv-preset` being a valid dependency for the worker project.

### What Was Tried

| Approach | Result |
|----------|--------|
| `@cloudflare/unenv-preset` as explicit devDependency | ❌ Still fails — esbuild reads global `~/.pnp.cjs`, not local |
| `YARN_IGNORE_PATH=1` | ❌ Doesn't affect esbuild's built-in PnP resolver |
| `.yarnrc.yml` with `nodeLinker: node-modules` | ❌ esbuild ignores project yarn config |
| Wrangler `[alias]` config | ❌ Can't alias dynamically generated virtual polyfills |

### The Fix

Created `worker/deploy.sh` — a shell script that:

1. Moves `~/.pnp.cjs` to `~/.pnp.cjs.bak` before running wrangler
2. Runs `wrangler deploy` or `wrangler dev`
3. Restores `~/.pnp.cjs` on exit (via `trap EXIT INT TERM`)

**Usage:**

```bash
cd worker
./deploy.sh          # Deploy to production
./deploy.sh dev      # Start local dev server (port 8787)
```

### Files Changed

| File | Change |
|------|--------|
| `worker/deploy.sh` | New — deploy/dev wrapper with PnP workaround |
| `worker/package.json` | Added `@cloudflare/unenv-preset` as explicit devDependency |
| `README.md` | Updated Quick Start and Deployment to reference `deploy.sh` |

---

## Production Deployment Reference

### URLs

| Resource | URL |
|----------|-----|
| Production | `https://bethere.solana-thailand.workers.dev` |
| Health check | `https://bethere.solana-thailand.workers.dev/api/health` |
| Frontend (SPA) | `https://bethere.solana-thailand.workers.dev/` |
| OAuth callback | `https://bethere.solana-thailand.workers.dev/api/auth/callback` |

### Cloudflare Account

| Field | Value |
|-------|-------|
| Account ID | `bb8f9ffa91e24d9ce850cbbc4fd45935` |
| Account subdomain | `solana-thailand` |
| Worker name | `bethere` |

### Secrets (9)

Configured via `npx wrangler secret put <NAME>`:

```
JWT_SECRET
GOOGLE_CLIENT_ID
GOOGLE_CLIENT_SECRET
GOOGLE_REDIRECT_URI
GOOGLE_SERVICE_ACCOUNT_EMAIL
GOOGLE_SERVICE_ACCOUNT_PRIVATE_KEY
GOOGLE_SERVICE_ACCOUNT_TOKEN_URI
GOOGLE_SHEET_ID
STAFF_EMAILS
```

### Non-Secret Vars (3)

In `worker/wrangler.toml` `[vars]`:

| Var | Value |
|-----|-------|
| `SERVER_URL` | `https://bethere.solana-thailand.workers.dev` |
| `GOOGLE_SHEET_NAME` | `checkin` |
| `GOOGLE_STAFF_SHEET_NAME` | `staff` |

### Google Cloud Console

- **OAuth redirect URI** must be set to: `https://bethere.solana-thailand.workers.dev/api/auth/callback`
- **Google Sheet** must have:
  - "checkin" tab with attendee data (columns A–Y, including J for `checked_in_by`)
  - "staff" tab with staff emails in column A

---

## Git History (After Cleanup)

```
4794efa (HEAD -> main, origin/main) fix: add deploy.sh to handle Yarn PnP conflict, update README
787a6a7 Merge branch 'develop/feature/02_update_readme' (PR #2)
b695b55 config: update worker name to bethere, SERVER_URL to solana-thailand subdomain
a20ebc5 Merge pull request #1 — Cloudflare Workers migration
```

Only `main` branch exists on origin.

---

## Build & Test Status

| Check | Result |
|-------|--------|
| `cargo test -p event-checkin-domain` | ✅ 14 passed |
| `cargo test -p event-checkin-worker` | ✅ 13 passed |
| `cargo check -p event-checkin-worker --target wasm32-unknown-unknown` | ✅ Zero errors |
| Production health check | ✅ 200 OK |
| `deploy.sh` deploy | ✅ Works, restores `~/.pnp.cjs` |

---

## Project Structure (Current)

```
event-checkin/
├── .handovers/           # 13 handover documents
├── .issues/              # Empty
├── domain/               # Shared types & logic (x86_64 + wasm32)
├── worker/               # Cloudflare Worker (wasm32-unknown-unknown)
│   ├── src/              #   Rust source (handlers, auth, sheets, crypto, state)
│   ├── build/worker/     #   WASM build output
│   ├── node_modules/     #   wrangler + @cloudflare/unenv-preset
│   ├── deploy.sh         #   Deploy wrapper (Yarn PnP workaround)
│   ├── wrangler.toml     #   Workers config + build pipeline
│   ├── package.json      #   wrangler + unenv-preset devDeps
│   └── .dev.vars         #   Local dev secrets (gitignored)
├── frontend-leptos/      # Leptos WASM frontend
│   ├── src/              #   Pages (admin, scanner, login), API client, utils
│   ├── dist/             #   Built frontend (served by Workers)
│   └── Cargo.toml        #   Standalone (excluded from workspace)
├── Cargo.toml            # Workspace root (members: domain, worker; exclude: frontend-leptos)
├── Cargo.lock
├── .gitignore
└── README.md
```

---

## Reflection — Struggles / Solved

### Yarn PnP is a global affliction
The `~/.pnp.cjs` file is installed by Yarn globally and affects every Node.js tool on the system that uses esbuild's built-in PnP resolver. There's no per-project way to disable it. The `deploy.sh` approach is the most reliable workaround — it's explicit, handles signals (INT/TERM) for cleanup, and doesn't require changes to the global Yarn installation.

### esbuild's PnP resolver cannot be disabled
Tried `YARN_IGNORE_PATH`, `.yarnrc.yml` `nodeLinker`, `npm_config_user_agent` — none of these affect esbuild's hardcoded PnP resolution logic. The resolver walks up from the working directory looking for `.pnp.cjs` and applies whatever it finds. The only way to prevent it is to ensure the file doesn't exist during the build.

---

## Issues Ref

- Handover 012: Legacy cleanup & deployment readiness
- Handover 011: Staff check-in log + sheet-based staff list
- Handover 009: Workers migration (Phases 3–6)
- PR #1: Cloudflare Workers migration (merged)
- PR #2: README update (merged)

---

## How to Dev / Test

```bash
# Verify all crates build + test
cargo check -p event-checkin-domain
cargo test -p event-checkin-domain    # 14 tests
cargo check -p event-checkin-worker --target wasm32-unknown-unknown
cargo test -p event-checkin-worker    # 13 tests

# Start Worker dev server (handles Yarn PnP automatically)
cd worker && ./deploy.sh dev

# Deploy to production (handles Yarn PnP automatically)
cd worker && ./deploy.sh

# Build frontend (if changed)
cd frontend-leptos && trunk build
# Output goes to frontend-leptos/dist/ — served by Workers via [assets]