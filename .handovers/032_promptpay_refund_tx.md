# 032 — PromptPay QR Generation + File Upload + USDC Refund TX Builder

## What Happened

Continued Issue #010 Phase 3/4 — implemented the two remaining frontend gaps identified after the PDA fix:

1. **PromptPay QR generation** for THB bank transfer deposits
2. **USDC refund transaction builder** for on-chain deposit refunds

Both were implemented in parallel using sub-agents.

## Branch

- `feature/010_deposit_refund_escrow` on `event-checkin/`

## Changes Summary

### 1. PromptPay QR (THB Deposit Path)

The deposit page previously had only a text input for "paste slip URL (upload coming soon)". Now:

- **`promptpay_id` field** added to `EventConfig`, `CreateEventRequest`, `UpdateEventRequest`, frontend types
- **`frontend-leptos/js/promptpay_qr.js`** — EMVCo QR generation with CRC16-CCITT checksum, auto-detects phone (10 digits) vs national ID (13 digits)
- **`frontend-leptos/js/file_upload.js`** — file reader as base64 data URL (5MB limit)
- **Deposit page THB card** now shows:
  - PromptPay QR code (rendered via existing QRious library) when `promptpay_id` is configured
  - File upload input for slip image (primary method)
  - Collapsible text input for manual URL paste (fallback)
- **Events admin form** has a new `promptpay_id` input in Deposit Settings section

### 2. USDC Refund TX Builder

The escrow program has a `refund` instruction (discriminator 3) that transfers USDC from vault back to attendee and closes the AttendeeDeposit PDA. Now implemented:

- **`worker/src/solana_escrow.rs`** — `RefundTransaction` struct + `build_refund_transaction()` function
  - Derives EventEscrow, AttendeeDeposit, vault ATA, attendee ATA PDAs
  - Builds instruction with 8 accounts in program-expected order
  - Serializes unsigned transaction for wallet signing
- **`POST /api/escrow/refund`** — public endpoint that validates event/deposit and returns serialized refund TX
- **Frontend types** — `RefundTxRequest`, `RefundTxResponse`, `build_refund_tx()` API function
- **Deposit page** — shows "💰 Refund will be available after the event" note on confirmed deposits

### Files Changed (11 files, +661/-22)

| File | Change |
|------|--------|
| `domain/src/models/event.rs` | Added `promptpay_id` to EventConfig, Create, Update |
| `domain/src/models/deposit.rs` | Added `promptpay_id` to DepositStatusResponse |
| `worker/src/event_store.rs` | Added `promptpay_id` to update_event |
| `worker/src/handlers/deposit.rs` | Added `refund_tx_handler` + populated `promptpay_id` in status |
| `worker/src/handlers/mod.rs` | Added `/escrow/refund` route |
| `worker/src/solana_escrow.rs` | Added `RefundTransaction` + `build_refund_transaction` |
| `frontend-leptos/js/promptpay_qr.js` | **New** — EMVCo PromptPay QR generation |
| `frontend-leptos/js/file_upload.js` | **New** — Base64 file reader |
| `frontend-leptos/src/api.rs` | Added promptpay_id + refund types |
| `frontend-leptos/src/pages/deposit.rs` | PromptPay QR + file upload + refund note |
| `frontend-leptos/src/pages/events_page.rs` | promptpay_id form field |

### Test Results

- `cargo test --workspace`: **50/50 pass** (14 domain + 36 worker)
- `cargo clippy --all-targets`: **0 warnings**
- `cargo check --target wasm32-unknown-unknown`: **✅ Clean**
- `cd frontend-leptos && cargo check`: **✅ Clean**

### Design Decisions

1. **PromptPay QR generated client-side** — no server dependency, follows EMVCo standard
2. **File upload uses base64 data URL** as slip_url — avoids needing R2/S3 setup; acceptable for MVP
3. **Refund endpoint is public** — wallet signature provides authentication (attendee must sign the TX)
4. **`promptpay_id` is optional** — if not set, THB card shows manual text input only (backward compatible)
5. **`#[serde(default)]` on all new fields** — backward compatible, existing clients won't break

## Code/Plan Location

- PromptPay QR: `frontend-leptos/js/promptpay_qr.js`
- File upload: `frontend-leptos/js/file_upload.js`
- Deposit page THB section: `frontend-leptos/src/pages/deposit.rs` (~L814-870)
- Refund TX builder: `worker/src/solana_escrow.rs` `build_refund_transaction()`
- Refund handler: `worker/src/handlers/deposit.rs` `refund_tx_handler()`

## Reflection / Struggling / Solved

- **PromptPay EMVCo format**: Had to implement CRC16-CCITT checksum + TLV encoding correctly. The Thai QR standard uses specific GUI ID `A000000677010112` for PromptPay.
- **Refund account ordering**: The escrow program expects accounts in a specific order (event_escrow, attendee_deposit, attendee signer, vault_ta, attendee_ta, organizer, token_program, system_program). These need to be reordered for Solana wire format (signers first, then writable, then readonly).

## Remain Work

### 🔴 Phase 4 — Devnet E2E with Real Wallets
- [ ] Seed event with organizer wallet + deposit config + promptpay_id
- [ ] Test `create_event` TX on devnet (initialize EventEscrow PDA)
- [ ] Test deposit TX on devnet (USDC via Phantom/Backpack)
- [ ] Test Solana Pay QR with mobile wallet
- [ ] Test refund TX on devnet (after deposit + check-in)
- [ ] Test PromptPay QR rendering (visual check)

### 🟢 Phase 5 — Mainnet
- [ ] Security review of escrow program
- [ ] Switch USDC mint to mainnet (`EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m`)
- [ ] Deploy escrow program to Solana mainnet
- [ ] Deploy worker to Cloudflare Workers production

### 📝 Technical Debt
- Consider R2/S3 for slip image storage instead of base64 data URLs
- Add CI step that cross-validates PDAs against `@solana/web3.js`
- Rate limiting on public endpoints (`/api/deposit/usdc/confirm`, `/api/escrow/refund`)
- Error recovery for `AwaitingConfirmation` state (store tx_signature in URL hash or localStorage)

## Issues Ref

- `.issues/010_deposit_refund_escrow.md`

## How to Dev/Test

```bash
# Run all tests
cargo test --workspace

# Start local worker with dev-mode auth
cd worker
echo 'DEV_MODE = "1"' >> .dev.vars
echo 'DEV_EMAIL = "your-email@example.com"' >> .dev.vars
npx wrangler dev --port 8787

# Test with dev-token
curl -H "Authorization: Bearer dev-token" http://localhost:8787/api/auth/me

# Test refund endpoint (needs verified USDC deposit first)
curl -X POST http://localhost:8787/api/escrow/refund \
  -H "Content-Type: application/json" \
  -d '{"event_id":"default","attendee_id":"test-1","wallet_address":"9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b"}'
```

## Commits

1. `69d0f0d` — `feat: PromptPay QR generation + file upload for THB deposits + USDC refund TX builder (Issue 010 Phase 3/4)`
