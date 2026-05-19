# 069 — Registration Capacity Phase 3 + Deposit UI Fix

## What Happened

Continued from Phase 2 (handover 068) to implement **Phase 3: Organizer Controls + Deposit Deadline** for issue 024. Also fixed a deposit page UI issue (QR code centering, spacing, file picker styling).

## Changes Made

### Deposit Page UI Fix (pre-Phase 3 commit)
- Centered QR code with equal padding using `flex` + `margin: 0 auto` in `.qr-wrapper`
- Added divider between QR scan and upload sections
- Modernized file picker with dashed border, hover effects, styled `::file-selector-button`
- Enlarged upload button with `btn-action-lg`
- Fixed `qr-img` display: block to remove inline baseline gap

### Capacity & Registration Control Form Section (`events_page.rs`)
- Added collapsible "Capacity & Registration Control" section to event create/edit form
- In-person capacity input (shown for in-person/hybrid events)
- Online capacity input (shown for hybrid events only)
- `OnlineOpenMode` selector dropdown (Always Open / Auto when full / Manual Toggle) for hybrid events
- Manual toggle checkbox (shown only when Manual mode is selected and event is hybrid)
- Deposit deadline hours input (shown when deposit is enabled and event has in-person track)
- All inputs conditionally shown/hidden based on `event_format` and `online_open_mode`

### Deposit Deadline Data Pipeline
- Added `deposit_deadline_hours: Option<u32>` to domain `DepositStatusResponse`
- Worker handler (`deposit/usdc.rs`) passes `event.deposit_deadline_hours` through
- Frontend API type (`api/deposit.rs`) includes the new field
- Deposit page shows deadline warning banner: "You have {X}h to complete your deposit..."

### Frontend API Enhancement (`api/event.rs`)
- Added `as_str()` method to `OnlineOpenMode` for select dropdown value binding

## Build Verification
- `cargo check -p event-checkin-domain -p event-checkin-worker` — ✅ zero errors
- `cargo clippy -p event-checkin-domain -p event-checkin-worker` — ✅ zero warnings
- `cargo test -p event-checkin-domain` — ✅ 26/26 passed
- Frontend LSP diagnostics — ✅ zero errors (all 4 modified files clean)

## Commits
1. `67b96e9` — fix: deposit page QR code centering, spacing balance, and file picker styling
2. `20a84cf` — feat: organizer capacity controls + deposit deadline on event form (issue #024 phase 3)

## Plan / Code / Test Locations

| Component | Path |
|-----------|------|
| Capacity form section | `frontend-leptos/src/pages/events_page.rs` (after Event Format section) |
| Deposit deadline warning | `frontend-leptos/src/pages/deposit.rs` (ChoosePayment state) |
| Domain deposit response | `domain/src/models/deposit.rs` (`DepositStatusResponse`) |
| Worker deposit status | `worker/src/handlers/deposit/usdc.rs` (`get_deposit_status_handler`) |
| Frontend deposit API | `frontend-leptos/src/api/deposit.rs` (`DepositStatusResponse`) |
| OnlineOpenMode as_str | `frontend-leptos/src/api/event.rs` |
| QR centering CSS | `frontend-leptos/style.css` (`.qr-wrapper`, `.qr-img-md`, `.file-input-styled`) |

## Reflection — Struggling / Solved

- **Solved**: Conditional rendering of capacity fields — used `<Show>` components gated on `event_format` and `online_open_mode` so only relevant fields appear
- **Solved**: `OnlineOpenMode` select binding — added `as_str()` method mirroring the `EventFormat` pattern
- **Solved**: Deposit deadline data flow — added field to domain → worker → frontend API → page rendering

## Remaining Work

### Phase 3 — Cron Trigger (last item)
- Cron/edge trigger for deposit deadline enforcement (auto-switch `participation_type` from In-Person to Online after deadline)
- Requires either a scheduled worker or an edge-triggered check on deposit status fetch

### Phase 4 — Walk-in Capacity Handling
- Walk-in counts against in-person capacity (enforce in `register_walkin`)
- Warning dialog when walk-in exceeds capacity (staff can override)
- Walk-in blocked for online-only events

## Issues Ref
- `.issues/024_registration_capacity_gating.md` — Phase 3 marked done (cron trigger remaining)

## How to Dev / Test
1. Create/edit an event → set format to Hybrid → verify Capacity section appears
2. Set in-person capacity to 50 → save → verify public event page shows "50 spots"
3. Register until capacity → verify next registration is rejected
4. Set deposit deadline to 24h → register → verify deposit page shows deadline warning
5. Toggle online_open_mode to Manual → toggle online_registration_open → verify frontend updates
