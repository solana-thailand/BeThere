# Handover 008: System Review & Cloudflare Workers Migration Plan

**Date**: 2026-04-23
**Branch**: main (`fc1a473`)
**Scope**: Full codebase review + Cloudflare Workers migration roadmap

---

## What Happened

Comprehensive system review after 17 commits in session 007 (Leptos 0.8 upgrade, camera QR scanner, Docker, attendee name fix). Verified backend health (25 tests, zero clippy warnings), reviewed all key files, confirmed working tree is clean. Then researched Cloudflare Workers feasibility — decided to go with **full Workers rewrite** using `workers-rs` with the `http` feature (Axum-on-Workers).

### System Health Summary (Current Axum Build)

| Check | Status |
|-------|--------|
| `cargo check` | ✅ Zero errors |
| `cargo test` | ✅ 25 passed, 0 failed |
| `cargo clippy` | ✅ Zero warnings |
| `git status` | ✅ Clean working tree |
| `git branch` | `main` only (no stale branches) |
| WASM build artifact | ✅ `dist/` present (1.4MB) |
| `.env.example` | ✅ Complete with all vars documented |
| `Dockerfile` | ✅ 3-stage build (WASM + backend + runtime) |
| `docker-compose.yml` | ✅ Healthcheck, resource limits, env file |

---

## Decision: Cloudflare Workers Migration

### Why Workers

- Edge deployment, global latency, zero cold starts
- Free tier: 100K requests/day, generous for event check-in
- Static assets served from same origin (SPA + API in one Worker)
- HTTPS by default — solves the `getUserMedia` camera requirement
- No Docker, no TLS termination, no infrastructure management

### Key Technical Finding: Axum-on-Workers Works

`workers-rs` 0.0.21+ has an `http` feature flag that bridges `http::Request` ↔ Axum directly:

```rust
use worker::*;
use axum::Router;

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> Result<http::Response<axum::body::Body>> {
    Ok(router().call(req).await?)
}
```

This means the **Axum router and handlers can be largely preserved** — only the entry point and HTTP client layer change.

### Dependency Migration Matrix

| Current Crate | Used For | Workers Replacement | Risk |
|---------------|----------|---------------------|------|
| `axum` 0.8 | HTTP router + handlers | Same — via `workers-rs` `http` feature | 🟢 Low |
| `tokio` 1 (full) | Async runtime, TCP, signals | `wasm-bindgen-futures` (no tokio) | 🟡 Medium |
| `reqwest` 0.12 (`rustls-tls`) | Google Sheets API, OAuth | `worker::Fetch` or `reqwest` WASM mode | 🟡 Medium |
| `rsa` 0.9 + `sha2` | Service account RS256 JWT signing | `web_sys::SubtleCrypto` via `wasm-bindgen` | 🔴 **High** |
| `jsonwebtoken` 9 | JWT create/verify (HMAC-SHA256) | Pure-Rust HMAC or `SubtleCrypto` | 🟡 Medium |
| `tower-http` | CORS, static files, tracing | Workers handles CORS + static assets natively | 🟢 Low |
| `qrcode` 0.14 | QR code SVG generation | Same — pure Rust, WASM-compatible | 🟢 Low |
| `chrono` 0.4 | Timestamps | Same — add `wasmbind` feature | 🟢 Low |
| `serde` + `serde_json` | Serialization | Same — WASM-compatible | 🟢 Low |
| `base64` 0.22 | Encoding | Same — WASM-compatible | 🟢 Low |
| `pem` 3 | RSA key parsing | Same — pure Rust | 🟢 Low |
| `dotenv` 0.15 | Env file loading | Remove — Workers uses `wrangler.toml` + secrets | 🟢 Low |

### The Hard Problem: RSA Signing

Google Sheets API uses **service account RS256 JWT assertion** — the `rsa` crate's `num-bigint` dependency doesn't compile to `wasm32-unknown-unknown`.

**Solution**: Call `crypto.subtle.sign()` from the Workers runtime via `wasm-bindgen` → `js_sys` → `SubtleCrypto`. This requires a custom bridge but uses the V8 built-in crypto (same engine Chrome uses).

---

## Migration Plan — Phased Approach

### Phase 1: Project Scaffold (1-2 days)

Set up the `worker` crate alongside the existing Axum build. Both targets coexist.

| Step | Description | Done? |
|------|-------------|-------|
| 1.1 | Create `worker/` directory with `wrangler.toml`, `Cargo.toml` for `wasm32-unknown-unknown` target | ☐ |
| 1.2 | Add `worker` crate dependency with `http` feature | ☐ |
| 1.3 | Create `worker/src/lib.rs` with `#[event(fetch)]` + health endpoint | ☐ |
| 1.4 | Verify `npx wrangler dev` runs and responds to `/api/health` | ☐ |
| 1.5 | Configure Workers Static Assets to serve `frontend-leptos/dist/` as SPA | ☐ |
| 1.6 | Verify frontend loads on Workers dev server at `/staff`, `/admin` | ☐ |

**Deliverable**: Worker responds to health check + serves static WASM frontend.

### Phase 2: Extract Shared Domain Code (1-2 days)

Move pure-Rust domain logic into a shared crate that compiles for both `x86_64` (tests) and `wasm32` (Workers).

| Step | Description | Done? |
|------|-------------|-------|
| 2.1 | Create `domain/` crate: `models/`, `qr/`, `config/types.rs` | ☐ |
| 2.2 | `domain/Cargo.toml` with only WASM-compatible deps: `serde`, `chrono`, `qrcode`, `base64`, `url`, `urlencoding` | ☐ |
| 2.3 | Verify `domain` compiles on both `x86_64-apple-darwin` and `wasm32-unknown-unknown` | ☐ |
| 2.4 | Update existing Axum `src/` to depend on `domain` — verify 25 tests still pass | ☐ |

**Deliverable**: Shared domain crate, zero regressions in existing tests.

### Phase 3: Crypto Bridge — SubtleCrypto for RSA (2-3 days)

The hardest part. Replace `rsa` + `jsonwebtoken` with WebCrypto calls.

| Step | Description | Done? |
|------|-------------|-------|
| 3.1 | Create `worker/src/crypto.rs` — `wasm-bindgen` bridge to `SubtleCrypto.importKey()` + `sign()` | ☐ |
| 3.2 | Implement `sign_rs256(private_key_pem: &str, payload: &[u8]) -> Vec<u8>` using JS interop | ☐ |
| 3.3 | Implement `verify_hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8>` for JWT session tokens | ☐ |
| 3.4 | Unit test: sign with SubtleCrypto, verify with known test vectors | ☐ |
| 3.5 | Implement JWT create/verify using the SubtleCrypto bridge | ☐ |

**Deliverable**: Crypto works in Workers without `rsa` or `ring` crates.

### Phase 4: HTTP Client Migration (1-2 days)

Replace `reqwest` with `worker::Fetch` for Google API calls.

| Step | Description | Done? |
|------|-------------|-------|
| 4.1 | Create `worker/src/http.rs` — typed HTTP client wrapping `worker::Fetch` | ☐ |
| 4.2 | Port `sheets::get_access_token()` to use new HTTP client + SubtleCrypto signing | ☐ |
| 4.3 | Port `sheets::get_attendees()` and `sheets::batch_update()` | ☐ |
| 4.4 | Port `auth::exchange_code()` and `auth::get_user_info()` | ☐ |
| 4.5 | Verify end-to-end: Worker fetches real Google Sheets data | ☐ |

**Deliverable**: Workers talks to Google Sheets API.

### Phase 5: Handlers Migration (1-2 days)

Port Axum handlers. Most handler logic stays the same — only state and HTTP types change.

| Step | Description | Done? |
|------|-------------|-------|
| 5.1 | Create `worker/src/state.rs` — `AppState` using `Env` bindings (secrets, vars) | ☐ |
| 5.2 | Port `handlers/health.rs` (trivial) | ☐ |
| 5.3 | Port `handlers/auth.rs` — Google OAuth callback, cookie set | ☐ |
| 5.4 | Port `handlers/attendee.rs` — list attendees | ☐ |
| 5.5 | Port `handlers/checkin.rs` — check-in attendee | ☐ |
| 5.6 | Port `handlers/qr.rs` — generate QR codes | ☐ |
| 5.7 | Port `middleware/security.rs` — security headers (or use `wrangler.toml` headers) | ☐ |
| 5.8 | Wire all handlers into Axum `Router` in `worker/src/lib.rs` | ☐ |

**Deliverable**: All API endpoints work on Workers.

### Phase 6: Integration Testing & Deploy (2-3 days)

| Step | Description | Done? |
|------|-------------|-------|
| 6.1 | End-to-end browser test: login → scan → confirm → scan again | ☐ |
| 6.2 | Mobile testing: Safari iOS + Chrome Android camera scanner | ☐ |
| 6.3 | Set up `wrangler.toml` secrets: `JWT_SECRET`, `GOOGLE_SERVICE_ACCOUNT_KEY`, etc. | ☐ |
| 6.4 | Configure custom domain or `*.workers.dev` | ☐ |
| 6.5 | `npx wrangler deploy` — go live | ☐ |
| 6.6 | Remove `Dockerfile`, `docker-compose.yml`, legacy `frontend/` | ☐ |
| 6.7 | Update README with Workers deployment instructions | ☐ |

**Deliverable**: Production deployment on Cloudflare Workers.

---

## Target Architecture

```
event-checkin/
├── domain/                      # Shared crate (compiles x86_64 + wasm32)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── models/
│       │   ├── attendee.rs      # AttendeeRow, Attendee, from_sheet_values()
│       │   ├── auth.rs          # Claims, ServiceAccountClaim
│       │   └── api.rs           # API response types
│       ├── qr/
│       │   └── generator.rs     # QR code generation (pure Rust)
│       └── config/
│           └── types.rs         # Config structs (no env reading)
│
├── worker/                      # Cloudflare Worker crate (wasm32 only)
│   ├── Cargo.toml               # worker crate with http feature
│   ├── wrangler.toml            # Workers config, static assets, secrets
│   └── src/
│       ├── lib.rs               # #[event(fetch)] + Axum router
│       ├── state.rs             # AppState from Env bindings
│       ├── crypto.rs            # SubtleCrypto bridge for RSA/HMAC
│       ├── http.rs              # HTTP client wrapping worker::Fetch
│       ├── handlers/
│       │   ├── auth.rs          # OAuth callback
│       │   ├── attendee.rs      # List attendees
│       │   ├── checkin.rs       # Check in
│       │   ├── qr.rs            # QR generation
│       │   └── health.rs        # Health check
│       └── middleware.rs        # Security headers
│
├── frontend-leptos/             # Leptos 0.8 WASM frontend (unchanged)
│   ├── src/
│   ├── js/
│   ├── dist/                    # Built → served as Workers Static Assets
│   └── Trunk.toml
│
├── src/                         # Legacy Axum build (kept for reference/tests)
├── .env.example
├── README.md
└── .handovers/
```

### What Gets Removed After Migration

| Item | Reason |
|------|--------|
| `Dockerfile` | No Docker needed |
| `docker-compose.yml` | No Docker needed |
| `frontend/` (legacy JS) | Replaced by Leptos frontend |
| `tokio` dependency | Workers has no tokio |
| `reqwest` dependency | Replaced by `worker::Fetch` |
| `rsa` + `sha2` dependencies | Replaced by SubtleCrypto |
| `dotenv` dependency | Workers uses `wrangler.toml` secrets |

### What Stays Unchanged

| Item | Reason |
|------|--------|
| `frontend-leptos/` | Entire frontend stays the same |
| `domain/src/models/` | Serde types, no platform dependency |
| `domain/src/qr/` | Pure Rust, WASM-compatible |
| All frontend API calls | Same endpoints, same cookies |

---

## Effort Estimate

| Phase | Days | Risk | Parallelizable? |
|-------|------|------|-----------------|
| Phase 1: Scaffold | 1-2 | 🟢 Low | No — foundation |
| Phase 2: Shared domain | 1-2 | 🟢 Low | Partial — can start models first |
| Phase 3: Crypto bridge | 2-3 | 🔴 **High** | Yes — independent from other phases |
| Phase 4: HTTP client | 1-2 | 🟡 Medium | After Phase 3 (needs crypto) |
| Phase 5: Handlers | 1-2 | 🟡 Medium | After Phase 4 (needs HTTP client) |
| Phase 6: Integration + Deploy | 2-3 | 🟡 Medium | After Phase 5 |
| **Total** | **10-14 days** | | |

---

## How to Dev / Test (Current Axum Build — Still Active)

### Backend

```bash
cargo check
cargo test          # 25 tests
cargo clippy
RUST_LOG=info cargo run  # port 3000
```

### Frontend

```bash
cd frontend-leptos
CARGO_BUILD_JOBS=1 trunk build       # build WASM
trunk serve --port 3001               # dev with hot-reload
cargo test --target x86_64-apple-darwin  # unit tests
```

### Workers (After Phase 1)

```bash
cd worker
npx wrangler dev                     # local dev server (port 8787)
npx wrangler deploy                  # deploy to Cloudflare
```

---

## Reflection — Struggles / Solved

No new code issues in this session. The system review confirmed a healthy codebase.

The **main strategic decision** was the Cloudflare Workers migration direction:
- `workers-rs` `http` feature makes Axum-on-Workers viable — handlers can be preserved
- RSA signing is the hardest problem — `SubtleCrypto` bridge is the solution
- Phased approach minimizes risk — each phase is independently verifiable
- `domain/` shared crate preserves the 25 existing tests

---

## Issues Ref

- Handover 007: Leptos 0.8 upgrade, camera QR scanner, CSP fix
- Handover 006: Admin badges, non-staff rejection, QR force regenerate
- Handover 005: Participation type check, preview mode
- Handover 004: Cookie auth fix and frontend
- Handover 003: P1/P2 production readiness