# 015 — Event Format Model (In-Person / Online / Hybrid)

## Summary

Add a top-level `EventFormat` enum that controls which subsystems are active: deposit, escrow, physical check-in, quest, NFT mint, and refund. The organizer selects the format at event creation — no separate deposit toggle.

## Format Definitions

| Format | Deposit | Physical Check-In | Quest | NFT | Refund |
|--------|---------|-------------------|-------|-----|--------|
| **In-Person** | ✅ Auto | ✅ Required | Optional | ✅ | ✅ |
| **Online** | ❌ | ❌ | ✅ Required (virtual check-in) | ✅ | ❌ |
| **Hybrid** | ✅ In-person track | ✅ In-person track | Required for online track | ✅ Both | ✅ In-person track |

## Implementation

### Domain (`domain/src/models/event.rs`)
- `EventFormat` enum: `InPerson`, `Online`, `Hybrid` with `has_in_person()`, `has_online()` methods
- Added `event_format` field to `EventMeta`, `EventConfig`, `CreateEventRequest`, `UpdateEventRequest`
- Updated `to_meta()` and `from_global_config()` defaults

### Worker (`worker/src/event_store.rs`)
- `create_event()`: auto-enables deposit when format has in-person track
- `update_event()`: re-syncs deposit when format changes
- `seed_from_config()`: defaults to `InPerson`

### Frontend (`frontend-leptos/src/pages/events_page.rs`)
- Replaced deposit toggle with format selector dropdown
- Format badge on event list cards (color-coded: info/warning/success)
- Auto-sync: selecting Online disables deposit fields, InPerson/Hybrid enables them

### Combined Refund + Close TX (`worker/src/solana_escrow.rs`)
- `build_refund_and_close_transaction()` — atomic TX with both refund + close_deposit instructions
- Replaces separate refund handler — one wallet signature does both
- Removed redundant "Reclaim SOL" button from deposit page

### Scanner Fix (`frontend-leptos/src/pages/scanner.rs`)
- Pre-poll wallet detection on mount (async retry loop) instead of sync call that returns []

## Backward Compatibility
- `EventFormat` defaults to `InPerson` — existing events deserialize correctly via `#[serde(default)]`
- Existing `deposit_enabled: true` events continue working unchanged

## Status

🟢 Complete — domain model, worker logic, frontend UI, combined TX, docs all done.
🟢 Complete — Self-registration API (POST /api/public/register) with Google Sheets append.
🟢 Complete — "Reserve Spot" registration form on public event page.
🟢 Complete — Scanner virtual check-in button for online attendees in hybrid events.
🟢 Complete — Online attendee claim path (quest completion triggers virtual check-in).

## Roadmap

| # | Item | Description | Status | Effort |
|---|------|-------------|--------|--------|
| 1 | EventFormat enum + format selector UI | Domain model + frontend dropdown | ✅ Done | ~1h |
| 2 | Combined refund+close TX | Atomic refund+close_deposit in one signature | ✅ Done | ~1h |
| 3 | Self-registration API | `POST /api/public/register` | ✅ Done | ~2h |
| 4 | "Reserve Spot" on `/e/{slug}` | Registration form on public event page | ✅ Done | ~2h |
| 5 | Online attendee claim path | Quest completion as virtual check-in | ✅ Done | ~1h |
| 6 | Scanner "Online Check-In" for staff | Button for staff to trigger virtual check-in for online attendees | ✅ Done | ~1h |

## Refs

- `c494efd` — feat: event format model + combined refund+close TX
- `f0fc2a7` — feat: self-registration API + Reserve Spot form on public event page
- `949cedd` — feat: scanner virtual check-in for online attendees (hybrid events)
- `13c69a8` — feat: online attendee virtual check-in via quest completion (roadmap #5)
- `.issues/010_deposit_refund_escrow.md` — parent issue
- `docs/business_flows_event_page.md` — updated with format model
- `docs/escrow_protocol.md` — updated with combined TX
- `DISCUSSION.md` — sections 9-11 added
