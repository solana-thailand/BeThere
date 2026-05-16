# 016 — Attendee Identity Verification via Google Sign-In

## Summary

Attendees currently register by entering any email on the public event page. This means anyone who knows an email address can impersonate that attendee — register, deposit, get the QR code, and check in. Fix by requiring Google Sign-In for registration and ticket access.

## Security Vulnerability (Current)

```
Attacker knows victim's email (from LinkedIn, GitHub, etc.)
  → Registers on /e/:slug with victim's email
  → Backend returns: attendee_id, claim_token, next_step URL
  → Attacker accesses deposit page → gets QR code → checks in as victim
```

The duplicate-email fix (commit 2499a78) made this worse — it returns the existing attendee's claim_token to anyone who knows the email.

## Proposed Flow

### Dual-purpose Google OAuth (reuse existing staff auth)

| | Staff | Attendee |
|---|---|---|
| OAuth scope | `openid email profile` | `openid email profile` (same) |
| Callback | Check staff allowlist → `/staff` | Not staff → redirect back to `/e/:slug` |
| JWT | Same cookie, same claims | Same cookie, same claims |
| Role check | Per-page (`/staff` checks staff role) | Per-page (`/e/:slug` checks registration) |

### User Flow

```
/e/:slug (public event page)
  │
  ├─ Not signed in → see event details + "Sign in with Google to register"
  │                    (registration form hidden)
  │
  └─ Signed in (JWT cookie) → see event details + registration form
                               (email locked to Google account, cannot change)
                               │
                               ├─ Already registered → show deposit/ticket status
                               └─ Not registered → fill name + submit → deposit/ticket
```

### Key Design Decisions

1. **Email lookup is per-event sheet** — not scanning all sheets. Backend receives slug from URL, queries only that event's Google Sheet.
2. **No attendee profile table needed** — identity is the Google account. Returning attendees recognized by email lookup in event sheet. Can add KV-based profile later.
3. **Staff + Attendee same email works** — JWT = identity proof, not role. `/staff` checks staff role, `/e/:slug` checks registration. Same person can be staff AND attendee.

## Implementation Plan

### Phase 1: Backend Changes

| # | Task | Files |
|---|------|-------|
| 1a | New endpoint: `GET /api/my-registration/:slug` (JWT-required, lookup by email in event sheet) | `worker/src/handlers/register.rs` or new `attendee_self.rs` |
| 1b | Modify auth callback: non-staff → redirect back to referrer (event page) instead of rejecting | `worker/src/handlers/auth.rs` |
| 1c | Lock registration: `POST /api/public/register` requires JWT, email taken from token | `worker/src/handlers/register.rs` |
| 1d | Auth middleware: allow `/api/public/event/:slug` without auth (public info), but require auth for `/api/public/register` | `worker/src/auth.rs` |

### Phase 2: Frontend Changes

| # | Task | Files |
|---|------|-------|
| 2a | Public event page: hide registration form when not signed in, show "Sign in with Google" | `frontend-leptos/src/pages/public_event.rs` |
| 2b | After sign-in: show registration form with email locked (from JWT), or show ticket status if already registered | `frontend-leptos/src/pages/public_event.rs` |
| 2c | New API call: `GET /api/my-registration/:slug` to check if already registered | `frontend-leptos/src/api.rs` |

### Phase 3: Cleanup

| # | Task | Files |
|---|------|-------|
| 3a | Remove localStorage resume flow (no longer needed — auth replaces it) | `frontend-leptos/src/pages/public_event.rs` |
| 3b | Update CSP if needed for Google Sign-In popup | `worker/src/middleware.rs` |
| 3c | Update docs | `docs/business_flows_event_page.md`, `docs/ux_roadmap.md` |

## Acceptance Criteria

- [ ] Cannot register without Google Sign-In
- [ ] Cannot view ticket/QR without Google Sign-In
- [ ] Email is auto-filled from Google account (read-only)
- [ ] Staff can register as attendee with same email
- [ ] Duplicate registration returns existing attendee data (no error)
- [ ] `/e/:slug` still shows event details without sign-in (public)
- [ ] Existing staff login flow unchanged

## Dependencies

- Existing Google OAuth setup (client_id, client_secret, redirect_uri) — already configured
- `worker/src/auth.rs` — auth module with get_auth_url, handle_callback, JWT creation
- `worker/src/handlers/auth.rs` — auth endpoints

## Refs

- `.handovers/057_csp_eval_fix.md` — CSP fix (prerequisite, code built but not deployed)
- `.handovers/056_ux_flow_improvements_deploy.md` — UX flow that exposed the vulnerability
- `docs/security_audit.md` — existing security docs

## Implementation Status

**Date**: 2026-05-17
**Status**: ✅ Implemented, built, ready for deploy

### Changes Made

See `.handovers/058_attendee_google_auth.md` for full details.

### Acceptance Criteria Status

- [x] Cannot register without Google Sign-In (`require_identity` middleware on `/api/public/register`)
- [x] Cannot view ticket/QR without Google Sign-In (`/api/my-registration/:slug` requires JWT)
- [x] Email is auto-filled from Google account (read-only, locked `<input readonly>`)
- [x] Staff can register as attendee with same email (same JWT, different role checks per route)
- [x] Duplicate registration returns existing attendee data (idempotent, email from JWT)
- [x] `/e/:slug` still shows event details without sign-in (public event data route unchanged)
- [x] Existing staff login flow unchanged (`require_auth` middleware still checks staff status)
