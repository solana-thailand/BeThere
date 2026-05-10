# 030 — Wallet Adapter Frontend & On-Chain Deposit Confirmation (Phase 5.3–5.4)

## What Happened

Implemented the two remaining frontend + backend pieces for the BeThere deposit flow:
1. **Wallet adapter frontend (Phase 5.3)** — Solana wallet detection, connection, and direct TX signing/sending in the deposit page, with QR code fallback for mobile
2. **On-chain deposit confirmation (Phase 5.4)** — Backend polling endpoint to verify USDC deposit transactions via Solana RPC, plus a webhook endpoint for recording TX signatures

## Branch

- `feature/010_deposit_refund_escrow` on `event-checkin/`

## Changes Summary

### Bug Fix: `ApiOk` Response Envelope
- All 10 deposit handlers returned `Json(T)` directly, but the frontend `api_post_json<T>()` and `api_get()` expect responses wrapped in `ApiResponse<T>` format: `{ success: bool, data: Option<T>, error: Option<String> }`
- Other handlers (events, quiz) use `ApiOk::new(data)` which auto-wraps, but deposit handlers were written returning raw `Json()`
- Converted all 10 deposit handlers from `Result<Json<T>, WorkerError>` to `Result<ApiOk<T>, WorkerError>`, changing `Ok(Json(...))` to `Ok(ApiOk::new(...))`
- exception: `deposit_usdc_tx_handler` (Solana Pay TX callback) intentionally kept as raw `Json(DepositTxResponse)` — external wallets expect standard Solana Pay Transaction Response format `{ transaction, message }`

### New API Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/api/deposit/usdc/confirm` | Public | Poll deposit TX confirmation status via Solana RPC. Returns `{ confirmed, tx_signature, solana_pay_url }` |
| `POST` | `/api/deposit/usdc/webhook` | Public | Record TX signature from frontend after wallet sends deposit. Attempts immediate on-chain verification. Also serves as Helius webhook target |

### JS Interop Module (`solana_wallet.js`)
7 exported functions for Solana wallet adapter interaction:

| Function | Purpose |
|----------|---------|
| `getDetectedWallets()` | Returns array of installed wallet names |
| `connectWallet(name)` | Connects wallet, returns base58 public key |
| `getConnectedPublicKey(name)` | Gets public key without prompting (`onlyIfTrusted`) |
| `signAndSendTransaction(name, txB64)` | Decodes base64 TX, signs + sends via wallet |
| `fetchTransactionFromCallback(url)` | Fetches serialized TX from Solana Pay callback URL |
| `isWalletAvailable(name)` | Checks if specific wallet provider is present |

### Page State Machine (Updated)
New states added to `DepositPageState`:
- **`WalletConnected(DepositStatusResponse, String, String)`** — wallet_name, public_key
- **`AwaitingConfirmation(DepositStatusResponse, String, String)`** — wallet_name, tx_signature
- **`DepositConfirmed(DepositStatusResponse, String)`** — tx_signature

### Confirmation Polling
- **Interval**: 2 seconds between polls (3 seconds on error)
- **Max attempts**: 30 (≈60 seconds total)
- **RPC method**: `getSignatureStatuses` — checks `confirmationStatus == "confirmed" || "finalized"`
- **Auto-update**: When confirmed, backend sets `DepositStatus.verified = true` in KV

### Dependency Addition
- **`wasm-bindgen-futures = "0.4"`** added to `frontend-leptos/Cargo.toml` — required for `async` wasm_bindgen extern functions (wallet connect, TX signing)

## Code/Plan Location

- JS interop: `frontend-leptos/js/solana_wallet.js`
- Deposit page: `frontend-leptos/src/pages/deposit.rs`
- API types: `frontend-leptos/src/api.rs`
- Deposit handlers: `worker/src/handlers/deposit.rs`
- Routes: `worker/src/handlers/mod.rs`
- Issue: `.issues/010_deposit_refund_escrow.md`

## Reflection / Struggling / Solved

- **Struggling:** Frontend API calls failed silently — all deposit handlers returned raw `Json(T)` but frontend `api_get()` / `api_post_json()` expected `ApiResponse<T>` envelope with `{ success, data, error }` fields
- **Solved:** Converted all 10 deposit handlers from `Result<Json<T>, WorkerError>` to `Result<ApiOk<T>, WorkerError>` using `ApiOk::new()` wrapper. Kept `deposit_usdc_tx_handler` as raw `Json()` since it serves Solana Pay protocol format
- **Struggling:** Wallet adapter JS interop needed async functions (`connectWallet`, `signAndSendTransaction`) but Leptos WASM didn't have the futures bridge
- **Solved:** Added `wasm-bindgen-futures = "0.4"` dependency and used `JsFuture::from()` to bridge JS promises into Rust async

## Remain Work

### 🔴 Immediate — Testing & Validation
- [ ] **E2E test wallet adapter flow** — Test with real Phantom/Backpack wallets on devnet. Verify TX building, signing, sending, and confirmation polling end-to-end
- [ ] **Verify PDA derivation against on-chain** — The PDA derivation (SHA-256 + Ed25519 curve check in `solana_escrow.rs`) needs validation against `Pubkey::find_program_address` on a real Solana validator
- [ ] **Test `create_event` TX on devnet** — Organizer escrow initialization flow with actual RPC connection
- [ ] **Test deposit TX on devnet** — Full deposit flow: create_event → deposit → confirm
- [ ] **Test Solana Pay QR flow** — Scan QR with mobile wallet, verify TX callback builds correct TX

### 🟡 Technical Debt
- [ ] **Helius webhook integration** — `HeliusWebhookPayload` and `HeliusTransactionData` structs exist but webhook handler uses simpler `UpdateDepositSignatureRequest`. Need separate Helius-specific handler
- [ ] **Wallet Standard deep integration** — Current detection uses legacy `window.solana`/`window.backpack` checks. Should migrate to full Wallet Standard registry
- [ ] **Error recovery for AwaitingConfirmation** — If user closes page during polling, should resume. Store `tx_signature` in URL hash or localStorage
- [ ] **Rate limiting on confirmation endpoint** — `GET /api/deposit/usdc/confirm` is public and could be abused

### 🟢 Phase 5 — Mainnet Readiness
- [ ] Switch USDC mint to `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m`
- [ ] Security review of the escrow program
- [ ] Deploy escrow program to Solana mainnet
- [ ] Deploy worker to Cloudflare Workers production
- [ ] Configure Helius webhook for mainnet TX monitoring

## Issues Ref

- `.issues/010_deposit_refund_escrow.md`
- `.handovers/029_deposit_e2e_validation.md`

## How to Dev/Test

```bash
# Start local worker
cd worker && npx wrangler dev --port 8787

# Seed event + enable deposit
TOKEN="eyJ..." # Generate with Python HMAC-SHA256 JWT
curl -X POST http://localhost:8787/api/events/seed -H "Authorization: Bearer $TOKEN"
curl -X PUT http://localhost:8787/api/events/default -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"deposit_enabled":true,"deposit_amount_usdc":15000000,"deposit_amount_thb":500}'

# Test deposit status
curl "http://localhost:8787/api/deposit/status/test-attendee-1?event_id=default"

# Test USDC deposit (get Solana Pay URL)
curl -X POST http://localhost:8787/api/deposit/usdc -H "Content-Type: application/json" \
  -d '{"attendee_id":"test-attendee-1","event_id":"default"}'

# Record TX signature
curl -X POST http://localhost:8787/api/deposit/usdc/webhook -H "Content-Type: application/json" \
  -d '{"attendee_id":"test-attendee-1","tx_signature":"5xX...","event_id":"default"}'

# Poll confirmation
curl "http://localhost:8787/api/deposit/usdc/confirm?attendee_id=test-attendee-1&event_id=default"
```

## Commits

1. `325b737` — `feat: wallet adapter frontend + on-chain deposit confirmation (Phase 5.3-5.4)`
