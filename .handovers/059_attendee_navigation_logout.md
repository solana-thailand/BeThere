# 059 — Attendee Navigation, Logout & My Registrations (Issue 017)

## What Happened

Continued from session 058 (Issue 016 — Google Sign-In). Three UX gaps were identified and implemented:

1. **Dead-end "← Back to home"** on deposit page → attendee can't navigate back to their event
2. **No logout button** anywhere in the attendee-facing UI
3. **No "My Registrations"** section on landing page for signed-in users

Additionally, several bugs were found and fixed during verification.

## Commits (4 total, on `main`)

| Commit | Description |
|--------|-------------|
| `3c89fd3` | feat(ux): attendee navigation, logout & my-registrations (issue 017) |
| `1577828` | fix(ux): issue 017 — gate deposit logout on auth, POST logout, status labels |
| `c9b6f1d` | fix(ux): unify Google sign-in button, improve deposit welcome, show events always |
| `64ec532` | fix(auth): respect state redirect for all users, fix events API parsing |

## Changes Made

### Backend

| File | Change |
|------|--------|
| `domain/src/models/deposit.rs` | Added `event_slug: String` to `DepositStatusResponse` |
| `worker/src/handlers/deposit.rs` | Populate `event_slug: event.slug` in deposit status response |
| `worker/src/handlers/register.rs` | New `MyRegistrationsItem` struct with `status` field + `my_registrations` handler (iterates active events, matches JWT email). Status derived from attendee fields: registered → deposit pending → deposit confirmed → checked in → nft claimed. |
| `worker/src/handlers/mod.rs` | Added `/my-registrations` route (GET, attendee_authed). Changed `/auth/logout` from GET to POST. |
| `worker/src/handlers/auth.rs` | Fixed redirect priority: `state` param (event page URL) now takes precedence for ALL users, not just non-staff. Prevents staff being redirected to `/staff` when signing in from `/e/:slug`. |

### Frontend

| File | Change |
|------|--------|
| `frontend-leptos/src/api.rs` | Added `event_slug: String` to frontend `DepositStatusResponse` |
| `frontend-leptos/src/pages/deposit.rs` | Added auth check (`signed_in_email` signal + Effect fetching `/api/auth/me`). Logout button only visible when signed in, shows "Welcome, {email}" in card-style bar. Back-links use `/e/{slug}` when event_slug available, fallback `/`. All logout calls use POST. |
| `frontend-leptos/src/pages/public_event.rs` | Added `google_icon()` function matching login page pattern. Sign-in button now uses `btn-google` class (same style as `/login`). Signed-in indicator bar with email + sign out. Auth redirect uses event-scoped URL. |
| `frontend-leptos/src/pages/landing.rs` | Added `MyRegistrations` component: checks auth on mount, fetches `/api/my-registrations`, renders "Your Events" section with color-coded status labels (green=confirmed, yellow=pending). Fixed `UpcomingEvents` — always shows heading + loading/empty state instead of invisible div. Fixed API response parsing: wrapped in `ApiResponse<PublicEventsResponse>` with `#[derive(Default)]`. |
| `frontend-leptos/src/auth.rs` | Changed logout call from GET to POST |
| `frontend-leptos/style.css` | Added `.logout-btn-wrapper` card-style CSS (flex, space-between, background, shadow) |

### Documentation

| File | Change |
|------|--------|
| `.issues/017_attendee_navigation_logout.md` | Created — 88 lines covering 3 UX gaps |
| `docs/ux_roadmap.md` | Updated AF-6, AF-7, AF-8 to ✅ Implemented |

## Bugs Found & Fixed

| Bug | Root Cause | Fix |
|-----|-----------|-----|
| Deposit page logout always visible | No auth state check — button rendered unconditionally | Added `signed_in_email` signal + Effect to check `/api/auth/me` |
| Logout used GET not POST | Route was `get(auth::auth_logout)`, frontend used `Request::get()` | Changed to `post()` in route + all 5 frontend callers |
| No status text in My Registrations | Only showed CTA labels ("Complete Deposit"), no explicit status | Added `status` field to backend + color-coded display in frontend |
| Staff redirected to `/staff` from `/e/:slug` | Auth callback prioritized role over `state` param | `state` param now takes precedence for ALL users |
| UpcomingEvents invisible | Response parsing: `json::<PublicEventsResponse>()` didn't account for `ApiResponse<>` wrapper | Changed to `json::<ApiResponse<PublicEventsResponse>>()` + `#[derive(Default)]` |
| Sign-in button on `/e/:slug` looked different from `/login` | Different CSS class (`btn-primary` vs `btn-google`), different SVG icon, different sizes | Unified to `btn-google` + same `google_icon()` function |
| `PublicEventsResponse` missing `Default` | `ApiResponse<T>` has `#[serde(default)]` fields requiring `T: Default` | Added `#[derive(Default)]` |
| No events heading when loading/empty | Rendered invisible `<div></div>` | Always show heading + spinner or empty message |

## Acceptance Criteria — Issue 017

- [x] Deposit page "← Back to event" links to `/e/:slug` (not `/`)
- [x] `DepositStatusResponse` includes `event_slug` field
- [x] `POST /api/auth/logout` clears JWT cookie and returns 200
- [x] Deposit page shows logout button (visible when signed in)
- [x] Public event page shows logout button (visible when signed in)
- [x] After logout, page refreshes to unsigned-in state
- [x] Landing page shows "My Registrations" section for signed-in users
- [x] Each registration shows event name + link to `/e/:slug` + status

## How to Dev/Test

1. `cd worker && bash deploy.sh dev --remote` — start worker with remote KV
2. `cd frontend-leptos && bash build.sh --watch` — build frontend with auto-rebuild
3. Open `http://localhost:8787` — should see "Upcoming Events" heading
4. Navigate to `/e/:slug` — sign in with Google, verify redirect back to event page
5. Check landing page shows "Your Events" section when signed in
6. Test logout on deposit page, event page, and landing page

## Remain Work

- Smoke test after deploy (all AC items)
- P2-5: Walk-in Phase 4 (CSV export + sheet sync) — Issue 014
- P2-1: Real-Time Admin Dashboard
- P2-2: Batch/Manual Check-In for staff
- P2-3: Wallet Error Recovery Messages
- P3-5: Event Cancellation UI
- P3-6: Load Testing
- P3-7: External Security Audit

## Issues Ref

- `.issues/017_attendee_navigation_logout.md`
- `.handovers/058_attendee_google_auth.md` — prerequisite (Issue 016)
