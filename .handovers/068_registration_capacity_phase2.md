# 068 — Registration Capacity Phase 2

## What Happened

Continued from Phase 1 (handover 067) to implement **Phase 2: Capacity Fields + Registration Enforcement** for issue 024.

Added domain types, backend enforcement logic, and frontend capacity display for multi-track registration capacity management.

## Changes Made

### Domain Layer (`domain/src/models/event.rs`)
- Added `OnlineOpenMode` enum: `Always` (default), `AutoOnFull`, `Manual`
- Added 5 new fields to `EventConfig`: `in_person_capacity`, `online_capacity`, `online_open_mode`, `online_registration_open`, `deposit_deadline_hours`
- Added 2 new fields to `EventMeta`: `in_person_capacity`, `online_capacity`
- Added capacity fields to `CreateEventRequest` and `UpdateEventRequest`
- Updated `to_meta()`, `from_global_config()` to include new fields
- All fields use `#[serde(default)]` for backward compatibility with existing KV data

### Worker — Registration Enforcement (`worker/src/handlers/register.rs`)
- Added `enforce_capacity()` function that:
  - Counts in-person attendees from sheet (using `is_in_person()`)
  - Counts online attendees from sheet
  - Counts walk-in attendees from KV (prefix scan `walkin:{event_id}:*`)
  - Rejects in-person registration when capacity reached
  - Rejects online registration when capacity reached
  - Enforces `OnlineOpenMode` gating (Always/AutoOnFull/Manual)
- Capacity check placed **after duplicate check** so existing registrants always get their info returned

### Worker — Public Event (`worker/src/handlers/public_event.rs`)
- Added `count_attendees_by_track()` helper (shared counting logic)
- Added `is_online_registration_open()` helper (OnlineOpenMode logic)
- Public event response now includes capacity data:
  - `in_person_capacity`, `online_capacity` (raw config)
  - `in_person_count`, `online_count` (current registrations)
  - `in_person_remaining`, `online_remaining` (calculated)
  - `in_person_available`, `online_available` (boolean gates)
  - `online_open_mode` (string)

### Worker — Event Store (`worker/src/event_store.rs`)
- `create_event()` wires new fields from request
- `update_event()` handles `Option<Option<u32>>` pattern for nullable capacity updates
- `seed_from_config()` includes defaults for new fields

### Frontend — API Types (`frontend-leptos/src/api/event.rs`)
- Added `OnlineOpenMode` enum with `label()` method
- Added capacity fields to `EventMeta`, `EventDetail`, `CreateEventBody`, `UpdateEventBody`

### Frontend — Public Event Page (`frontend-leptos/src/pages/public_event.rs`)
- `PublicEventData` struct includes all capacity fields from API
- New **capacity indicator card** shows remaining spots with green/red status dots
- Hybrid track dropdown shows remaining spots per track: "In-Person (on-site) — 3 spots left"
- In-person option **disappears** when full (matches design spec)
- Online option hidden when not available (based on OnlineOpenMode)

### Frontend — Events Page (`frontend-leptos/src/pages/events_page.rs`)
- `EventForm` includes capacity fields (string inputs for numeric fields)
- `form_from_detail()` maps capacity data from API
- `CreateEventBody` construction parses capacity strings to `Option<u32>`
- `UpdateEventBody` construction wraps in `Option<Option<u32>>`
- Note: UI inputs for capacity fields not yet added to form (Phase 3)

## Build Verification
- `cargo check -p event-checkin-domain -p event-checkin-worker` — ✅ zero errors
- `cargo clippy -p event-checkin-domain -p event-checkin-worker` — ✅ zero warnings
- `cargo test -p event-checkin-domain` — ✅ 26/26 passed
- Frontend LSP diagnostics — ✅ zero errors

## Plan / Code / Test Locations

| Component | Path |
|-----------|------|
| Domain types | `domain/src/models/event.rs` (OnlineOpenMode, EventConfig, CreateEventRequest, UpdateEventRequest) |
| Capacity enforcement | `worker/src/handlers/register.rs` (`enforce_capacity()`) |
| Public event capacity | `worker/src/handlers/public_event.rs` (`count_attendees_by_track()`, `is_online_registration_open()`) |
| Frontend capacity UI | `frontend-leptos/src/pages/public_event.rs` (capacity indicator, hybrid dropdown) |
| Frontend API types | `frontend-leptos/src/api/event.rs` (OnlineOpenMode, capacity fields) |

## Reflection — Struggling / Solved

- **Solved**: Walk-in attendee counting — they're stored as individual KV keys (`walkin:{event_id}:{email}`), not as a list. Used prefix scan with pagination to count.
- **Solved**: Capacity check ordering — placed after duplicate check so existing registrants always get their data returned regardless of capacity.
- **Solved**: `UpdateEventRequest` uses `Option<Option<u32>>` pattern — outer Option means "was this field provided?", inner Option means "set to Some(50) or clear to None (unlimited)".
- **Solved**: Leptos `disabled` attribute syntax — used the design spec approach of simply hiding the full option instead of showing a disabled option.

## Remaining Work

### Phase 3 — Organizer Controls + Deposit Deadline
- Add capacity input fields to the event create/edit form UI
- Add `OnlineOpenMode` selector dropdown to event form
- Add manual toggle for online registration in staff/admin UI
- Add deposit deadline field + countdown on deposit page
- Cron/edge trigger for deposit deadline enforcement (auto-switch to online)

### Phase 4 — Walk-in Capacity Handling
- Walk-in counts against in-person capacity (enforce in `register_walkin`)
- Warning dialog when walk-in exceeds capacity (staff can override)
- Walk-in blocked for online-only events

### Outstanding
- Frontend form UI doesn't have input fields for capacity settings yet (fields are wired in code but no UI inputs rendered)
- The `is_hybrid_with_options` variable was computed but not yet used for gating (currently the dropdown itself handles gating)

## Issues Ref
- `.issues/024_registration_capacity_gating.md` — Phase 2 marked done

## How to Dev / Test
1. Set `in_person_capacity` on an event via API (PUT /api/events/{id})
2. Register attendees until capacity reached
3. Verify next registration returns "In-person spots are full"
4. Check public event page shows capacity indicator with remaining spots
5. For hybrid events, verify dropdown hides full track
