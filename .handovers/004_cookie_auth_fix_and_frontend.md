# 004 - Cookie Auth Fix & Frontend Overhaul

**Date**: 2026-04-21
**Status**: ✅ Complete
**Branch**: main

## What Happened

The browser OAuth flow was broken: after successful Google login, the staff page flashed briefly then redirected back to the login page. Root cause was a race condition in the JS token handling. Fixed by switching from URL-based JWT tokens to HTTP-only cookies. Also began a Leptos frontend rewrite (scaffold created, not yet compiled).

## The Bug: URL Token Race Condition

### Symptom
- User clicks "Sign in with Google" → Google consent → redirects back
- Staff page (`/staff.html`) renders briefly (1-2 seconds) then redirects to `/` (login page)
- Server logs showed `"staff login successful"` — the backend was fine

### Root Cause
The old auth flow put the JWT in the URL query parameter:
```
/staff.html?token=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

In `scanner.js` DOMContentLoaded handler, the execution order was:
1. `requireAuth()` — checks `localStorage` for token → **not found** (token is still in URL) → redirects to `/`
2. `handleTokenFromUrl()` — never reached

First attempted fix: swap the order (`handleTokenFromUrl()` before `requireAuth()`). This still failed because `requireAuth()` was synchronous while the actual auth verification needed an async API call. The `isTokenExpired()` function tried to parse the JWT client-side which could fail in subtle ways.

### The Real Fix: Cookie-Based Auth

Switched to HTTP-only cookies entirely, eliminating client-side token management:

1. Server sets `event_checkin_token` cookie on OAuth callback
2. Browser sends cookie automatically on every request
3. `requireAuth()` became async — calls `/api/auth/me` to verify cookie server-side
4. No more localStorage, no more URL tokens, no more client-side JWT parsing

## Changes Made

### Server-Side

#### `src/handlers/auth.rs` — Cookie Auth + Logout

| Change | Detail |
|--------|--------|
| `auth_callback` | Sets HTTP-only cookie (`event_checkin_token=...; HttpOnly; SameSite=Lax; Path=/; Max-Age=86400`) instead of URL param. Redirects to `/staff.html` with no query params |
| `auth_logout` (new) | `GET /api/auth/logout` — clears cookie (`Max-Age=0`) and redirects to `/` |
| `require_auth` middleware | Now reads JWT from either `Authorization: Bearer` header OR `cookie` header. Backward compatible with API clients |
| `extract_token_from_request` (new) | Extracts token from Bearer header first, falls back to cookie parsing |
| `verify_token` (new) | Takes `Option<String>`, verifies JWT, returns `Claims` |
| `is_public_route` | Added `/api/auth/logout` and `/auth/logout` to public route list |

#### `src/handlers/mod.rs` — New Route

Added `.route("/auth/logout", get(auth::auth_logout))` to the API router.

#### `src/auth/mod.rs` — Export Update

Removed `extract_claims` export (dead code after refactor). Exports: `create_jwt`, `verify_jwt`.

#### `src/auth/jwt.rs` — Cleanup

Removed `extract_claims` function (was only used by old middleware). Removed unused `AppState` import.

#### `src/middleware/security.rs` — CSP Fix

Updated `Content-Security-Policy` to allow external scripts from unpkg.com:
```
script-src 'self' 'unsafe-inline' https://unpkg.com;
```
This is needed for the `html5-qrcode` library loaded from `https://unpkg.com/html5-qrcode@2.3.8/html5-qrcode.min.js` in `staff.html`.

### Frontend

#### `frontend/js/app.js` — Complete Rewrite

| Before | After |
|--------|-------|
| `getToken()`, `setToken()`, `clearToken()` — localStorage management | Removed entirely |
| `parseJwt()`, `isTokenExpired()` — client-side JWT parsing | Removed entirely |
| `handleTokenFromUrl()` — extract token from URL query | Removed entirely |
| `requireAuth()` — sync check of localStorage | `async requireAuth()` — calls `/api/auth/me` (cookie sent automatically) |
| `logout()` — clear localStorage + redirect | `logout()` — redirect to `/api/auth/logout` (server clears cookie) |
| `apiRequest()` — adds `Authorization: Bearer` header | `apiRequest()` — uses `credentials: "include"` (cookies sent automatically) |
| `initLoginPage()` — sync | `async initLoginPage()` — calls `checkAuth()` to see if already logged in |
| `currentUser` (new) | In-memory cache of user info from `/api/auth/me` |

#### `frontend/js/scanner.js` — Simplified Init

| Before | After |
|--------|-------|
| `DOMContentLoaded` sync handler | `DOMContentLoaded` async handler |
| `handleTokenFromUrl()` then `requireAuth()` | `await requireAuth()` only (cookie handles everything) |
| `async loadUserInfo()` calls `api.getMe()` | `loadUserInfo()` reads from cached `getUser()` |

#### `frontend/js/admin.js` — Same Pattern

Same changes as scanner.js: async `requireAuth()`, cached user info, removed `handleTokenFromUrl()`.

### Leptos Frontend Scaffold (Not Yet Compiled)

Created `frontend-leptos/` with a full Leptos 0.7 CSR structure:

```
frontend-leptos/
├── Cargo.toml          # leptos 0.7 CSR, web-sys, gloo, serde
├── Trunk.toml          # trunk build config (port 3001)
├── index.html          # HTML shell for trunk
├── style.css           # Dark theme (copied from frontend/css/style.css)
└── src/
    ├── main.rs         # WASM entry point
    ├── lib.rs          # App component with Router, 3 routes
    ├── auth.rs         # Token management, JWT parsing
    ├── api.rs          # API client using gloo
    └── pages/
        ├── mod.rs
        ├── login.rs    # Login page component
        ├── scanner.rs  # Staff scanner component
        └── admin.rs    # Admin dashboard component
```

**Status**: Files created but not yet compiled. Needs the Rust `trunk` build tool (the installed `trunk` CLI is the trunk.io merge tool, not the Rust WASM builder). Install with: `cargo install trunk` (or download a pre-built binary).

## Test Results

```
cargo build   → 0 errors, 0 warnings
cargo test    → 17/17 tests pass
curl tests    → Cookie auth verified: /api/auth/me returns 401 without cookie, 200 with cookie
               Bearer auth still works for API clients
```

### Verified Cookie Auth Flow

```bash
# Without cookie → 401
curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/api/auth/me
# → 401

# With cookie → 200
curl -s -b "event_checkin_token=$TOKEN" http://localhost:3000/api/auth/me
# → {"email":"ratchapon.poc@gmail.com","is_staff":true,"sub":"test-user-id"}

# Bearer header still works → 200
curl -s -H "Authorization: Bearer $TOKEN" http://localhost:3000/api/auth/me
# → {"email":"ratchapon.poc@gmail.com","is_staff":true,"sub":"test-user-id"}
```

### Browser Test

- ✅ `http://localhost:3000` → login page renders
- ✅ Click "Sign in with Google" → Google consent screen
- ✅ After approval → redirected to `/staff.html` (cookie set)
- ✅ Staff page stays rendered (no more redirect loop)
- ⬜ QR scanner camera test — not yet tested
- ⬜ Admin dashboard — not yet tested
- ⬜ Logout flow — not yet tested

## Files Changed

| File | Change |
|------|--------|
| `src/handlers/auth.rs` | Rewrote: cookie-based auth, logout endpoint, dual token extraction (header + cookie) |
| `src/handlers/mod.rs` | Added `/auth/logout` route |
| `src/auth/mod.rs` | Removed `extract_claims` export |
| `src/auth/jwt.rs` | Removed `extract_claims` function, removed unused `AppState` import |
| `src/middleware/security.rs` | CSP: added `https://unpkg.com` to `script-src` |
| `frontend/js/app.js` | Complete rewrite: cookie-based auth, removed all localStorage code |
| `frontend/js/scanner.js` | Simplified: async requireAuth, cached user info |
| `frontend/js/admin.js` | Simplified: async requireAuth, cached user info |
| `frontend-leptos/*` | New: Leptos 0.7 CSR scaffold (not yet compiled) |

## Key Lessons

1. **Never pass session tokens in URLs**: They're visible in browser history, server logs, and Referer headers. Cookies are the standard approach for browser sessions.
2. **HTTP-only cookies are more secure**: JavaScript cannot access them (no XSS token theft), and `SameSite=Lax` prevents CSRF for navigation requests.
3. **Async auth checks are more reliable**: Instead of client-side JWT parsing (which can fail on encoding edge cases), call the server to verify the session. The cookie is sent automatically.
4. **CSP blocks external resources silently**: The `html5-qrcode` library from unpkg.com was silently blocked by `script-src 'self'`. No error, no redirect — just a missing library. Always check CSP when external scripts don't load.
5. **The installed `trunk` CLI may not be the Rust one**: `trunk.io` (merge tool) and `trunk` (Rust WASM builder) share the same binary name. Check with `trunk --version` and `trunk build --help`.

## Remaining Work

### Immediate Testing
- [ ] **QR scanner camera test**: Open `staff.html` on a mobile device, verify camera scanning works
- [ ] **Manual check-in test**: Enter attendee ID `gst-GcdL8A3BeFTtfBd` in manual tab
- [ ] **Admin dashboard test**: Visit `/admin.html`, verify stats, attendee list, QR generation
- [ ] **Logout test**: Click "Sign Out", verify redirect to login, verify `/staff.html` redirects to login

### Leptos Frontend (Track A)
- [ ] Install Rust `trunk` build tool: `cargo install trunk` (may need to rename existing trunk.io binary first)
- [ ] Compile `frontend-leptos/`: `cd frontend-leptos && trunk serve`
- [ ] Fix compilation errors (Leptos 0.7 API may have drift)
- [ ] Wire up pages to the existing API endpoints
- [ ] Replace `frontend/` static files with trunk output

### Deployment (Track B)
- [ ] Choose deployment target: Railway, Fly.io, VPS
- [ ] `docker compose up --build -d` (already configured)
- [ ] Update Google OAuth redirect URI in Google Cloud Console: `http://localhost:3000/api/auth/callback` → `https://<domain>/api/auth/callback`
- [ ] Update `SERVER_URL` in `.env` to production URL
- [ ] HTTPS termination: nginx/Caddy reverse proxy or axum-server with rustls

### Future Improvements
- [ ] **Access token caching**: Cache Google API access token for ~55 minutes (currently re-fetches on every request, ~2s latency)
- [ ] **CSRF protection**: Add state parameter to OAuth flow
- [ ] **Rate limiting**: Add `tower-governor` for API rate limiting
- [ ] **WebSocket support**: Real-time check-in notifications to admin dashboard
- [ ] **Error page polish**: Styled HTML error pages instead of JSON for browser-facing routes

## How to Dev/Test

```bash
# Start server
RUST_LOG=info cargo run

# Browser test
open http://localhost:3000

# API test with cookie
TOKEN=$(node -e "const c=require('crypto');const s='YOUR_JWT_SECRET';const h=Buffer.from(JSON.stringify({alg:'HS256',typ:'JWT'})).toString('base64url');const n=Math.floor(Date.now()/1000);const p=Buffer.from(JSON.stringify({email:'YOUR_EMAIL',sub:'test',iat:n,exp:n+86400})).toString('base64url');process.stdout.write(h+'.'+p+'.'+c.createHmac('sha256',s).update(h+'.'+p).digest('base64url'))")
curl -b "event_checkin_token=$TOKEN" http://localhost:3000/api/attendees

# Run tests
cargo test