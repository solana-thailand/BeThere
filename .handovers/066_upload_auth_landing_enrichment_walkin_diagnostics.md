# Handover 066 — Upload Auth, Landing Enrichment, Walk-in Diagnostics

## What Happened

Continued from handover 065 action items. Implemented 3 tasks and discussed Solana Thailand organizer platform integration.

## Changes

### 1. Upload Auth (Security Fix)

**Problem**: `POST /api/deposit/thb/upload` was in the public router — anyone could upload THB deposit slips for any attendee without authentication.

**Fix**: Moved the route to the `attendee_authed` router which uses `require_identity` middleware (JWT verification without staff check). Added `Extension(claims)` to the handler with audit logging.

**Files**:
- `worker/src/handlers/mod.rs` — route moved from `public` → `attendee_authed`
- `worker/src/handlers/deposit/thb.rs` — added `Extension(claims): Extension<Claims>` + `uploader_email` log

**Impact**: Unauthenticated THB slip uploads now return 401. Frontend `api_post_json` already sends `Authorization: Bearer` token — no frontend changes needed. Deposit page shows "Session expired" toast if user isn't signed in.

### 2. Landing Page Enrichment

**Problem**: Landing page event cards only showed name + date + deposit badge. Missing tagline, location, and badge image that help attendees identify events.

**Fix**: Added `tagline`, `location`, `nft_image_url` to `EventMeta` (backward compatible), public API response, and frontend cards.

**Files**:
- `domain/src/models/event.rs` — 3 new `#[serde(default)]` fields on `EventMeta` + `to_meta()`
- `worker/src/handlers/public_event.rs` — 3 new fields in `list_public_events` JSON
- `frontend-leptos/src/pages/landing.rs` — `PublicEventItem` struct + card rendering:
  - Badge image (80px thumbnail at top)
  - Tagline (italic subtitle below name)
  - Location with 📍 Pin icon (below date)

### 3. Walk-in Wrong-Sheet Diagnostic Logging

**Problem**: Walk-in auto-sync writes to the wrong Google Sheet (issue #019). Root cause unknown.

**Fix**: Enhanced diagnostic logging to trace event resolution. Added `tracing::info!` that logs `requested_event_id`, `resolved_event_id`, `event_name`, `sheet_id`, `sheet_name` right after `resolve_event_with_access`.

**Files**:
- `worker/src/handlers/walkin.rs` — new diagnostic log + clippy formatting fix in `find_walkin_by_any`

**To reproduce the bug**:
```bash
cd worker && npx wrangler tail --format json
# Register a walk-in → look for "walk-in: resolved event config"
# Compare sheet_id with expected Google Sheet
```

**Most likely causes**:
1. Event config in KV has wrong `sheet_id` (human error during creation)
2. Multiple events share the same `sheet_id` (copy-paste)
3. `sheet_name` falls back to global config when empty

## Build Status

- `cargo check --workspace` — ✅ zero warnings, zero errors
- `cargo check --target wasm32-unknown-unknown` (frontend) — ✅ clean
- `cargo clippy -p event-checkin-worker -p event-checkin-domain` — ✅ clean
- Frontend build (`bash build.sh`) — not run (OOM in agent env)

## Commits

| Hash | Message |
|------|---------|
| `d8157cb` | `feat: enrich landing event cards with tagline, location, badge image` |
| `2fd9403` | `fix: require identity middleware on THB slip upload endpoint` |
| `6fc849c` | `refactor: add walk-in event resolution diagnostic logging` |

## Solana Thailand Integration Discussion

Discussed how Solana Thailand Genesis (a community organizer) could use BeThere instead of manually coding event HTML on their Zola static site.

**Current state**: Solana Thailand uses Luma for RSVPs, manual HTML for event announcements, no deposit/check-in/NFT capabilities.

**Recommendation**: Start with Option A (Organizer Self-Service) — zero code changes needed:
1. Add Solana Thailand organizers as `organizer_emails` on their events
2. They use existing `/staff` admin UI to create/manage events
3. Link from their static site to BeThere `/e/{slug}` pages
4. Future: embeddable event widget, custom branding per organizer

**Created issue**: `.issues/023_organizer_platform_integration.md` with 3 options (A: Self-Service, B: Headless API, C: Multi-Tenant Platform)

## Remaining Work / Action Items

| Priority | Item | Description |
|----------|------|-------------|
| 🔴 | **Walk-in wrong-sheet root cause** | Run `wrangler tail`, register walk-in, check `sheet_id` vs expected — Issue #019 |
| 🟡 | **Deposit page auth guard** | Show "Sign in to upload slip" when user isn't authenticated on `/deposit/{id}` |
| 🟡 | **Embeddable event widget** | JS snippet or iframe for partner sites (Solana Thailand) — Issue #023 |
| 🟡 | **Organizer branding on cards** | `organizer_name` / `organizer_logo_url` in `EventMeta` — Issue #023 |
| 🟡 | **Open event creation to organizers** | Currently restricted to super_admin — Issue #023 |
| 🟠 | **Unified attendee list** | Merge walk-ins into `GET /api/attendees` with `source` field — Issue #019 |
| 🟠 | **Event cancellation UI** | Issue #020 |
| 🔵 | **Replace `json!({})` with typed structs** | ~44 call sites remaining — Issue #009 |
| 🔵 | **Upload to R2 instead of KV** | Base64 images bloat storage |

## Issues Refs

- Issue #016 — Attendee Google Auth (✅ done)
- Issue #019 — Walk-in sync/export (🐛 wrong-sheet bug remains)
- Issue #023 — Organizer platform integration (🆕 new)

## How to Dev/Test

```bash
# Build check (fast)
cargo check --workspace
cargo check --target wasm32-unknown-unknown --manifest-path frontend-leptos/Cargo.toml

# Clippy
cargo clippy -p event-checkin-worker -p event-checkin-domain

# Frontend build (run locally)
bash build.sh

# Test: Upload Auth
# 1. Go to /deposit/{attendee_id}?event_id=xxx without signing in
# 2. Upload THB slip → should get "Session expired" error
# 3. Sign in → upload → should succeed

# Test: Landing Enrichment
# 1. Create event with tagline, location, badge image
# 2. Visit landing page → card shows all 3 new fields
# 3. Verify badge image loads, tagline in italic, location with 📍 icon

# Test: Walk-in Diagnostics
# 1. cd worker && npx wrangler tail --format json
# 2. Register walk-in via scanner
# 3. Check "walk-in: resolved event config" log → verify sheet_id matches
```
