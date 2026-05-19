# Handover 067 — Registration Capacity, Claim Gating & Sheet Fix (Phase 1)

## What Happened

Implemented Phase 1 of Issue 024 — the critical fixes for registration capacity, claim timing, and Google Sheets row deletion. This session focused on three backend changes that address real production issues.

## Changes Made

### 1. Sheet Row Deletion Fix (`worker/src/sheets/write.rs`)

**Problem**: `delete_sheet_row` used `spreadsheets.values.clear` which emptied cells but left empty rows. When Google Sheets tries to `INSERT_ROWS` with gaps, it fails with "protected cell" errors.

**Fix**: Replaced with `spreadsheets.batchUpdate` + `DeleteDimensionRequest` which removes the entire row dimension, shifting subsequent rows up. No gaps left.

- New helper `resolve_sheet_gid()` maps tab names to numeric GIDs (Attendees=0, Staff=1)
- Row index converted from 1-based to 0-based for the dimension API
- All caches invalidated after deletion (row indices shift)
- `_mapping` parameter preserved in signature but unused (entire row deleted, no column range needed)

### 2. Online Claim Timing Gate (`worker/src/claim.rs`)

**Problem**: Online attendees could claim NFTs immediately after registration, potentially completing everything before the event occurs.

**Fix**: Added gate in `execute_claim()` that checks if attendee is online and event hasn't ended yet:
- Uses `attendee.is_in_person()` to detect online track
- Compares `chrono::Utc::now().timestamp_millis()` against `event.event_end_ms`
- Returns user-friendly error: "Online claims open after the event ends. Xh Xm remaining."
- Walk-in path intentionally skips this gate (walk-ins are always in-person)

### 3. Online Next Step Changed (`worker/src/handlers/register.rs`)

**Problem**: `build_next_step` returned `step_type: "quest"` with direct claim URL for online attendees, enabling immediate claiming.

**Fix**: Changed online branch to return `step_type: "waiting"` with ticket page URL `/ticket/{api_id}?event_id={event_id}`. The frontend ticket page will show "You're registered! Claims open after the event ends."

- `claim_token` parameter prefixed with `_` (unused for online path now)
- `claim_base` prefixed with `_` (will be needed for hybrid later)

### 4. Documentation Updates

- Created `.issues/024_registration_capacity_gating.md` — full issue with 4 phases
- Updated `docs/business_flows_event_page.md` — Section 19 (capacity, gating, UX wireframes)
- Updated `docs/ux_roadmap.md` — P0.8 items (RC-1 through RC-5)
- Cross-referenced issues 014 and 019 to issue 024

## Build Status

- `cargo check --workspace` — ✅ zero warnings, zero errors
- `cargo clippy -p event-checkin-worker -p event-checkin-domain` — ✅ clean
- `cargo test -p event-checkin-domain` — ✅ 26 passed, 0 failed
- Zed shows 2 stale diagnostics in `register.rs` (WASM cache, not real errors)

## Files Changed

| File | Change |
|------|--------|
| `worker/src/sheets/write.rs` | `delete_sheet_row` rewritten with `DeleteDimensionRequest`, added `resolve_sheet_gid` |
| `worker/src/claim.rs` | Added online claim timing gate in `execute_claim`, comment in `execute_walkin_claim` |
| `worker/src/handlers/register.rs` | Changed `build_next_step` online path from "quest" to "waiting" |
| `.issues/024_registration_capacity_gating.md` | Created — Phase 1 marked done |
| `.issues/014_walkin_attendee_flow.md` | Cross-ref to 024 |
| `.issues/019_walkin_sync_export.md` | Cross-ref to 024 |
| `docs/business_flows_event_page.md` | Added Section 19 |
| `docs/ux_roadmap.md` | Added P0.8 items |

## Remaining Work (Phase 2-4)

| Phase | Scope | Status |
|-------|-------|--------|
| 2 | Capacity fields (`in_person_capacity`, `online_capacity`, `OnlineOpenMode`) + registration endpoint enforcement | Not started |
| 3 | Organizer controls (capacity UI, deposit deadline, manual toggle) | Not started |
| 4 | Walk-in capacity handling (warn + override) | Not started |

## Frontend Note

The `step_type: "waiting"` change requires frontend handling on the ticket page. Currently the ticket page may not have a "waiting" state — it will need to detect this and show "You're registered! Claims open after the event ends on [date]." This is a frontend task for Phase 2.

## How to Test

### Sheet Row Deletion
1. Register an attendee → note their row in Google Sheet
2. Delete the attendee via admin UI → verify the row is completely removed (not just cleared)
3. Register a new attendee → verify it appends without error (no gap rows)

### Online Claim Gate
1. Create an Online event with `event_end_ms` in the future
2. Register as online attendee → try to claim → should get "Online claims open after event ends"
3. Wait for event to end (or update `event_end_ms` to past) → claim should succeed

### Online Next Step
1. Register as online attendee → verify `next_step.step_type == "waiting"`
2. Frontend should redirect to ticket page (not claim page)

## Reflections

- The `resolve_sheet_gid` function uses hardcoded GID mappings. This could break if spreadsheets have non-standard tab structures. A more robust solution would fetch GIDs from the API, but the hardcoded approach covers 99% of cases.
- The online claim gate only applies to the `execute_claim` (POST) path. The `get_claim` (GET) path still returns claim info — this is intentional so the frontend can show "claims open after event ends" rather than a blank page.
- Phase 2-4 require new `EventConfig` fields which need `#[serde(default)]` for backward compatibility with existing KV data.
