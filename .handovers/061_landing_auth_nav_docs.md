# Handover 061 — Landing Page Auth-Aware Nav + README Update

**Date**: 2025-06-05
**Issue refs**: AF-9 (landing page auth-aware nav), docs cleanup
**Commits**: (pending commit)
**Deploy**: Version `20be5a83` — 3 new static assets (index.html, JS, WASM)

---

## What Happened

Continued from handover 060. Two parallel workstreams:

1. **Landing page auth-aware navigation** — unified Google Sign-In on landing page
2. **README + docs overhaul** — brought documentation current with issues 015-020

### User Context

The user reported:
- "Landing page has Sign In button that's for staff (the old way) — now we have attendees and organizer/staff sign in"
- Walk-in registration shows success but doesn't sync to Google Sheet (confirmed: sync is manual via admin button)
- Phantom wallet button not visible in production (confirmed: `DEV_MODE=0` in wrangler.toml hides it by design)

---

## What Changed

### 1. Landing Page Auth-Aware Nav (`frontend-leptos/src/pages/landing.rs`, +197/-11)

**New helpers** (module-level functions):
- `AuthState` enum: `Checking | SignedIn(email) | NotSignedIn`
- `trigger_landing_oauth()` — fetches OAuth URL with `?redirect=/`, navigates browser
- `trigger_landing_signout()` — POST `/api/auth/logout` + reload

**Nav bar (desktop)**:
- Not signed in → "Sign In" button triggers Google OAuth
- Signed in (attendee) → email + "Sign Out"
- Signed in (staff) → email + "Scanner" + "Sign Out"
- Signed in (admin) → email + "Dashboard" + "Sign Out"

**Mobile menu**: Same auth-aware pattern with full-width buttons.

**Hero section**: "Sign In" → `/login` replaced with "Find Events ↓" → `#events` anchor.

**Footer**: "Sign In" relabeled to "Staff Portal".

**Upcoming Events**: Added `id="events"` to all 3 section variants for anchor scrolling.

**Zero backend changes** — auth callback already handles non-staff redirect to `/`.

### 2. README Overhaul (`README.md`, +148/-44)

**API Endpoints**: Restructured from flat table to 9 category sections:
- Auth & Users (7 endpoints) — added `POST /auth/logout`, `GET /my-registration/{slug}`, `GET /my-registrations`
- Public Event & Registration (7 endpoints) — added `GET /public/event/{slug}`, `POST /public/register`, `GET /public/ticket/{id}`, badge SVGs, waitlist
- Events Admin (12 endpoints) — cleaned up ordering
- Attendees & Check-In (6 endpoints) — added `undo-checkin`, `flush-cache`
- Walk-in Attendees (4 endpoints) — new section
- Quiz & Adventure (8 endpoints) — reorganized
- Claim & NFT (2 endpoints)
- Deposits (8 endpoints) — added `GET /deposit/usdc/tx`
- Escrow (11 endpoints) — added `close-deposit`, `backfill-wallets`, `sync`, `onchain-webhook`, fixed `create-event` → `init`
- Refunds & Cancellation (5 endpoints) — new section with `batch-thb`, `refund-queue`, `cancel-status`

**Frontend Routes**: Added `/e/{slug}`, `/ticket/{attendee_id}`. Updated descriptions.

**Features**: Added 7 new features (Google Sign-In for attendees, self-registration, My Registrations, wallet error recovery, event cancellation, walk-in management, event format model). Updated descriptions.

**Roles & Access Control**: Added `attendee` role. Updated unauthenticated description.

**What's Built**: Updated from 9 to 10 phases. Added 7 new rows (attendee identity, self-registration, My Registrations, walk-in, cancellation, wallet errors). Updated test count to 68.

### 3. Other Docs

- `docs/ux_roadmap.md` — Added AF-9 tracking item for landing page auth-aware nav
- `docs/business_flows_event_page.md` — Struck through "No attendee identity verification" as ✅ Fixed

---

## What's Not Changed

- `/login` page stays as-is (staff fallback entry point)
- `/e/{slug}` auth flow unchanged (independent OAuth with `?redirect=/e/{slug}`)
- Backend auth callback unchanged (already handles non-staff → `/`)
- Worker code unchanged (same deploy, just new frontend assets)

---

## Build & Deploy

- Frontend WASM built successfully on user's machine (user confirmed enough RAM)
- Build output: 2.8 MB WASM + 69 KB JS + 165 KB CSS + 6.7 KB HTML
- Worker deployed: Version `20be5a83`, 3 new/modified assets uploaded
- No worker code changes — worker WASM unchanged from previous deploy

---

## Remaining Work

### Browser Testing Required
- [ ] Landing page "Sign In" → Google OAuth → redirects back to `/` with auth state
- [ ] Nav bar shows email + role-appropriate links after sign-in
- [ ] "Sign Out" clears session and reloads
- [ ] Mobile menu auth-aware buttons work
- [ ] "Find Events ↓" scrolls to Upcoming Events section
- [ ] Footer "Staff Portal" link goes to `/login`
- [ ] MyRegistrations section appears for signed-in users
- [ ] Staff see "Scanner" link, admins see "Dashboard" link

### Backlog (from ux_roadmap.md)
- P2-1: Real-Time Admin Dashboard (auto-poll)
- P2-2: Batch/Manual Check-In
- P3-1 through P3-7: Nice-to-haves

### Technical Debt
- CI/CD with staging environment (GitHub Actions — considered, deferred)
- 8GB RAM build machine constraints (resolved by user building manually for now)
- `/login` page could be updated to say "Staff & Organizer Portal" for clarity
- README test count badge (L29) may be stale — shows hardcoded "29" in badge URL

---

## Reflection

**Solved**: The landing page auth issue was a UX regression from Issue 016 — we added attendee Google Sign-In but the landing page still linked to the staff-only login page. The fix was clean: reuse the same OAuth pattern from `/e/{slug}` with `?redirect=/`, and make the nav bar auth-aware. Zero backend changes needed because the auth callback already handled non-staff redirect correctly.

**Struggled**: OOM on `cargo check` during local development — the 8GB Mac can't compile Leptos WASM with `CARGO_BUILD_JOBS=1`. LSP diagnostics worked fine though, confirming no compile errors. User resolved by building manually.

**CI/CD**: Discussed setting up GitHub Actions with staging environment (develop → staging, main → production). User deferred as "consider implement" — tracked for future.
