# 060 — Wallet Error Recovery, Walk-in Sync/Export, Event Cancellation

## What Happened

Implemented three P2/P3 features in a single session: wallet error recovery messages (P2-3), walk-in attendee CSV export + Google Sheet sync (P2-5), and event cancellation UI with batch THB refund (P3-5).

## Deployed

- **Version**: `6a82aa76-6843-4563-b31e-0a81eb053dd4`
- **Commit**: `bd9601e` on `main`
- **URL**: https://bethere.solana-thailand.workers.dev
- **4 new/modified assets uploaded** (new WASM, new JS with structured errors)

## Changes Summary

### P2-3: Wallet Error Recovery Messages (Issue 018)

**Problem**: JS wallet bridge returned `null` on all failures — user rejection, wrong network, insufficient balance, simulation errors all showed generic "Transaction rejected or failed".

**Solution**:
1. **`solana_wallet.js`** — Changed `doConnect()` and `signAndSendTransaction()` to return structured JSON `{"__wallet_error__":true,"code":...,"message":...,"logs":...}` instead of `null` on error. 8 error return paths updated.
2. **`wallet_error.rs`** (new) — `WalletResult` enum (`Success`/`Error`/`UnknownFailure`), `WalletError` with classification methods (`is_user_rejected()`, `is_insufficient_balance()`, etc.), `user_friendly_message()` for actionable guidance, `translate_api_error()` for server error translation.
3. **Async wrappers** in `deposit.rs`, `claim.rs`, `escrow_init.rs`, `admin_escrow.rs`, `scanner.rs`, `events_page.rs` — All `connect_wallet_js` and `sign_and_send_tx_js` now return `WalletResult` with parsed error details.
4. **~18 call sites** updated to pattern match on `WalletResult` and show specific messages.

### P2-5: Walk-in Phase 4 — CSV Export + Google Sheet Sync (Issue 019)

**Problem**: Walk-in attendees stored in KV only — invisible to organizers post-event, not in Google Sheets, no CSV export.

**Solution**:
1. **`walkin.rs`** — `list_walkin_attendees()` with cursor-based KV pagination, `list_walkin_handler()` (GET `/api/walkin/list`), `walkin_export_csv_handler()` (GET `/api/walkin/export`), `walkin_sync_handler()` (POST `/api/walkin/sync`) with idempotency via `walkin_synced:{event_id}:{email}` KV markers.
2. **`sheets.rs`** — `append_walkin_row()` maps `WalkinAttendee` fields to sheet columns (Name, Email, Phone, Ticket="Walk-in", Status="CheckedIn", Participation="In-Person").
3. **`api.rs`** — Frontend API types: `WalkinAttendeeInfo`, `WalkinListResponse`, `WalkinExportResponse`, `WalkinSyncResponse` + async functions.
4. **`admin.rs`** — "Export Walk-in CSV" and "Sync Walk-ins to Sheet" buttons with result feedback.
5. **`audit_store.rs`** — Added `WalkinSynced` and `WalkinExported` audit actions.

### P3-5: Event Cancellation UI & Batch Refund (Issue 020)

**Problem**: No cancellation workflow. Organizers must refund attendees one by one.

**Solution**:
1. **`event.rs`** — Added `Cancelled` variant to `EscrowStatus` enum.
2. **`deposit.rs`** — `batch_thb_refund_handler()` (POST `/api/refund/batch-thb`), `usdc_refund_queue_handler()` (GET `/api/escrow/refund-queue`), `cancel_status_handler()` (GET `/api/escrow/cancel-status`).
3. **`admin_cancel.rs`** (new, 415 lines) — Cancellation workflow: status grid, THB batch refund, USDC refund queue with "Needs Signature" badges, confirmation modal.
4. **`admin.rs`** — New "Cancellation" section (Alt+8 shortcut) in sidebar.
5. **`api.rs`** — Frontend types: `CancelStatusResponse`, `UsdcQueueItem`, `UsdcRefundQueueResponse`, `BatchThbRefundResponse`.

## Key Technical Decisions

- **Wallet errors via JSON strings**: JS returns `{"__wallet_error__":true,...}` on error, Rust detects prefix and parses. Avoids breaking existing callers while adding error context.
- **THB batch refund = KV-only**: No on-chain component. Admin can instantly mark all THB deposits as refunded.
- **USDC refunds still need attendee signature**: On-chain constraint — organizer cannot force-refund. The UI shows a queue and explains the process.
- **Walkin sync idempotency**: `walkin_synced:{event_id}:{email}` KV markers prevent duplicate sheet rows on re-sync.

## Struggling / Solved

- **OOM on 8GB machine**: `cargo clean` freed 7GB of cached artifacts, then rebuilding from scratch caused OOM kills. Solved by: `CARGO_BUILD_JOBS=1`, building worker first (fewer deps), then frontend. Each took 5-6 minutes with warm cache from previous OOM'd attempts.
- **`cached_get` API mismatch**: The walkin list function used `cached_get(&path, CACHE_TTL)` but `cached_get` takes 1 arg. Fixed to use `cached_get(&path)` + manual `serde_json::from_str`.
- **Missing `Default` derives**: `WalkinExportResponse`, `WalkinSyncResponse`, `WalkinListResponse` needed `#[derive(Default)]` for `ApiResponse<T>` deserialization.
- **Move errors in Leptos view closures**: `r.errors` and `items` moved into `when` closures then used again in child content. Fixed by cloning before the closures.
- **`onclick` vs `on:click`**: In Leptos view macros, `onclick` is an HTML attribute (0-arg closure), `on:click` is the event listener (1-arg closure). Fixed admin_cancel.rs.

## Files Changed (22 files, +2340/-170 lines)

| File | Status | Purpose |
|------|--------|---------|
| `.issues/018_wallet_error_recovery.md` | New | Issue doc |
| `.issues/019_walkin_sync_export.md` | New | Issue doc |
| `.issues/020_event_cancellation.md` | New | Issue doc |
| `frontend-leptos/js/solana_wallet.js` | Modified | Structured error returns |
| `frontend-leptos/src/wallet_error.rs` | New | Wallet error types + translation |
| `frontend-leptos/src/lib.rs` | Modified | Register wallet_error module |
| `frontend-leptos/src/api.rs` | Modified | Walkin + cancel API types |
| `frontend-leptos/src/pages/deposit.rs` | Modified | WalletResult in call sites |
| `frontend-leptos/src/pages/claim.rs` | Modified | WalletResult in call sites |
| `frontend-leptos/src/pages/escrow_init.rs` | Modified | WalletResult in call sites |
| `frontend-leptos/src/pages/admin_escrow.rs` | Modified | WalletResult in call sites |
| `frontend-leptos/src/pages/scanner.rs` | Modified | WalletResult in call sites |
| `frontend-leptos/src/pages/events_page.rs` | Modified | WalletResult in call sites |
| `frontend-leptos/src/pages/admin.rs` | Modified | Walkin buttons + cancel section |
| `frontend-leptos/src/pages/admin_cancel.rs` | New | Cancellation workflow UI |
| `frontend-leptos/src/pages/mod.rs` | Modified | Register admin_cancel module |
| `domain/src/models/event.rs` | Modified | Cancelled EscrowStatus variant |
| `worker/src/audit_store.rs` | Modified | WalkinSynced/Exported audit actions |
| `worker/src/handlers/walkin.rs` | Modified | List/export/sync handlers |
| `worker/src/handlers/deposit.rs` | Modified | Batch THB refund + cancel status + USDC queue |
| `worker/src/handlers/mod.rs` | Modified | New routes |
| `worker/src/sheets.rs` | Modified | append_walkin_row |

## Remain Work

- [ ] P2-1: Real-Time Admin Dashboard (auto-poll/SSE for live stats)
- [ ] P2-2: Batch/Manual Check-In for staff (search by name/email)
- [ ] P3-1: Social Proof — Attendee Deposit Count
- [ ] P3-2: PWA Install Prompt
- [ ] P3-3: Light Mode Toggle
- [ ] P3-4: Thai i18n
- [ ] P3-6: Load Testing (100+ concurrent deposits)
- [ ] P3-7: External Security Audit
- [ ] On-chain: Add `organizer_refund` instruction to escrow program (allows batch USDC refunds without attendee signature)
- [ ] Unified attendee list: merge walk-ins into `GET /api/attendees` response

## How to Dev/Test

1. **Wallet errors**: Open `/e/{slug}`, try deposit with wallet. Reject in wallet → see "You cancelled" message. Wrong network → see network mismatch guidance. Check browser console for structured error JSON.
2. **Walkin export**: Admin dashboard → select event → "Export Walk-in CSV" button. Verify CSV download.
3. **Walkin sync**: Admin dashboard → "Sync Walk-ins to Sheet" → verify sync result (synced/skipped counts). Check Google Sheet for new rows.
4. **Event cancellation**: Admin dashboard → Cancellation section (Alt+8) → see status grid → THB batch refund → USDC refund queue.
5. **API tests**: All new endpoints return 401 without auth. Test with staff JWT cookie.

## Issues Ref

- `.issues/018_wallet_error_recovery.md`
- `.issues/019_walkin_sync_export.md`
- `.issues/020_event_cancellation.md`
