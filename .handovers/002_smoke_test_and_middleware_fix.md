# 002 - Smoke Test & Auth Middleware Bug Fix

**Date**: 2025-04-19
**Status**: ✅ Complete
**Branch**: main

## What Happened

After the compilation fixes in step 001, ran a comprehensive smoke test of the running server. Discovered and fixed a critical bug in the auth middleware where public routes were incorrectly requiring authentication.

## Bug Found: Auth Middleware Not Skipping Public Routes

### Symptoms

- `GET /api/health` returned `401 Unauthorized` instead of the health check JSON
- `GET /api/auth/url` returned `401 Unauthorized` instead of the OAuth URL
- Only `GET /api/auth/callback` was also affected (3 routes total)

### Root Cause

When Axum nests a `Router` with `.nest("/api", api)`, the middleware inside `api` receives the request URI with the **prefix already stripped**. So the middleware on the inner router sees `/health` instead of `/api/health`.

The `is_public_route()` function only checked for full paths:
```rust
fn is_public_route(path: &str) -> bool {
    matches!(path, "/api/health" | "/api/auth/url" | "/api/auth/callback")
}
```

But the actual path the middleware received was `/health`, `/auth/url`, `/auth/callback` — none of which matched.

### How It Was Debugged

1. Added `tracing::info!` log to print the actual path the middleware sees
2. Discovered `.env` file had `RUST_LOG=event_checkin=info` which overrode shell env vars (early debug logs at `debug!` level were filtered out)
3. Changed to `info!` level, saw: `auth middleware processing path: /health` — confirming the prefix was stripped

### Fix

Updated `is_public_route()` in `src/handlers/auth.rs` to check both with and without `/api` prefix:

```rust
fn is_public_route(path: &str) -> bool {
    matches!(
        path,
        "/api/health"
            | "/health"
            | "/api/auth/url"
            | "/auth/url"
            | "/api/auth/callback"
            | "/auth/callback"
    )
}
```

### Alternative Considered

Splitting public and protected routes into separate routers with middleware only on the protected router. This would be cleaner but more invasive. The current fix is minimal and correct.

## Smoke Test Results

Created `.env` with dummy values for local testing. All 12 smoke tests pass:

| # | Test | Result |
|---|------|--------|
| 1 | `GET /api/health` (public) | ✅ 200 `{"status":"ok","service":"event-checkin","version":"0.1.0"}` |
| 2 | `GET /api/auth/url` (public) | ✅ 200 Returns Google OAuth URL |
| 3 | `GET /` (static index.html) | ✅ 200, 2076 bytes |
| 4 | `GET /staff.html` | ✅ 200, 8019 bytes |
| 5 | `GET /admin.html` | ✅ 200, 7405 bytes |
| 6 | `GET /css/style.css` | ✅ 200, 14514 bytes |
| 7 | `GET /js/app.js` | ✅ 200, 9928 bytes |
| 8 | `GET /api/attendees` (no JWT) | ✅ 401 `missing authorization header` |
| 9 | `GET /api/attendees` (invalid JWT) | ✅ 401 `invalid token` |
| 10 | `GET /api/auth/me` (valid staff JWT) | ✅ 200 `{"email":"admin@example.com","is_staff":true}` |
| 11 | `GET /api/attendees` (non-staff JWT) | ✅ 403 `user is not in staff allowlist` |
| 12 | `GET /nonexistent` | ✅ 404 |

### Notes

- Endpoints requiring Google Sheets API (attendees, checkin, generate-qrs) correctly fail with RSA key parse error since `.env` has a dummy private key. This is expected.
- JWT generation/verification works correctly with the `jsonwebtoken` crate.
- Staff allowlist enforcement works (non-staff email gets 403).

## Final Verification

```
cargo clippy  → 0 warnings, 0 errors
cargo test    → 16/16 tests pass
smoke test    → 12/12 endpoints verified
```

## Files Changed

| File | Change |
|------|--------|
| `src/handlers/auth.rs` | Fixed `is_public_route()` to match paths without `/api` prefix |
| `.env` | Created with dummy values for local smoke testing |

## Key Lessons

1. **Axum nested router strips prefix**: When middleware is applied inside a router that gets `.nest()`-ed, the request URI path has the prefix removed. The middleware sees the inner path, not the full path.
2. **dotenv overrides shell env**: `dotenv::dotenv()` sets env vars from `.env` file, which override shell-set `RUST_LOG`. For debugging, either change `.env` or use a different mechanism.
3. **Test the happy path**: Compilation passing doesn't mean runtime behavior is correct. Always smoke test the running server.

## Next Steps

1. **Google Cloud setup** (user action required):
   - Create Google Cloud project
   - Enable Google Sheets API
   - Create OAuth 2.0 Client ID (for staff login)
   - Create Service Account + download JSON key (for Sheets API access)
   - Share the Google Sheet with the service account email
2. **Fill in `.env`** with real credentials
3. **Test full OAuth flow** end-to-end
4. **Test QR generation** with real Google Sheet data
5. **Test check-in flow** with real attendees
6. **Deploy** to VPS/cloud (Railway, Fly.io, etc.)