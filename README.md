# Event Check-In

QR-based event check-in system. Staff scan attendee QR codes to check them in. Admin dashboard shows stats.

**Stack:** Rust (Axum) + Leptos 0.8 WASM frontend + Google Sheets

## Quick Start

```bash
# 1. Copy env and fill in your values
cp .env.example .env

# 2. Build frontend (only needed once, or after frontend changes)
cd frontend-leptos && trunk build && cd ..

# 3. Run server
cargo run
```

Open `http://localhost:3000` — Axum serves both the API and the pre-built WASM frontend from `frontend-leptos/dist/`.

**Frontend dev with hot-reload:** `cd frontend-leptos && trunk serve --port 3001` (separate terminal, proxies `/api` to port 3000)

## Environment Variables

Key variables in `.env`:

| Variable | Purpose |
|----------|---------|
| `GOOGLE_CLIENT_ID` | Google OAuth client ID |
| `GOOGLE_CLIENT_SECRET` | Google OAuth client secret |
| `JWT_SECRET` | Secret for session tokens |
| `GOOGLE_SERVICE_ACCOUNT_KEY` | Service account JSON for Sheets API |
| `SPREADSHEET_ID` | Google Sheet ID with attendee data |
| `SERVER_URL` | Public URL (e.g. `http://localhost:3000`) |

## Architecture

```
src/
  handlers/    — API endpoints (auth, check-in, QR, health)
  auth/        — Google OAuth + JWT session
  sheets/      — Google Sheets read/write
  qr/          — QR code generation
  middleware/   — CSP headers, security

frontend-leptos/
  src/pages/   — Scanner (camera QR), Admin dashboard, Login
  js/          — Camera + QR detection module
  src/components.rs  — Shared UI (header, toast, protected route)
  src/utils.rs       — Shared helpers (timestamps, badges)
```

## API Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/health` | No | Health check |
| GET | `/api/auth/google` | No | Start Google OAuth |
| GET | `/api/auth/callback` | No | OAuth callback |
| POST | `/api/auth/logout` | Cookie | Clear session |
| GET | `/api/attendees` | Cookie | List all attendees |
| POST | `/api/checkin/:id` | Cookie | Check in attendee |
| POST | `/api/qr/generate` | Cookie | Generate QR codes |

## Frontend Routes

| Path | Page |
|------|------|
| `/` | Login (Google OAuth) |
| `/staff` | Scanner — camera QR + manual lookup |
| `/admin` | Dashboard — attendee list, stats, QR management |

## Features

- **Camera QR Scanner** — BarcodeDetector (Chrome) + jsQR fallback (Firefox/Safari)
- **Participation types** — In-Person / Online badges, reject online at physical check-in
- **Admin stats** — Checked-in count, In-Person vs Online breakdown
- **Force QR regenerate** — Admin can regenerate codes per attendee
- **CSP compliant** — Zero `eval()` calls, no `unsafe-eval` directive

## Docker

```bash
docker compose up --build
```

## Tests

```bash
# Backend
cargo test

# Frontend (native target)
cd frontend-leptos && cargo test --target x86_64-apple-darwin