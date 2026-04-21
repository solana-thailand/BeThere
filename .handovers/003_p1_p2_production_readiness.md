# 003 - P1 End-to-End Testing & P2 Production Readiness

**Date**: 2026-04-21
**Status**: ✅ Complete
**Branch**: main

## What Happened

Completed P1 (end-to-end testing with real Google credentials) and P2 (production readiness: security hardening, Docker, graceful shutdown). All endpoints verified working with real Google Sheets API. Added security headers middleware, restrictive CORS, Dockerfile, and docker-compose.yml.

## P1: End-to-End Test Results

All tests run against real Google Cloud credentials (OAuth + Service Account). Server connected to production Google Sheet `1FMQiTsHl1msFVpgcB4aymvxwtkLGR0ulo4UhckCAhdk`.

| # | Test | Result |
|---|------|--------|
| 1 | `GET /api/health` | ✅ 200 `{"status":"ok"}` |
| 2 | `GET /api/auth/url` | ✅ 200 Real Google OAuth URL with correct client_id |
| 3 | `GET /api/auth/me` (valid staff JWT) | ✅ 200 `{"email":"ratchapon.poc@gmail.com","is_staff":true}` |
| 4 | `GET /api/attendees` (Sheets API) | ✅ 200 21 approved attendees, 54 total rows |
| 5 | `GET /api/attendee/gst-GcdL8A3BeFTtfBd` | ✅ 200 Full details + QR image base64 |
| 6 | `POST /api/checkin/gst-GcdL8A3BeFTtfBd` | ✅ 200 Checked in, timestamp written to Google Sheet |
| 7 | `POST /api/checkin/gst-GcdL8A3BeFTtfBd` (duplicate) | ✅ 200 `"attendee is already checked in"` |
| 8 | `POST /api/generate-qrs` | ✅ 200 21 total, 0 generated (all have existing URLs), 21 skipped |
| 9 | `GET /api/attendees` (no auth) | ✅ 401 |
| 10 | `GET /api/attendees` (non-staff JWT) | ✅ 403 |
| 11 | Static files (index, staff, admin, css, js) | ✅ All 200 |
| 12 | `GET /nonexistent` | ✅ 404 |

### Verified Google Sheet State

- Column I (checked_in_at): Attendee `gst-GcdL8A3BeFTtfBd` (Marketings) has timestamp `2026-04-21T10:27:47.298611+00:00`
- Column K (qr_code_url): All 21 approved attendees have existing Luma URLs

## P2: Production Readiness Changes

### 1. Security Headers Middleware

**File**: `src/middleware/mod.rs`, `src/middleware/security.rs`

Added a middleware that injects 9 security headers into every response:

| Header | Value |
|--------|-------|
| `Strict-Transport-Security` | `max-age=63072000; includeSubDomains; preload` |
| `X-Content-Type-Options` | `nosniff` |
| `X-Frame-Options` | `DENY` |
| `X-XSS-Protection` | `0` (modern browsers use CSP) |
| `Referrer-Policy` | `strict-origin-when-cross-origin` |
| `Content-Security-Policy` | `default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; font-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'` |
| `Permissions-Policy` | `camera=(self), microphone=(), geolocation=(), payment=()` |
| `Cross-Origin-Opener-Policy` | `same-origin` |
| `Cross-Origin-Resource-Policy` | `same-origin` |

Uses `LazyLock<HeaderValue>` for zero-allocation header injection. Includes unit test verifying all 9 headers.

### 2. CORS: Permissive → Configurable

**File**: `src/main.rs` — `build_cors_layer()`

- **Development** (localhost/127.0.0.1): `AllowOrigin::any()` (permissive)
- **Production**: `AllowOrigin::exact(SERVER_URL)` (restrictive)
- Methods: `GET`, `POST`, `OPTIONS` only
- Headers: `Authorization`, `Content-Type` only

### 3. Graceful Shutdown

**File**: `src/main.rs` — `shutdown_signal()`

- Handles Ctrl+C (SIGINT) and SIGTERM
- Uses `axum::serve().with_graceful_shutdown()`
- Logs shutdown event
- Unix-only SIGTERM handler behind `#[cfg(unix)]`

### 4. Dockerfile (Multi-Stage)

**File**: `Dockerfile`

Two-stage build:
1. **Builder** (`rust:1.85-bookworm`): Dependency caching layer + release build
2. **Runtime** (`debian:bookworm-slim`): Minimal image with ca-certificates, non-root user, healthcheck

Features:
- Non-root user (`appuser`, uid 1000)
- Health check (`curl /api/health` every 30s)
- Default env vars (`HOST=0.0.0.0`, `PORT=3000`)
- Only copies binary + frontend assets

### 5. Docker Compose

**File**: `docker-compose.yml`

- Reads `.env` automatically
- Resource limits: 256MB RAM, 0.5 CPU
- Logging: json-file driver, 10MB max, 3 files rotation
- `restart: unless-stopped`

### 6. .dockerignore

**File**: `.dockerignore`

Excludes: target/, .env, .git/, IDE files, docs, logs, Docker files themselves.

## Files Changed

| File | Change |
|------|--------|
| `src/middleware/mod.rs` | New — re-exports security middleware |
| `src/middleware/security.rs` | New — 9 security headers + unit test |
| `src/main.rs` | Added security headers layer, configurable CORS, graceful shutdown, middleware module |
| `Dockerfile` | New — multi-stage production build |
| `docker-compose.yml` | New — deployment config |
| `.dockerignore` | New — Docker build context exclusions |

## Test Results

```
cargo build   → 0 errors, 0 warnings
cargo clippy  → 0 errors, 0 warnings
cargo test    → 17/17 tests pass (16 original + 1 new security headers test)
e2e smoke     → 12/12 endpoints verified with real Google credentials
```

## Key Lessons

1. **Axum middleware ordering matters**: Layers are applied bottom-to-top in the chain. Security headers layer placed before TraceLayer so headers appear in trace logs.
2. **`LazyLock<HeaderValue>`** avoids allocating HeaderValue on every request — compile-time verified constant headers.
3. **Docker dependency caching**: By creating a dummy `main.rs`, building deps, then replacing with real source, Docker caches the expensive dependency compilation layer. Only source changes trigger recompile.
4. **CORS permissive vs restrictive**: Detecting localhost in SERVER_URL is a simple heuristic that avoids needing a separate `CORS_ORIGINS` env var.

## Remaining Work / Future Improvements

- [ ] **Browser OAuth flow test**: The OAuth login flow (`index.html` → Google → callback → JWT → `staff.html`) needs manual browser testing
- [ ] **Rate limiting**: Add `tower-governor` or similar for API rate limiting
- [ ] **HTTPS termination**: Use a reverse proxy (nginx/Caddy) or add `axum-server` with rustls for direct TLS
- [ ] **Health check without curl**: Install `curl` in Docker runtime or switch to a Rust-native health check
- [ ] **Access token caching**: Currently re-fetches Google API access token on every request. Could cache it for ~55 minutes (expires in 1 hour)
- [ ] **CSRF protection**: Add state parameter to OAuth flow for CSRF protection
- [ ] **Deploy**: Push Docker image to registry, deploy to VPS/cloud (Railway, Fly.io, etc.)
- [ ] **Update Google OAuth redirect URI** to production URL when deployed

## How to Deploy

```bash
# 1. Build and run locally with Docker
docker compose up --build -d

# 2. Check logs
docker compose logs -f

# 3. Stop
docker compose down

# 4. Deploy to Fly.io (example)
fly launch
fly secrets set GOOGLE_CLIENT_ID=... GOOGLE_CLIENT_SECRET=... JWT_SECRET=... etc
fly deploy