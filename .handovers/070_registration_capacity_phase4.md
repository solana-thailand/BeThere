# 070 — Registration Capacity Phase 4 + Serde Bug Fix + QR Alignment

## What Happened

Completed **Phase 4 of Issue #024** (Walk-in Capacity Handling), fixed a **critical production bug** preventing event editing (serde `rename_all` mismatch on `OnlineOpenMode`), added **serde contract tests** to prevent future serialization bugs, and fixed **QR code optical alignment** across all three QR display contexts.

## Changes Made

### Phase 4: Walk-in Capacity Enforcement (`worker/src/handlers/walkin.rs`)
- `WalkinRegisterRequest` gained `override_capacity: bool` field
- Online-only event check: `!event.event_format.has_in_person()` → returns validation error
- `enforce_walkin_capacity()` helper: counts sheet in-person + KV walk-ins, rejects if `>= capacity`
- `CAPACITY_REACHED` error prefix for frontend detection
- Audit log entry for capacity override usage

### Frontend Walk-in Capacity (`frontend-leptos/src/pages/scanner.rs`)
- `CheckInState::WalkinCapacityWarning` — new state storing pending registration data
- Walk-in button disabled for online-only events with text "Walk-in Not Available (Online Event)"
- `active_event_format` and `active_in_person_capacity` signals populated from EventDetail
- Warning dialog with amber border, "Register Anyway (Override)" button
- `handle_walkin_override` — re-submits with `override_capacity: true`

### Frontend API (`frontend-leptos/src/api/attendee.rs`)
- `WalkinRegisterBody` gained `override_capacity: bool` field

### Critical Bug Fix: OnlineOpenMode Serde Mismatch
- **Root cause**: Backend `OnlineOpenMode` has `#[serde(rename_all = "snake_case")]`, serializing as `"always"` / `"auto_on_full"` / `"manual"`. Frontend enum was missing this attribute, expecting `"Always"` / `"AutoOnFull"` / `"Manual"`.
- **Why silent**: `EventMeta` has `#[serde(default)]` on its fields — when deserialization fails, it silently falls back to default. Error only surfaced on `EventDetail` used for edit form.
- **Fix**: Added `#[serde(rename_all = "snake_case")]` to frontend `OnlineOpenMode`

### QR Code Optical Centering Fix
- Set QRious `padding: 0` (was 16) — tight crop, no internal whitespace
- All whitespace controlled by CSS wrapper padding — asymmetric:
  - `.qr-wrapper` (deposit): `1.5rem 1rem 1rem 1.25rem`
  - `.scanner-qr-card` (scanner): `1.25rem 0.75rem 0.75rem 1rem`
  - `.ticket-qr-wrapper` (ticket): `1.25rem 0.75rem 0.75rem 1rem`
- Removed `transform: translate()` hack
- Added `object-fit: contain` to all QR images

### Serde Contract Tests
- `worker/tests/serde_contract.rs` — 17 runnable tests
  - Tests all 8 domain enums round-trip through snake_case JSON
  - Each enum has both `round_trip` and `rejects_pascal_case` tests
  - Integration test parses a full `EventMeta` JSON payload
  - Run: `cd worker && cargo test --test serde_contract`
- `frontend-leptos/tests/serde_contract.rs` — reference contract (documentation only, not runnable as WASM)

## Commits
1. `93a0ba0` — feat: walk-in capacity enforcement + online event blocking (#024 phase 4)
2. `ac0ccc0` — fix: OnlineOpenMode serde snake_case mismatch + QR optical alignment
3. `331d8b4` — feat: serde contract tests + QR optical centering fix

## Build Verification
- `cargo check -p event-checkin-worker` — ✅ zero errors
- `cargo check -p event-checkin-frontend-leptos` — ✅ zero errors
- `cargo test --test serde_contract` (worker) — ✅ 17/17 passed

## Plan / Code / Test Locations

| Component | Path |
|-----------|------|
| Walk-in capacity enforcement | `worker/src/handlers/walkin.rs` (`enforce_walkin_capacity`) |
| Walk-in capacity warning UI | `frontend-leptos/src/pages/scanner.rs` (`WalkinCapacityWarning` state) |
| Walk-in API body | `frontend-leptos/src/api/attendee.rs` (`WalkinRegisterBody`) |
| Serde contract tests (backend) | `worker/tests/serde_contract.rs` |
| Serde contract reference (frontend) | `frontend-leptos/tests/serde_contract.rs` |
| QR CSS fixes | `frontend-leptos/style.css` (`.qr-wrapper`, `.scanner-qr-card`, `.ticket-qr-wrapper`) |

## Reflection — Struggling / Solved

- **Solved**: Serde mismatch — systematic audit of ALL backend/frontend shared enums revealed `OnlineOpenMode` as the only mismatch. Other enums (`EventStatus`, `EscrowStatus`, `EventFormat`, `QuizStatus`, `AdventureStatus`) already had matching `rename_all`.
- **Solved**: QR optical imbalance — root cause was QRious `padding: 16` adding uniform whitespace that doesn't account for QR code's asymmetric visual weight (three finder patterns at top-left, top-right, bottom-left). Fix: zero padding + CSS asymmetric margins.
- **Solved**: Capacity override pattern — using `CAPACITY_REACHED` error prefix lets frontend detect capacity errors specifically without coupling to error codes.

## Remaining Work

- **Deploy to production** — all Phase 1–4 + bug fixes need `wrangler deploy`
- **E2E validation** — event editing, walk-in capacity, online-only blocking, QR visual check
- **Add typed frontend enums** — `DepositMethod`, `CheckInStatus`, `QrGenerationStatus` are still `String` on frontend
- **CI integration** — add `cargo test --test serde_contract` to CI pipeline
- **Issue #019** — walk-in may sync to wrong sheet tab, needs `wrangler tail` investigation

## Issues Ref
- `.issues/024_registration_capacity_gating.md` — All 4 phases complete

## How to Dev / Test
1. Walk-in on at-capacity event → verify warning dialog → override → verify registered
2. Walk-in on online-only event → verify button disabled + API blocked
3. Edit a hybrid event → verify form loads (the serde fix)
4. `cd worker && cargo test --test serde_contract` → verify all 17 pass
5. Visual check: deposit page QR should appear optically centered
