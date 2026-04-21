# Handover 005: Participation Type Check, Preview Mode, Eth Removal, Leptos Plan

**Date**: 2026-04-21
**Branch**: `main`
**Commit**: `639e1dd` — feat: initial commit - event QR check-in system
**Remote**: `https://github.com/solana-thailand/BeThere.git`

---

## What Happened

Three changes were made to the event check-in system based on user feedback:

1. **Removed `eth_address`** from the entire data model — only `solana_address` remains
2. **Added participation type check** — column Y is now read from Google Sheets, and only "In-Person" attendees can be checked in (not "Online")
3. **Added preview check-in mode** — staff page now has two modes: Preview (show info first, then confirm) and Instant (check in immediately)

---

## Changes Made

### Backend

| File | Change |
|------|--------|
| `src/models/attendee.rs` | Removed `eth_address` field from `Attendee` and `AttendeeRow`. Added `participation_type` field (column Y, index 24). Added `is_in_person()` method with substring matching. Added 8 tests for participation type matching. |
| `src/models/api.rs` | Added `participation_type` to `AttendeeResponse` |
| `src/handlers/checkin.rs` | Added check: denies check-in if `!attendee.is_in_person()` — returns error with participation type |
| `src/handlers/attendee.rs` | Added `is_in_person` and `participation_type` to the single-attendee GET response JSON |
| `src/sheets/client.rs` | Extended sheet read range from `A2:K` to `A2:Z` to include column Y |
| `src/qr/generator.rs` | Removed `eth_address` from test mock |

### Frontend

| File | Change |
|------|--------|
| `frontend/staff.html` | Added mode toggle (Preview / Instant) at top. Added preview panel with attendee details (name, email, ticket, participation, status, ID) and Confirm/Cancel buttons. Renamed submit button to "Look Up". Added mode toggle CSS. |
| `frontend/js/scanner.js` | Rewrote `processScanResult()` to support two modes. Added `showPreview()`, `confirmCheckIn()`, `cancelPreview()`, `performCheckIn()` functions. Added `pendingAttendeeId`/`pendingAttendeeData` state. Added participation type check on frontend side (Online → error). Added debug logging throughout. Updated `hideAllPanels()` to include preview panel. |

---

## Check-In Conditions (Full Flow)

An attendee must pass ALL these checks before check-in:

| # | Condition | Source | Failure Message |
|---|-----------|--------|-----------------|
| 1 | Attendee exists by `api_id` (column A) | Backend + Frontend | "attendee with id '...' not found" |
| 2 | `is_approved()` — status is "Approved" or "CheckedIn" (column H) | Backend + Frontend | "attendee is not approved (status: ...)" |
| 3 | `is_in_person()` — participation type contains "in-person" or "in person" (column Y) | Backend + Frontend | "attendee is not In-Person (participation type: Online)" |
| 4 | `!is_checked_in()` — `checked_in_at` is empty (column I) | Backend + Frontend | "attendee is already checked in" |

### Participation Type Matching

The `is_in_person()` method uses **case-insensitive substring matching**:

```
✅ "In-Person"                    → match
✅ "in-person"                    → match
✅ "IN-PERSON"                    → match
✅ "In Person"                    → match
✅ "In-Person (Physical)"         → match (substring)
✅ "In-Person - On Site"          → match (substring)
❌ "Online"                       → no match
❌ "online"                       → no match
❌ "Virtual"                      → no match
❌ "Hybrid"                       → no match
❌ "" (empty)                     → no match
```

This handles cases where the Google Sheet has longer values like "In-Person (Physical Attendance)".

---

## QR Code Generation

When clicking "Generate QR Codes" in admin dashboard:

- **Column K** (`qr_code_url`) is updated with the check-in URL
- Format: `{SERVER_URL}/staff.html?scan={api_id}`
- Only generated for approved attendees that don't already have a QR URL
- Uses Google Sheets batch update API

---

## Sheet Column Mapping (A-Z)

| Column | Field | Notes |
|--------|-------|-------|
| A (0) | `api_id` | e.g. `gst-GcdL8A3BeFTtfBd` |
| B (1) | `first_name` | |
| C (2) | `last_name` | |
| D (3) | `name` | Display name |
| E (4) | `email` | |
| F (5) | `ticket_name` | |
| G (6) | `solana_address` | Solana wallet address |
| H (7) | `approval_status` | "Approved", "Pending", etc. |
| I (8) | `checked_in_at` | ISO 8601 timestamp when checked in |
| J (9) | *(unused — was eth_address)* | Skipped |
| K (10) | `qr_code_url` | Check-in URL for QR generation |
| Y (24) | `participation_type` | "In-Person" or "Online" |

---

## Staff Page: Check-In Modes

### Preview Mode (default)
1. Scan QR or enter ID → click "Look Up"
2. Preview panel shows: name, email, ticket, participation type, status, ID
3. Staff reviews the info
4. Click "Confirm Check-In" → checks in
5. Click "Cancel" → goes back

### Instant Mode
1. Scan QR or enter ID → click "⚡ Check In"
2. Immediately checks in (no preview step)

---

## Tests

```
running 25 tests ... 25 passed
```

- 17 original tests
- 8 new tests for `is_in_person()` matching (exact, case-insensitive, spaces, long values, online, virtual, empty, unknown)

---

## What's NOT Done Yet

- [ ] **Leptos frontend rewrite** — scaffold exists in `frontend-leptos/` but not compiled
- [ ] **Manual browser test** — user needs to test staff.html with preview mode
- [ ] **Manual browser test** — user needs to test admin.html with participation type display
- [ ] **Push to remote** — done, pushed to `solana-thailand/BeThere`
- [ ] **Access token caching** — still re-fetches Google API token on every request (~2s latency)
- [ ] **CSRF protection** — OAuth flow has no state parameter
- [ ] **Deploy** — not deployed yet, only localhost

---

## Plan: Leptos Frontend (Next Step)

The `frontend-leptos/` directory has a scaffold for Leptos 0.7 CSR. Plan:

1. **Install trunk**: `cargo install trunk` (may conflict with trunk.io tool)
2. **Add wasm target**: `rustup target add wasm32-unknown-unknown`
3. **Compile scaffold**: `cd frontend-leptos && trunk serve`
4. **Fix compilation errors** — Leptos 0.7 API may have drifted
5. **Wire up to Axum API**:
   - Login page → `GET /api/auth/url`
   - Scanner page → `GET /api/attendee/:id`, `POST /api/checkin/:id`
   - Admin page → `GET /api/attendees`, `POST /api/generate-qrs`
6. **Implement preview + instant modes** in Leptos
7. **Implement participation type display** in Leptos
8. **Replace `frontend/` static files** with trunk build output
9. **Serve from Axum** — trunk output goes to `frontend-leptos/dist/`, Axum serves it

### Key Decision Points
- Leptos 0.7 uses `Effect`, `Signal`, `Resource` — need to verify scaffold matches current API
- Consider using `leptos_axum` for SSR vs pure CSR with `trunk`
- CSS: reuse existing `style.css` or rebuild with Leptos-compatible approach

---

## How to Dev/Test

```bash
# Build and run
RUST_LOG=info cargo run --quiet

# Run tests
cargo test --quiet

# Test specific
curl -s http://localhost:3000/api/health

# Browser: login, test staff preview mode, test admin
open http://localhost:3000

# Check console logs for debug output (F12 → Console)
```

---

## Issues Ref

- No `.issues/` files created this session
- Previous issues in `.handovers/001-004` are historical context