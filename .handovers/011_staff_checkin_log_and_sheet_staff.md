# Handover 011: Staff Check-In Log + Sheet-Based Staff List

**Date**: 2025-06-28
**Branch**: `develop/feature/01_workers_migration`
**Scope**: Log which staff member checked in each participant, read staff emails from a "staff" sheet tab

---

## What Happened

Implemented two new features requested by the user:

1. **Staff check-in logging**: When a staff member checks in a participant, their email is recorded in column J (`checked_in_by`) of the Google Sheet. Both the timestamp (column I) and staff email (column J) are written in a single batch update API call.

2. **Sheet-based staff list**: The auth middleware now checks the "staff" sheet tab (in the same Google Sheet file) for authorized staff emails, in addition to the `STAFF_EMAILS` env var. The lists are unioned — a user is staff if their email appears in either source.

Both features were implemented across all three crates: domain (shared), worker (Cloudflare Workers), and legacy Axum. The admin dashboard frontend was updated to show "by staff@email" on checked-in attendees.

Additionally, the column B name fix from Handover 010 was verified working during the e2e smoke test.

---

## Feature 1: Staff Check-In Log

### Column Mapping Update

| Column | Index | Field | Notes |
|--------|-------|-------|-------|
| A | 0 | `api_id` | |
| B | 1 | `name` | **Fixed in Handover 010** |
| C | 2 | `last_name` | |
| D | 3 | `display_name` | Fallback if B is empty |
| E | 4 | `email` | |
| F | 5 | `ticket_name` | |
| G | 6 | `solana_address` | Optional |
| H | 7 | `approval_status` | |
| I | 8 | `checked_in_at` | ISO 8601 timestamp |
| **J** | **9** | **`checked_in_by`** | **NEW — staff email** |
| K | 10 | `qr_code_url` | Optional |
| Y | 24 | `participation_type` | |

### Implementation Details

- `mark_checked_in(row_index, staff_email, state)` now accepts the staff email parameter
- Uses `batchUpdate` Sheets API to write both columns I and J in a single HTTP call
- The `CheckInResponse` includes `checked_in_by` so the scanner UI can display who checked in
- The `AttendeeResponse` includes `checked_in_by` for the admin dashboard
- The `RecentCheckIn` struct includes `checked_in_by` for the recent check-ins panel

### Frontend Changes

- `admin.js` attendee list: shows "2m ago by staff@email" when attendee is checked in
- `admin.js` recent check-ins panel: shows "timestamp by staff@email"
- Gracefully handles `null`/missing `checked_in_by` (attendees checked in before the feature was added)

---

## Feature 2: Sheet-Based Staff List

### How It Works

1. A "staff" sheet tab exists in the same Google Sheet file
2. Column A contains staff email addresses (header in row 1, emails from row 2)
3. The `get_staff_emails(state)` function reads from `staff!A2:A` range
4. The Worker's `require_auth` middleware calls `is_staff()` which:
   - First checks the static `STAFF_EMAILS` env var (fast path, no API call)
   - If not found in env var, fetches from the "staff" sheet tab (dynamic path)
   - The two lists are unioned — a user is staff if their email is in either source
5. Email comparison is case-insensitive

### Configuration

| Source | Env Var | Notes |
|--------|---------|-------|
| Static (env var) | `STAFF_EMAILS` | Comma-separated emails, set via `wrangler secret put` |
| Dynamic (sheet) | `GOOGLE_STAFF_SHEET_NAME` | Sheet tab name, defaults to `"staff"` |

### Graceful Degradation

- If the "staff" sheet tab doesn't exist, the API returns an error, and the middleware falls back to env var only
- The env var is always checked first (no API call needed), so latency is minimal for known staff
- Sheet fetch adds ~200ms latency for dynamic lookups (only on first unrecognized email)

---

## Files Modified

### Domain Crate (`domain/src/`)

| File | Changes |
|------|---------|
| `config/types.rs` | Added `staff_sheet_name: String` to `SheetsConfig` |
| `models/attendee.rs` | Added `checked_in_by: Option<String>` to `Attendee` and `AttendeeRow`, reads column J (index 9) |
| `models/api.rs` | Added `checked_in_by` to `AttendeeResponse`, `CheckInResponse`, `RecentCheckIn` |
| `qr/generator.rs` | Added `checked_in_by: None` to mock attendee in tests |

### Worker Crate (`worker/src/`)

| File | Changes |
|------|---------|
| `state.rs` | Added `staff_sheet_name` to SheetsConfig init, reads `GOOGLE_STAFF_SHEET_NAME` with fallback `"staff"` |
| `sheets.rs` | Added `get_staff_emails()`, updated `mark_checked_in()` to accept `staff_email` and batch write columns I+J |
| `auth.rs` | Made `is_staff()` async — checks env var first, then sheet. Updated `require_auth` middleware |
| `handlers/checkin.rs` | Passes `claims.email` to `mark_checked_in()`, includes `checked_in_by` in response |
| `handlers/attendee.rs` | Passes `checked_in_by` in `RecentCheckIn` construction |
| `handlers/auth.rs` | Added `staff_sheet_name` to test config |
| `http.rs` | Minor import updates |

### Legacy Axum (`src/`)

| File | Changes |
|------|---------|
| `config/types.rs` | Added `staff_sheet_name` to SheetsConfig |
| `models/attendee.rs` | Added `checked_in_by` to both structs, reads column J |
| `models/api.rs` | Added `checked_in_by` to response types |
| `sheets/client.rs` | Added `get_staff_emails()`, updated `mark_checked_in()` with batch write |
| `handlers/checkin.rs` | Passes staff email to check-in |
| `handlers/attendee.rs` | Passes `checked_in_by` in `RecentCheckIn` |
| `auth/google.rs` | Added `staff_sheet_name` to test config |
| `qr/generator.rs` | Added `checked_in_by: None` to mock attendee |

### Frontend (`frontend/js/`)

| File | Changes |
|------|---------|
| `admin.js` | Shows "by staff@email" in attendee list and recent check-ins panel |

---

## E2E Smoke Test Results

All endpoints tested against `wrangler dev` (port 8787):

| # | Test | Result |
|---|------|--------|
| 1 | Health check | ✅ 200 |
| 2 | Auth Me (valid JWT) | ✅ Returns email, is_staff=true |
| 3 | Attendees list | ✅ 21 approved, names from column B confirmed |
| 4 | Get single attendee | ✅ Full details + checked_in_by field |
| 5 | Check-in In-Person attendee | ✅ **checked_in_by: "ratchapon.poc@gmail.com"** logged |
| 6 | Verify persistence | ✅ Get attendee shows checked_in_by after refresh |
| 7 | Non-staff token rejected | ✅ 403 "user is not in staff allowlist" |
| 8 | Duplicate check-in rejected | ✅ "attendee is already checked in" |

---

## Build & Test Status

| Check | Result |
|-------|--------|
| `cargo check -p event-checkin-domain` | ✅ Zero errors |
| `cargo test -p event-checkin-domain` | ✅ 14 passed |
| `cargo check -p event-checkin-worker --target wasm32-unknown-unknown` | ✅ Zero errors |
| `cargo test -p event-checkin-worker` | ✅ 13 passed |
| `cargo check` (main Axum) | ✅ Zero errors (2 warnings: unused new fields in legacy) |
| `cargo test` (main Axum) | ✅ 25 passed |
| `wrangler dev` | ✅ Starts, serves requests |

Total tests: **52** (25 main + 14 domain + 13 worker)

---

## Where Is the Plan/Code/Test

**Plan**: User request — "insert the staff log, who're checking in participant" and "maybe give the list of email staff in another sheet."

**Code**: 20 files changed across domain, worker, legacy Axum, and frontend.

**Tests**: Existing 52 tests still pass. No new unit tests added for the new features (sheet API calls are integration-level, tested via e2e smoke).

---

## Reflection — Struggles / Solved

### Sub-agents for parallel implementation
Used two sub-agents to update Worker and Legacy Axum crates in parallel. Both completed successfully with no conflicts. Key learning: domain crate must be updated first as the shared foundation, then parallel agents can work on the two consumers independently.

### Worker WASM build time
The `wrangler dev` custom build command compiles the Rust crate to WASM (~60s for release build). This is expected but means the feedback loop for testing is longer than the Axum build. The `--release` flag is required by wrangler's WASM packaging.

### Sheet tab must exist
The `get_staff_emails()` function will fail if the "staff" sheet tab doesn't exist in the Google Sheet. This is handled gracefully — the middleware falls back to the env var `STAFF_EMAILS`. The user needs to create a "staff" sheet tab with emails in column A for the dynamic feature to work.

---

## Remain Work

### Phase 6: Deploy (continued)

| Step | Description | Status |
|------|-------------|--------|
| 6.1 | E2E browser test | ✅ Done |
| 6.2 | Mobile testing | ☐ (post-deploy) |
| 6.3 | Set up production secrets + deploy | ☐ **Next** |
| 6.4 | Configure custom domain | ☐ |
| 6.5 | `npx wrangler deploy` | ☐ |
| 6.6 | Remove legacy files | ☐ |

### Pre-deploy Checklist

1. Create "staff" sheet tab in Google Sheet with staff emails in column A
2. Add column J header "checked_in_by" to the attendee sheet
3. Build frontend: `cd frontend-leptos && trunk build`
4. Set secrets: `npx wrangler secret put <NAME>` for all 9 secrets
5. Set `GOOGLE_STAFF_SHEET_NAME` var in `wrangler.toml` or via `wrangler secret put`
6. Update `SERVER_URL` in `wrangler.toml` to actual Workers URL
7. Deploy: `cd worker && npx wrangler deploy`

---

## Issues Ref

- Handover 010: Column B name fix
- Handover 009: Workers migration (Phases 3-6)
- Handover 008: System review and migration plan

---

## How to Dev / Test

```bash
# Verify all crates build + test
cargo check -p event-checkin-domain
cargo test -p event-checkin-domain    # 14 tests
cargo check -p event-checkin-worker --target wasm32-unknown-unknown
cargo test -p event-checkin-worker    # 13 tests
cargo check                            # legacy Axum
cargo test                             # 25 tests

# Start Worker dev server
cd worker
mv ~/.pnp.cjs ~/.pnp.cjs.bak    # Yarn PnP workaround
npx wrangler dev --port 8787
# In another terminal:
mv ~/.pnp.cjs.bak ~/.pnp.cjs    # Restore after build

# E2e test with crafted JWT
TOKEN="<crafted JWT>"
curl -s -H "Authorization: Bearer $TOKEN" http://localhost:8787/api/attendees | python3 -m json.tool
curl -s -X POST -H "Authorization: Bearer $TOKEN" http://localhost:8787/api/checkin/gst-XXXXX | python3 -m json.tool
```
