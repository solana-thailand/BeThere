# Handover 058: Google Sign-In for Attendees (Issue 016)

**Date**: 2026-05-17
**Branch**: main
**Status**: ✅ Built, ready for deploy + testing

---

## What Happened

Implemented Google Sign-In for attendees to fix a critical security vulnerability (Issue 016): anyone who knew an attendee's email could register with it, receive the claim token, and check in as that person. This is now fixed by requiring Google OAuth for registration — email is taken from JWT claims, not the request body.

## Security Vulnerability Fixed

**Before**: Attacker knows email → registers with it → gets claim_token + QR → checks in as victim
**After**: Must sign in with Google → JWT proves email ownership → registration locked to verified email

## Changes Summary

### Backend (Worker)

| File | Change |
|------|--------|
| `worker/src/auth.rs` | Added `require_identity` middleware (JWT-only, no staff check). Updated `get_auth_url` to accept optional `redirect` param → OAuth `state` parameter |
| `worker/src/handlers/auth.rs` | Modified `auth_callback` to create JWT for ALL users (not just staff). Non-staff → redirect to `state` param (event page) instead of rejection. Added `AuthUrlQuery` for redirect param |
| `worker/src/handlers/register.rs` | Added `GET /api/my-registration/:slug` endpoint. Modified `register_attendee` to take email from JWT claims. Added `MyRegistrationResponse` struct |
| `worker/src/handlers/mod.rs` | Created `attendee_authed` router with `require_identity` middleware. Moved `/public/register` to it. Added `/my-registration/:slug` route |

### Frontend (Leptos)

| File | Change |
|------|--------|
| `frontend-leptos/src/pages/public_event.rs` | Complete rewrite of registration section: auth gate with `AuthState` enum, `RegistrationLookup` for existing registration check, Google Sign-In button, locked email field, session expiry handling |
| `frontend-leptos/Cargo.toml` | Added `urlencoding = "2"` dependency |

### Key Architecture Decisions

1. **Dual middleware strategy**: `require_auth` (staff check) for protected routes, `require_identity` (JWT only) for attendee routes
2. **OAuth state parameter**: Used to pass redirect URL through Google OAuth flow so non-staff users return to the event page
3. **Cookie path = /api**: JWT cookie scoped to API requests only. Frontend `fetch()` calls include it automatically
4. **Registration lookup**: `GET /api/my-registration/:slug` checks if already registered → returns attendee data + next_step for redirect
5. **Removed localStorage resume flow**: Google auth replaces it — identity is now the JWT, not localStorage progress

## Build

| Step | Result |
|------|--------|
| `cargo check -p event-checkin-worker --target wasm32-unknown-unknown` | ✅ Clean |
| `cargo test -p event-checkin-worker` | ✅ 47/47 passed |
| `cargo clippy -p event-checkin-worker` | ✅ Clean |
| `cd frontend-leptos && cargo check --target wasm32-unknown-unknown` | ✅ Clean |
| `trunk build --release` | ✅ 4m50s, success |

## Issues Ref

- Implements issue 016 (Attendee Google Auth)
- Depends on handover 057 (CSP eval fix — already built)
- Supersedes localStorage resume flow from earlier implementations

## User Flow (New)

```
/e/:slug (public event page)
  │
  ├─ Not signed in → see event details + "Sign in with Google" button
  │                    (registration form hidden)
  │
  └─ Signed in (JWT cookie) → event details + registration form
                               (email locked to Google account, read-only)
                               │
                               ├─ Already registered → show "Welcome back" + auto-redirect
                               └─ Not registered → fill name + submit → deposit/ticket
```

## Next Steps

- [ ] Deploy and test in browser (staff + attendee flows)
- [ ] Verify Google OAuth popup works on mobile
- [ ] Test: staff can register as attendee with same email
- [ ] Test: duplicate registration returns existing data
- [ ] Consider: set cookie `Path=/` instead of `Path=/api` if SPA needs broader access
