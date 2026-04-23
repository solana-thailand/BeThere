# Event Check-In

QR-based event check-in system. Staff scan attendee QR codes to check them in. Admin dashboard shows stats.

**Stack:** Rust (Axum) + Leptos 0.8 WASM frontend + Google Sheets + Cloudflare Workers

## Quick Start (Local Axum)

```bash
# 1. Copy env and fill in your values
cp .env.example .env

# 2. Build frontend (only needed once, or after frontend changes)
cd frontend-leptos && trunk build && cd ..

# 3. Run server
RUST_LOG=info cargo run
```

Open `http://localhost:3000` — Axum serves both the API and the pre-built WASM frontend from `frontend-leptos/dist/`.

**Frontend dev with hot-reload:** `cd frontend-leptos && trunk serve --port 3001` (separate terminal, proxies `/api` to port 3000)

## Workspace Structure

```
event-checkin/
├── src/                  — Axum server (main build, x86_64)
├── domain/               — Shared types & logic (compiles x86_64 + wasm32)
├── worker/               — Cloudflare Worker (wasm32-unknown-unknown)
├── frontend-leptos/      — Leptos 0.8 WASM frontend
├── Cargo.toml            — Workspace root
└── docker-compose.yml    — Docker setup (legacy Axum)
```

The `domain/` crate contains shared types (`Attendee`, `Claims`, `AppConfig`), QR generation, and sheet row parsing — used by both the Axum build and the Worker.

## Cloudflare Workers Deployment

The Worker crate provides full API parity with the Axum build, compiled to `wasm32-unknown-unknown`. It replaces `reqwest` with `worker::Fetch` and `rsa`/`jsonwebtoken` with V8 SubtleCrypto via `wasm-bindgen`.

### Prerequisites

```bash
# Install wasm-bindgen CLI (must match crate version)
cargo install wasm-bindgen-cli --version 0.2.100

# Install wrangler in the worker directory
cd worker && npm install
```

### Local Dev

```bash
cd worker
npx wrangler dev --port 8787
```

Note: If you have `~/.pnp.cjs` (Yarn PnP), temporarily rename it:
`mv ~/.pnp.cjs ~/.pnp.cjs.bak` before running, then restore after.

### Configure Secrets

Set each secret via wrangler (one-time setup):

```bash
cd worker
npx wrangler secret put JWT_SECRET
npx wrangler secret put GOOGLE_CLIENT_ID
npx wrangler secret put GOOGLE_CLIENT_SECRET
npx wrangler secret put GOOGLE_REDIRECT_URI
npx wrangler secret put GOOGLE_SERVICE_ACCOUNT_EMAIL
npx wrangler secret put GOOGLE_SERVICE_ACCOUNT_PRIVATE_KEY
npx wrangler secret put GOOGLE_SERVICE_ACCOUNT_TOKEN_URI
npx wrangler secret put GOOGLE_SHEET_ID
npx wrangler secret put STAFF_EMAILS
```

Non-secret vars are in `wrangler.toml` `[vars]` (e.g. `SERVER_URL`).

### Deploy

```bash
cd worker
npx wrangler deploy
```

The frontend is served from `../frontend-leptos/dist/` via Workers Assets with SPA fallback.

### Build Frontend for Workers

```bash
cd frontend-leptos
trunk build
```

The `dist/` directory is automatically picked up by `wrangler.toml` `[assets]`.

## Environment Variables

### Axum Build (`.env`)

| Variable | Purpose |
|----------|---------|
| `GOOGLE_CLIENT_ID` | Google OAuth client ID |
| `GOOGLE_CLIENT_SECRET` | Google OAuth client secret |
| `JWT_SECRET` | Secret for session tokens |
| `GOOGLE_SERVICE_ACCOUNT_KEY` | Service account JSON for Sheets API |
| `SPREADSHEET_ID` | Google Sheet ID with attendee data |
| `SERVER_URL` | Public URL (e.g. `http://localhost:3000`) |

### Worker Build (`wrangler secret`)

| Secret | Purpose |
|--------|---------|
| `JWT_SECRET` | HMAC-SHA256 key for session tokens |
| `GOOGLE_CLIENT_ID` | OAuth client ID |
| `GOOGLE_CLIENT_SECRET` | OAuth client secret |
| `GOOGLE_REDIRECT_URI` | OAuth redirect URI |
| `GOOGLE_SERVICE_ACCOUNT_EMAIL` | Service account email |
| `GOOGLE_SERVICE_ACCOUNT_PRIVATE_KEY` | PEM-encoded RSA private key |
| `GOOGLE_SERVICE_ACCOUNT_TOKEN_URI` | OAuth2 token endpoint |
| `GOOGLE_SHEET_ID` | Spreadsheet ID |
| `STAFF_EMAILS` | Comma-separated staff email allowlist |

## API Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/health` | No | Health check |
| GET | `/api/auth/url` | No | Google OAuth URL |
| GET | `/api/auth/callback` | No | OAuth callback, sets cookie |
| GET | `/api/auth/me` | Cookie | Current user info |
| GET | `/api/auth/logout` | No | Clear session cookie |
| GET | `/api/attendees` | Cookie | List all attendees + stats |
| GET | `/api/attendee/{id}` | Cookie | Single attendee details |
| POST | `/api/checkin/{id}` | Cookie | Check in attendee |
| POST | `/api/generate-qrs` | Cookie | Generate QR codes |

## Frontend Routes

| Path | Page |
|------|------|
| `/` | Login (Google OAuth) |
| `/staff` | Scanner — camera QR + manual lookup |
| `/admin` | Dashboard — attendee list, stats, QR management |

## Architecture

```
src/                    — Axum server
  handlers/             — API endpoints (auth, check-in, QR, attendee)
  auth/                 — Google OAuth + JWT session (jsonwebtoken)
  sheets/               — Google Sheets read/write (reqwest)
  qr/                   — QR code generation
  middleware/           — CSP headers, security

worker/src/             — Cloudflare Worker
  handlers/             — Same API endpoints
  auth.rs               — OAuth + JWT (SubtleCrypto HMAC-SHA256)
  sheets.rs             — Google Sheets (worker::Fetch)
  crypto.rs             — SubtleCrypto bridge (RSA-SHA256, HMAC-SHA256)
  http.rs               — HTTP client wrapping worker::Fetch
  middleware.rs         — Security headers

domain/src/             — Shared (compiles x86_64 + wasm32)
  config/               — AppConfig, OAuthConfig, SheetsConfig
  models/               — Attendee, Claims, API response types
  qr/                   — QR URL generation + base64 image

frontend-leptos/src/
  pages/                — Scanner (camera QR), Admin, Login
  js/                   — Camera + QR detection module
  components.rs         — Shared UI (header, toast, protected route)
  utils.rs              — Helpers (timestamps, badges)
```

## Tests

```bash
# All tests (52 total)
cargo test

# Individual crates
cargo test -p event-checkin          # 25 tests — Axum handlers, auth, QR
cargo test -p event-checkin-domain   # 14 tests — shared types, QR logic
cargo test -p event-checkin-worker   # 13 tests — crypto, auth, sheets

# Worker WASM build check
cargo check -p event-checkin-worker --target wasm32-unknown-unknown

# Clippy (zero warnings)
cargo clippy --all-targets
```

## Features

- **Camera QR Scanner** — BarcodeDetector (Chrome) + jsQR fallback (Firefox/Safari)
- **Participation types** — In-Person / Online badges, reject online at physical check-in
- **Admin stats** — Checked-in count, In-Person vs Online breakdown
- **Force QR regenerate** — Admin can regenerate codes per attendee
- **CSP compliant** — Zero `eval()` calls, no `unsafe-eval` directive
- **Cloudflare Workers** — Deploy to edge with SubtleCrypto for JWT signing

## Docker (Legacy Axum)

```bash
docker compose up --build