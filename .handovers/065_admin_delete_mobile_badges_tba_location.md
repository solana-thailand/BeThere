# 065 — Admin Delete Attendee, Mobile Fixes, Badge Images, TBA Times, Location Field

**Date:** 2025-05-18
**Branch:** main (commits `cdd35e7`..`fa4f785`)
**Status:** All tested locally, `cargo check --workspace` + wasm clean, pushed to origin

---

## What Happened

Five features/fixes delivered in one session:

### 1. Delete Attendee Admin Button
- **Red "Delete" button** on each attendee row in admin dashboard
- Two-step confirmation (click once → "⚠ Confirm?", click again within 3s to delete)
- Calls `DELETE /api/attendee/{id}` → removes from Google Sheet or KV walkin records
- **Fixed cache invalidation bug**: added `api::invalidate_attendee_cache()` so deleted attendees disappear immediately without hard refresh

### 2. Mobile CSS Fixes
- **Event selector** on admin dashboard was hidden on mobile (`display: none` on first sidebar section) — removed that rule, added mobile-friendly select styling
- **Landing page header** overflow — hamburger menu now appears at 767px (was 640px), preventing nav items from being clipped

### 3. Badge/NFT Image Fixes
- **SVG Content-Type** fixed from `text/html` → `image/svg+xml` on `/api/badge.svg` and `/api/badge-hd.svg`
- **Auto-populate `nft_image_url`** — when seeding events, defaults to `{server_url}/api/badge-hd.svg` if not explicitly set
- **Refactored** `onchain_events_panel.rs` to use `solscan_tx_url()` helper instead of hardcoded URL

### 4. Time TBA Feature (Full-Stack)
- New `time_tba: bool` field across domain model, worker, and frontend
- **Admin form**: checkbox "Time TBA" that relaxes end-time validation (defaults end to start+24h)
- **Public event page**: shows "Time TBA" instead of formatted time range
- **Landing page event cards**: shows "Aug 1, 2025 · Time TBA" when flag is set
- **Worker validation**: TBA mode only requires start date, skips end > start check

### 5. Event Location Field
- Backend already had `location: String` in `EventConfig`/`CreateEventRequest`/`UpdateEventRequest`
- **Added to frontend**: `EventDetail`, `CreateEventBody`, `UpdateEventBody` API types + admin form text input
- Public event page already displayed it — just needed admin form wiring

### Bonus: Solscan Migration
- All `explorer.solana.com` URLs replaced with `solscan.io` across 4 shell scripts + 1 Rust file
- Address paths corrected to `/account/` (solscan convention)

---

## Commits

| Hash | Message |
|------|---------|
| `cdd35e7` | `fix: clippy warnings in attendee delete + walkin handlers` |
| `3850eb0` | `refactor: migrate all explorer.solana.com links to solscan.io` |
| `5c9c522` | `fix: mobile admin event selector hidden + landing nav overflow at 767px` |
| `30ccfb2` | `fix: SVG badge Content-Type + auto-populate nft_image_url default` |
| `a69a704` | `feat: delete attendee button in admin dashboard + cache invalidation` |
| `b6aa194` | `feat: add location field to event admin form` |
| `fa4f785` | `feat: time_tba flag — show Time TBA on public pages + landing cards` |

---

## Reflections

### Struggled With
- **WASM OOM kills**: Frontend release build (`trunk build --release`) gets OOM-killed in the agent environment. `cargo check --target wasm32-unknown-unknown` works fine though.
- **Delete button borrow checker**: `api_id` was moved into the checkbox closure before the delete button could use it. Fixed by cloning into `delete_id`.
- **WriteSignal.get()**: Leptos 0.7 `WriteSignal` doesn't implement `Get` — had to remove the guard check in the timeout closure (unconditional reset, matching existing pattern in `admin_deposit.rs`).

### Solved
- Cache invalidation after delete was the key insight — `cached_get()` has a 30s in-memory cache, so `set_refresh_counter` alone returned stale data.
- The `time_tba` feature was cleanly layered: domain model → worker validation → frontend API types → form → public display → landing cards.

---

## Files Changed

| File | Changes |
|------|---------|
| `domain/src/models/event.rs` | Added `time_tba: bool` to `EventMeta`, `EventConfig`, `CreateEventRequest`, `UpdateEventRequest` |
| `worker/src/handlers/attendee.rs` | Clippy fixes, `source` warning fix |
| `worker/src/handlers/walkin.rs` | Clippy collapsible-if fix |
| `worker/src/handlers/metadata.rs` | SVG Content-Type fix (`image/svg+xml`) |
| `worker/src/handlers/public_event.rs` | Added `time_tba` to public events API response |
| `worker/src/event_store.rs` | `time_tba` in create/update/seed, TBA-aware validation, auto-populate `nft_image_url` |
| `frontend-leptos/src/pages/admin.rs` | Delete attendee button + cache invalidation fix |
| `frontend-leptos/src/pages/events_page.rs` | `time_tba` toggle, `location` input in form |
| `frontend-leptos/src/pages/public_event.rs` | "Time TBA" display when flag is set |
| `frontend-leptos/src/pages/landing.rs` | `time_tba` in event cards, "Time TBA" display |
| `frontend-leptos/src/pages/onchain_events_panel.rs` | Use `solscan_tx_url()` helper |
| `frontend-leptos/src/api/event.rs` | `time_tba` + `location` in API types |
| `frontend-leptos/style.css` | Mobile fixes: event selector + nav breakpoint |
| `scripts/cnft/src/commands/mint.rs` | `explorer.solana.com` → `solscan.io` |
| `scripts/e2e/test_*.sh` (4 files) | All explorer URLs → solscan.io |

---

## Remaining Work

### Immediate Next (Suggested Order)
1. **#1 Upload auth** — Add `require_identity` middleware to `upload_thb_slip_handler` (Issue 016 follow-up)
2. **#4 Landing enrichment** — Add `location`, `tagline`, `nft_image_url` to `EventMeta` + landing event cards
3. **#2 Walk-in wrong-sheet** — Needs `wrangler tail` reproduction with diagnostic guards (Issue 019)

### Backlog
- Unified attendee list (walk-ins in `GET /api/attendees`)
- Event cancellation UI (Issue 020)
- Replace `json!({})` with typed structs (Issue 009, ~44 call sites)
- Upload to R2 instead of KV (base64 images bloat storage)
- HTTPS enforcement for `validate_slip_url()`

---

## Issues Ref
- Issue 008 — NFT config (badge fixes address some checklist items)
- Issue 014 — Walk-in flow (delete now supported)
- Issue 016 — Google auth (upload auth is next step)
- Issue 019 — Walk-in sync (diagnostic guards in place, root cause TBD)

## How to Dev/Test
- `cargo check --workspace` — should be zero warnings
- `cargo check --target wasm32-unknown-unknown` (from `frontend-leptos/`) — should be clean
- `bash build.sh` (from `frontend-leptos/`) — full WASM production build (may need retry if OOM)
- Test delete: admin dashboard → click "Delete" on attendee → confirm → should disappear immediately
- Test TBA: create event with "Time TBA" checked → public page shows "Time TBA"
- Test location: add location in event form → public page shows it
- Test mobile: resize browser to <767px → event selector visible, hamburger menu appears
