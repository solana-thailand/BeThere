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
🟡 Remaining: self-registration API, public page registration form, online claim path, scanner online check-in button.

## Remaining Roadmap

| # | Item | Description | Effort |
|---|------|-------------|--------|
| 3 | Self-registration API | `POST /api/public/register` for public event page | ~2h |
| 4 | "Reserve Spot" on `/e/{slug}` | Registration form on public event page | ~2h |
| 5 | Online attendee claim path | Quest completion as virtual check-in, KV-based claim tokens | ~3h |
| 6 | Scanner "Online Check-In" for staff | Button for staff to trigger online check-in for online attendees | ~1h |

## Refs

- `c494efd` — feat: event format model + combined refund+close TX
- `.issues/010_deposit_refund_escrow.md` — parent issue
- `docs/business_flows_event_page.md` — updated with format model
- `docs/escrow_protocol.md` — updated with combined TX
- `DISCUSSION.md` — sections 9-11 added
