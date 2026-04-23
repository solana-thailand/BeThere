# Handover 009: Workers Migration Phases 3-6 — Crypto, HTTP, Handlers, Deploy Prep

**Date**: 2025-06-28
**Branch**: `develop/feature/01_workers_migration` (off `main` `fc1a473`)
**Scope**: SubtleCrypto bridge, HTTP client, all handler ports, clippy fixes, README, deploy readiness

---

## What Happened

Implemented Phases 3 through 5 of the Cloudflare Workers migration plan outlined in Handover 008, then progressed Phase 6 (deploy preparation). The Worker crate now has full API parity with the Axum build — all endpoints ported, crypto works via SubtleCrypto, outbound HTTP via `worker::Fetch`. The Worker compiles to `wasm32-unknown-unknown` with zero errors and zero clippy warnings.

Phase 6 progress: `wrangler dev` verified working (local server starts on port 8787), README rewritten with Workers deployment instructions, `.gitignore` updated, all clippy warnings fixed across all three crates.

### Phase 3: Crypto Bridge (`worker/src/crypto.rs`)

Replaced `rsa` + `jsonwebtoken` crates (which don't compile to WASM) with V8 SubtleCrypto via `wasm-bindgen` + `js-sys`.

Key implementations:
- `sign_rs256()` — RSA-SHA256 signing: PEM → DER → `SubtleCrypto.importKey("pkcs8")` → `SubtleCrypto.sign("RSASSA-PKCS1-v1_5")`
- `sign_jwt_assertion()` — Builds full JWT `header.payload.signature` for Google service account auth
- `create_jwt()` / `verify_jwt()` — HMAC-SHA256 session tokens: `SubtleCrypto.importKey("raw")` → `SubtleCrypto.sign("HMAC")` with constant-time byte comparison
- `pem_to_der()` — Strips PEM markers, decodes base64 to DER bytes
- `subtle_call()` — Generic helper that dispatches `crypto.subtle.${method}(...args)` with up to 5 positional args, reducing repetitive JS interop boilerplate from ~30 lines per call to ~5

Design decision: `Object::try_from(&JsValue)` returns `Option<&Object>`, so we `.cloned()` to get owned `Object`. This is fine — Workers is single-threaded, no real ownership concerns.

### Phase 4: HTTP Client (`worker/src/http.rs`)

Replaced `reqwest::Client` with typed wrappers around `worker::Fetch`.

Key implementations:
- `get_json<T>()` — GET with Bearer token, parse JSON response
- `post_form<T>()` — POST with `application/x-www-form-urlencoded` body
- `post_json<T>()` — POST with JSON body (unused but kept for future use)
- `put_json()` — PUT with JSON body and Bearer token
- `check_status()` — Status code validation, reads body on error for diagnostics
- `exchange_oauth_code()` — Google OAuth token exchange
- `fetch_user_info()` — Google userinfo endpoint
- `exchange_jwt_assertion()` — Service account JWT → access token
- `fetch_sheet_range()`, `update_sheet_range()`, `batch_update_sheet()` — Google Sheets API

API difference from reqwest: `worker::Fetch` uses `RequestInit` + `Headers` objects instead of builder pattern. Response methods (`json()`, `text()`) take `&mut self` because the body stream is consumed.

### Phase 5: Handlers + Auth + Sheets + Middleware

Ported all Axum handlers, auth logic, sheets operations, and security middleware to the Worker crate.

Files created:
- `worker/src/auth.rs` — OAuth URL generation, callback handling, JWT session management, auth middleware (`require_auth`)
- `worker/src/sheets.rs` — Google Sheets operations (get attendees, mark checked in, update QR URLs) using SubtleCrypto for JWT signing and `worker::Fetch` for HTTP
- `worker/src/middleware.rs` — Security headers middleware (identical headers to Axum build)
- `worker/src/handlers/auth.rs` — `/api/auth/url`, `/api/auth/callback`, `/api/auth/me`, `/api/auth/logout`
- `worker/src/handlers/attendee.rs` — `/api/attendees`, `/api/attendee/{id}`
- `worker/src/handlers/checkin.rs` — `/api/checkin/{id}`
- `worker/src/handlers/qr.rs` — `/api/generate-qrs`
- `worker/src/handlers/mod.rs` — Full route router with auth middleware layer
- `worker/src/lib.rs` — Updated to wire all modules + security headers layer

### Phase 6: Deploy Preparation

- **wrangler dev verified** — Local server starts successfully on `http://localhost:8787` with static assets from `frontend-leptos/dist/`
- **Clippy fixes** — All three crates now have zero warnings:
  - `domain/src/models/attendee.rs`: Implemented `FromStr` trait for `CheckInStatus` instead of `from_str` method (clippy `should_implement_trait`)
  - `worker/src/crypto.rs`: Changed `.map(|o| o.clone())` to `.cloned()` (clippy `map_clone`)
  - `worker/src/auth.rs`: Collapsed nested `if let` into `if let ... && let ...` chain (clippy `collapsible_if`)
  - `src/models/attendee.rs`: Same `FromStr` fix as domain
- **README rewritten** — Added Workers deployment section, workspace structure, secret configuration, build instructions, and updated API endpoint table
- **`.gitignore` updated** — Added `worker/build/`, `worker/.wrangler/`, `worker/.dev.vars`

---

## Key Technical Decisions

### `#[worker::send]` on All Async Handlers

WASM futures don't implement `Send` (single-threaded runtime), but Axum's `Handler` trait requires `Send`. The `worker` crate provides `#[worker::send]` which wraps the function body in `worker::send::SendFuture` (unsafe impl `Send`). Applied to:
- All 7 handler functions (`auth_url`, `auth_callback`, `auth_me`, `auth_logout`, `list_attendees`, `get_attendee`, `check_in`, `generate_qrs`)
- Auth middleware (`require_auth`)

### Concrete Return Types for Axum Compatibility

`auth_callback` and `auth_logout` return `axum::response::Response` instead of `impl IntoResponse`. The opaque `impl` return type prevented Axum from resolving the `Handler` trait — the compiler couldn't determine the concrete future type at compile time. Using `Response` directly (via `.into_response()`) gives Axum a concrete type.

### `subtle_call()` Generic Dispatch

Instead of repeating `Reflect::get → Function::from → callN → JsFuture::from` for every SubtleCrypto operation (importKey, sign, verify), extracted a generic `subtle_call(method, args)` helper. Matches on args length (0-5) to call the right `callN` method. Reduced each SubtleCrypto invocation from ~15 lines to ~5.

### Axum Features for WASM

Worker uses `axum` with `default-features = false` (no `tokio`, no `http1`, no `tracing`). Required features:
- `macros` — `#[debug_handler]` support
- `json` — `Json<T>` extractor
- `query` — `Query<T>` extractor
- `matched-path` — `MatchedPath` extractor (needed for multi-extractor handlers)
- `original-uri` — `OriginalUri` extractor

### `FromStr` instead of `from_str` method

Clippy flagged `CheckInStatus::from_str` as conflicting with the standard `std::str::FromStr` trait. Implemented the trait properly with `type Err = std::convert::Infallible` (since parsing never fails — unknown values default to `PendingApproval`). Callers updated from `CheckInStatus::from_str(s)` to `s.parse::<CheckInStatus>().unwrap()`.

### Yarn PnP Workaround

The `~/.pnp.cjs` Yarn Plug'n'Play manifest interferes with wrangler's esbuild bundler. Wrangler resolves `../../.pnp.cjs` from `worker/node_modules/wrangler/` to `~/.pnp.cjs`, which then forbids importing `@cloudflare/unenv-preset`. Workaround: temporarily rename `~/.pnp.cjs` before running `wrangler dev` or `wrangler deploy`, then restore after. This only affects local dev; remote deploy may not be affected depending on the environment.

---

## Build & Test Status

| Check | Result |
|-------|--------|
| `cargo check` (main Axum) | ✅ Zero errors |
| `cargo test` (main Axum) | ✅ 25 passed |
| `cargo test -p event-checkin-domain` | ✅ 14 passed |
| `cargo check -p event-checkin-worker --target wasm32-unknown-unknown` | ✅ Zero errors |
| `cargo test -p event-checkin-worker` (native) | ✅ 13 passed |
| `cargo clippy --all-targets` | ✅ Zero warnings |
| `wrangler dev` (local) | ✅ Starts on port 8787 |

Total tests: **52** (25 main + 14 domain + 13 worker)

---

## File Map — New/Modified

```
worker/src/
├── lib.rs              # Updated: wires all modules + security headers
├── state.rs            # Pre-existing: AppState from Env bindings
├── crypto.rs           # NEW: SubtleCrypto bridge (RSA-SHA256, HMAC-SHA256, JWT)
├── http.rs             # NEW: HTTP client wrapping worker::Fetch
├── auth.rs             # NEW: OAuth, JWT, staff verification, auth middleware
├── sheets.rs           # NEW: Google Sheets API operations
├── middleware.rs       # NEW: Security headers middleware
└── handlers/
    ├── mod.rs          # Updated: full route router with auth middleware
    ├── health.rs       # Pre-existing: health check
    ├── auth.rs         # NEW: OAuth URL, callback, me, logout
    ├── attendee.rs     # NEW: list attendees, get attendee
    ├── checkin.rs      # NEW: check-in attendee
    └── qr.rs           # NEW: bulk QR generation

worker/Cargo.toml       # Updated: added base64, url, urlencoding, query+matched-path+original-uri features
worker/wrangler.toml    # Pre-existing: build command, assets, secrets config
worker/package.json     # Pre-existing: local wrangler dependency

domain/src/models/attendee.rs  # Updated: FromStr trait for CheckInStatus
src/models/attendee.rs         # Updated: FromStr trait for CheckInStatus
README.md                      # Updated: Workers deployment, workspace structure, testing
.gitignore                     # Updated: worker build artifacts
Cargo.toml                     # Updated: workspace members
Cargo.lock                     # Updated: new dependencies
```

---

## Where Is the Plan/Code/Test

**Plan**: Handover 008 — 6-phase Cloudflare Workers migration. Phases 1-2 were pre-existing scaffold + shared domain. Phases 3-5 are the main implementation. Phase 6 is deploy prep.

**Code**: `worker/src/` — 8 Rust source files totaling ~1200 lines of new code.

**Tests**:
- Worker unit tests (13): `crypto::tests` (6), `auth::tests` (5), `sheets::tests` (1), plus domain tests inherited via shared crate
- Main build tests (25): unchanged, all pass — zero regressions
- Domain tests (14): unchanged, all pass
- No integration tests yet (manual browser testing needed)

---

## Reflection — Struggles / Solved

### `Handler` trait not satisfied
**Struggle**: All handlers with `State` + `Extension<Claims>` + `Path`/`Query` extractors failed with "trait Handler<_, _> is not implemented". No clear error message — just the opaque trait bound failure.

**Root cause**: Two issues combined:
1. WASM futures are `!Send` — Axum requires `Send`. Fixed with `#[worker::send]`.
2. `impl IntoResponse` return type prevented type inference. Fixed by returning concrete `Response`.

### `Object::try_from` signature mismatch
**Struggle**: `Object::try_from(JsValue)` failed — expected `&JsValue`, found `JsValue`.

**Root cause**: `js_sys::Object::try_from` takes `&JsValue` and returns `Option<&Object>` (not owned). Fixed with `Object::try_from(&val).cloned()`.

### Yarn PnP vs wrangler esbuild
**Struggle**: `wrangler dev` fails with "Yarn Plug'n'Play manifest forbids importing @cloudflare/unenv-preset".

**Root cause**: `~/.pnp.cjs` is resolved by esbuild from `worker/node_modules/wrangler/` → `../../.pnp.cjs` → `~/.pnp.cjs`. The PnP manifest doesn't list `@cloudflare/unenv-preset` as a dependency of wrangler.

**Workaround**: Temporarily rename `~/.pnp.cjs` before running wrangler. This is an environmental issue, not a code issue. Deploy (`wrangler deploy`) builds remotely and may not be affected.

### `response.status()` vs `response.status_code()`
**Struggle**: `response.status()` method not found on `worker::Response`.

**Root cause**: `worker::Response` uses `status_code()` (returns `u16`), not `status()` (Axum's `StatusCode`). Different API.

---

## Remain Work

### Phase 6: Integration Testing & Deploy (continued)

| Step | Description | Done? |
|------|-------------|-------|
| 6.1 | End-to-end browser test: login → scan → confirm → scan again | ☐ |
| 6.2 | Mobile testing: Safari iOS + Chrome Android camera scanner | ☐ |
| 6.3 | Set up `wrangler.toml` secrets (`JWT_SECRET`, `GOOGLE_SERVICE_ACCOUNT_*`, etc.) | ☐ |
| 6.4 | Configure custom domain or `*.workers.dev` | ☐ |
| 6.5 | `npx wrangler deploy` — go live | ☐ |
| 6.6 | Remove `Dockerfile`, `docker-compose.yml`, legacy `frontend/` (optional cleanup) | ☐ |

### Pre-deploy Checklist

1. **Build frontend**: `cd frontend-leptos && trunk build` — ensure `dist/` is fresh
2. **Set secrets**: Run `npx wrangler secret put <NAME>` for each of the 9 secrets
3. **Update `SERVER_URL`** in `wrangler.toml` `[vars]` to the actual Workers URL
4. **Deploy**: `cd worker && mv ~/.pnp.cjs ~/.pnp.cjs.bak && npx wrangler deploy && mv ~/.pnp.cjs.bak ~/.pnp.cjs`
5. **Test**: Visit the deployed URL, verify OAuth login, attendee listing, QR generation

### Known Risks for Deployment

1. **SubtleCrypto at runtime** — All crypto tests are pure-logic (base64, PEM parsing). The actual SubtleCrypto calls (`sign_rs256`, `hmac_sha256`) can only be tested in a real Workers environment. May discover issues with key format, algorithm parameters, or ArrayBuffer handling at runtime.
2. **`worker::Fetch` body streaming** — The `response.text()` and `response.json()` methods consume the response body. If `check_status` reads the body for error diagnostics, subsequent `.json()` calls may fail. Currently passing `&mut Response` to handle this, but worth verifying.
3. **Cookie handling** — Workers serve static assets from the same origin. The `HttpOnly; Path=/api` cookie should work but needs browser testing to confirm the cookie is actually sent on API requests from the SPA.
4. **Yarn PnP on deploy** — `wrangler deploy` may hit the same PnP issue as `wrangler dev`. If so, the workaround is the same: temporarily rename `~/.pnp.cjs`.

---

## Issues Ref

- Handover 008: Migration plan (6 phases), system review, dependency matrix
- Handover 007: Leptos 0.8 upgrade, camera QR scanner
- Handover 006: Admin badges, non-staff rejection, QR force regenerate

---

## How to Dev / Test

### Main Axum Build (unchanged)
```bash
cargo check
cargo test          # 25 tests
cargo clippy
RUST_LOG=info cargo run  # port 3000
```

### Domain Crate (shared)
```bash
cargo test -p event-checkin-domain  # 14 tests
```

### Worker Crate
```bash
# Check WASM build
cargo check -p event-checkin-worker --target wasm32-unknown-unknown

# Run pure-logic tests (native, no WASM runtime needed)
cargo test -p event-checkin-worker  # 13 tests

# Local dev server (needs wrangler + wasm-bindgen CLI)
cd worker
# Workaround: mv ~/.pnp.cjs ~/.pnp.cjs.bak
npx wrangler dev  # port 8787
# Restore: mv ~/.pnp.cjs.bak ~/.pnp.cjs

# Deploy
npx wrangler deploy
```

### Frontend
```bash
cd frontend-leptos
CARGO_BUILD_JOBS=1 trunk build       # build WASM → dist/
trunk serve --port 3001               # dev with hot-reload
```

### Full Clippy (zero warnings expected)
```bash
cargo clippy --all-targets
```
