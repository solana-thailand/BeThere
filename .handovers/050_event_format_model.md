# 050 — Event Format Model Implementation

## What Happened

Continued from previous session that designed the Event Format model architecture. This session implemented the full stack:

1. **Domain**: Added `EventFormat` enum (`InPerson`, `Online`, `Hybrid`) with helper methods
2. **Worker**: Auto-sync deposit based on format, re-sync on format change
3. **Frontend**: Replaced deposit toggle with format selector dropdown, format badges on cards
4. **Combined TX**: Merged refund + close_deposit into single atomic Solana transaction
5. **Scanner fix**: Wallet detection now pre-polls on mount (async) instead of sync call
6. **Docs**: Updated DISCUSSION.md (sections 9-11), business_flows_event_page.md, escrow_protocol.md

## Code Changes

| File | Change |
|------|--------|
| `domain/src/models/event.rs` | EventFormat enum + field on all event types |
| `worker/src/event_store.rs` | Auto-enable deposit for in-person formats |
| `worker/src/solana_escrow.rs` | `build_refund_and_close_transaction()` combined TX |
| `worker/src/handlers/deposit.rs` | `refund_and_close_tx_handler` replaces separate refund |
| `worker/src/handlers/mod.rs` | Route updated to combined handler |
| `worker/src/handlers/public_event.rs` | event_format in public responses |
| `frontend-leptos/src/api.rs` | EventFormat enum + fields on all types |
| `frontend-leptos/src/pages/events_page.rs` | Format selector, badges, auto-sync |
| `frontend-leptos/src/pages/deposit.rs` | Removed redundant close-deposit button |
| `frontend-leptos/src/pages/scanner.rs` | Async wallet detection on mount |
| `frontend-leptos/src/pages/public_event.rs` | event_format field |
| `DISCUSSION.md` | Sections 9-11: format model, journeys, self-registration |
| `docs/business_flows_event_page.md` | Format-aware flows |
| `docs/escrow_protocol.md` | Combined refund+close TX docs |

## Struggling / Solved

- **Scanner wallet detection**: Sync `get_detected_wallets_js()` was returning `[]` because Phantom injects async. Fixed by pre-polling in `spawn_local` with 10x retry at 300ms intervals (same pattern as events_page.rs).
- **Combined TX account ordering**: Used existing `merge_message_accounts()` helper to merge accounts from both refund and close_deposit instructions into a single ordered message, then resolve indices for each instruction independently.

## Remain Work

| # | Item | Effort |
|---|------|--------|
| 3 | Self-registration API (`POST /api/public/register`) | ~2h |
| 4 | "Reserve Spot" registration form on `/e/{slug}` | ~2h |
| 5 | Online attendee claim path (quest = virtual check-in) | ~3h |
| 6 | Scanner "Online Check-In" button for staff | ~1h |

## How to Dev/Test

1. `cargo check && cargo clippy` — clean build
2. `cd worker && npx wrangler dev` — start worker
3. Hard-refresh browser (Cmd+Shift+R) for new frontend assets
4. Create event → verify format selector appears, deposit auto-syncs
5. Check event list → format badge should show (In-Person = blue, Online = yellow, Hybrid = green)

## Issues Ref

- `.issues/015_event_format_model.md`
- `.issues/010_deposit_refund_escrow.md` (parent)
