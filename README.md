# Event Check-In

QR-based event check-in system. Staff scan attendee QR codes to check them in. Admin dashboard shows stats.

**Stack:** Rust (Cloudflare Workers) + Leptos WASM frontend + Google Sheets

## Quick Start

```bash
# 1. Install prerequisites
cargo install wasm-bindgen-cli --version 0.2.100
cd worker && npm install && cd ..

# 2. Build frontend
cd frontend-leptos && trunk build && cd ..

# 3. Configure secrets (first time only)
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

# 4. Run locally
cd worker && ./deploy.sh dev
```

Open `http://localhost:8787`.

> **Note:** `deploy.sh` automatically handles the Yarn PnP (`~/.pnp.cjs`) conflict with wrangler's esbuild bundler — no manual `mv` needed.

## Workspace Structure

```
event-checkin/
├── domain/               — Shared types & logic (compiles x86_64 + wasm32)
├── worker/               — Cloudflare Worker (wasm32-unknown-unknown)
├── frontend-leptos/      — Leptos WASM frontend (standalone trunk build)
├── Cargo.toml            — Workspace root (members: domain, worker)
└── README.md
```

The `domain/` crate contains shared types (`Attendee`, `Claims`, `AppConfig`), QR generation, and sheet row parsing. The `worker/` crate consumes it, replacing `reqwest` with `worker::Fetch` and `rsa`/`jsonwebtoken` with V8 SubtleCrypto via `wasm-bindgen`.

## Google Sheet Layout

The attendee sheet (tab name configurable via `GOOGLE_SHEET_NAME`, default `"checkin"`):

| Column | Index | Field | Notes |
|--------|-------|-------|-------|
| A | 0 | `api_id` | Unique ID (e.g. `gst-abc123`) |
| B | 1 | `name` | First name |
| C | 2 | `last_name` | Last name |
| D | 3 | `display_name` | Fallback display name |
| E | 4 | `email` | Attendee email |
| F | 5 | `ticket_name` | Ticket type |
| G | 6 | `solana_address` | Optional Solana wallet |
| H | 7 | `approval_status` | Approval state |
| I | 8 | `checked_in_at` | ISO 8601 timestamp |
| J | 9 | `checked_in_by` | Staff email who checked in |
| K | 10 | `qr_code_url` | QR code link |
| Y | 24 | `participation_type` | In-Person / Online |

A separate **"staff"** sheet tab (configurable via `GOOGLE_STAFF_SHEET_NAME`) holds authorized staff emails in column A (header in row 1, emails from row 2). This is unioned with the `STAFF_EMAILS` secret — a user is staff if their email appears in either source.

## Deployment

```bash
# Build frontend (if changed)
cd frontend-leptos && trunk build && cd ..

# Deploy to Cloudflare Workers
cd worker && ./deploy.sh
```

The `deploy.sh` script handles the Yarn PnP conflict automatically. Alternatively, you can run `npx wrangler deploy` directly if you don't have `~/.pnp.cjs`.

Non-secret vars are in `worker/wrangler.toml` `[vars]`:

| Var | Default | Purpose |
|-----|---------|---------|
| `SERVER_URL` | `https://event-checkin.workers.dev` | Public URL for OAuth redirect |
| `GOOGLE_SHEET_NAME` | `checkin` | Attendee sheet tab name |
| `GOOGLE_STAFF_SHEET_NAME` | `staff` | Staff sheet tab name |

The frontend is served from `frontend-leptos/dist/` via Workers Assets with SPA fallback.

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
| POST | `/api/checkin/{id}` | Cookie + Staff | Check in attendee |
| POST | `/api/generate-qrs` | Cookie + Staff | Generate QR codes |

## Frontend Routes

| Path | Page |
|------|------|
| `/` | Login (Google OAuth) |
| `/staff` | Scanner — camera QR + manual lookup |
| `/admin` | Dashboard — attendee list, stats, QR management |

## Architecture

```
worker/src/             — Cloudflare Worker
  handlers/             — API endpoints (auth, check-in, QR, attendee, health)
  auth.rs               — Google OAuth + JWT (SubtleCrypto HMAC-SHA256)
  sheets.rs             — Google Sheets read/write (worker::Fetch)
  crypto.rs             — SubtleCrypto bridge (RSA-SHA256, HMAC-SHA256)
  http.rs               — HTTP client wrapping worker::Fetch
  middleware.rs         — Security headers, auth guard
  state.rs              — AppState from Env bindings

domain/src/             — Shared (compiles x86_64 + wasm32)
  config/               — AppConfig, OAuthConfig, SheetsConfig
  models/               — Attendee, Claims, API response types
  qr/                   — QR URL generation + base64 image

frontend-leptos/src/
  pages/                — Scanner (camera QR), Admin, Login
  api.rs                — API client types and fetch wrappers
  utils.rs              — Helpers (timestamps, badges, participation)
  js/                   — Camera + QR detection module
```

## Tests

```bash
# All tests (27 total)
cargo test

# Individual crates
cargo test -p event-checkin-domain   # 14 tests — shared types, QR logic
cargo test -p event-checkin-worker   # 13 tests — crypto, auth, sheets

# Worker WASM build check
cargo check -p event-checkin-worker --target wasm32-unknown-unknown

# Clippy
cargo clippy --all-targets
```

## Features

- **Camera QR Scanner** — BarcodeDetector (Chrome) + jsQR fallback (Firefox/Safari)
- **Staff check-in logging** — Records which staff member checked in each attendee (column J)
- **Sheet-based staff list** — Staff emails loaded from "staff" sheet tab + env var (unioned)
- **Participation types** — In-Person / Online badges, reject online at physical check-in
- **Admin stats** — Checked-in count, In-Person vs Online breakdown
- **Force QR regenerate** — Admin can regenerate codes per attendee
- **CSP compliant** — Zero `eval()` calls, no `unsafe-eval` directive
- **Edge deployment** — Cloudflare Workers with SubtleCrypto for JWT signing