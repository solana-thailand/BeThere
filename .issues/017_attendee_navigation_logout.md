# 017 — Attendee Navigation, Logout & "My Events" UX

## Summary

After implementing Google Sign-In (Issue 016), three UX gaps remain for attendees navigating between pages:

1. **Dead-end "Back to home"** on deposit page → attendee can't get back to their event
2. **No logout** anywhere in the attendee-facing UI
3. **No "My Events"** on landing page → signed-in attendees have no way to find their registrations

## Problems

### P1: "← Back to home" is a dead-end

The deposit page (`/deposit/:attendee_id?event_id=xxx`) has two `<a href="/">"← Back to home"</a>` links. Clicking goes to the landing page, which shows upcoming events but:
- No indication the user is signed in
- No section showing "Your Events" or "Your Registrations"
- No way to navigate back to `/e/:slug` for the event they were just viewing

The attendee would have to scroll through upcoming events and find their event again.

**Root cause**: `DepositStatusResponse` has `event_name` but no `event_slug`. Without the slug, we can't link back to `/e/:slug`.

### P2: No logout functionality

There is zero logout/sign-out in the entire frontend. Grep confirms: no `logout`, `sign_out`, `signout` anywhere in `.rs` files. Attendees who share devices, or want to switch Google accounts, have no way to clear their session.

The JWT cookie (`event_checkin_token`) is `HttpOnly` — JavaScript can't delete it. A server-side endpoint is needed.

### P3: No "My Events" for signed-in attendees

After registering for Event A, if the attendee visits the landing page `/`, there's:
- No "Welcome back, user@gmail.com"
- No list of their registered events
- No way to navigate to their deposit/ticket pages

The JWT cookie is global (not per-event) — signed-in once, works for all events. But the landing page doesn't use it.

## Proposed Solution

### A. Add `event_slug` to deposit response + fix navigation

| Change | Files |
|--------|-------|
| Add `event_slug: String` to `DepositStatusResponse` | `domain/src/models/deposit.rs` |
| Populate `event_slug` in deposit status handler | `worker/src/handlers/deposit.rs` |
| Add `event_slug` to frontend `DepositStatusResponse` | `frontend-leptos/src/api.rs` |
| Replace `<a href="/">"← Back to home"</a>` → `<a href="/e/{slug}">"← Back to event"</a>` | `frontend-leptos/src/pages/deposit.rs` |

### B. Add logout endpoint + UI

| Change | Files |
|--------|-------|
| New `POST /api/auth/logout` — clears `event_checkin_token` cookie | `worker/src/handlers/auth.rs`, `worker/src/handlers/mod.rs` |
| Logout button on deposit page (small, top-right corner) | `frontend-leptos/src/pages/deposit.rs` |
| Logout button on public event page (when signed in) | `frontend-leptos/src/pages/public_event.rs` |

### C. Add "My Registrations" to landing page

| Change | Files |
|--------|-------|
| New `GET /api/my-registrations` — returns all events the signed-in user has registered for | `worker/src/handlers/register.rs`, `worker/src/handlers/mod.rs` |
| "My Registrations" section on landing page (visible when signed in) | `frontend-leptos/src/pages/landing.rs` |

## Acceptance Criteria

- [ ] Deposit page "← Back to event" links to `/e/:slug` (not `/`)
- [ ] `DepositStatusResponse` includes `event_slug` field
- [ ] `POST /api/auth/logout` clears JWT cookie and returns 200
- [ ] Deposit page shows logout button (visible when signed in)
- [ ] Public event page shows logout button (visible when signed in)
- [ ] After logout, page refreshes to unsigned-in state
- [ ] Landing page shows "My Registrations" section for signed-in users
- [ ] Each registration shows event name + link to `/e/:slug` + status (deposit pending / confirmed / checked-in)

## Multi-Event Auth Clarification

**Attendees sign in once**. The JWT cookie is global (`Path=/api`, `Max-Age=86400`) — not scoped to any event. After Google Sign-In, the cookie is sent with every API request. `/e/event-a`, `/e/event-b`, `/e/event-c` all read the same JWT. No re-authentication needed for multiple events.

## Dependencies

- Issue 016 (Google Sign-In) — must be deployed first
- `worker/src/auth.rs` — `require_identity` middleware for new endpoints

## Refs

- `.handovers/058_attendee_google_auth.md` — Issue 016 implementation
- `docs/ux_roadmap.md` — AF-5 (Google Sign-In) is prerequisite
