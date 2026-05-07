# Handover 042: Admin Escrow UI + USDC Mint Config

## What Happened

Built the admin frontend for escrow lifecycle management (Deactivate / Claim Forfeited / Close Event) and made the USDC mint address configurable for mainnet deployment.

## Changes

### 1. Admin Escrow Management UI (`frontend-leptos/src/pages/admin_escrow.rs`)

New component with:
- **Wallet Connect** — detect available Solana wallets (Phantom/Solflare), connect organizer wallet
- **3 Lifecycle Steps** as sequential cards:
  1. ⏸ Deactivate Event — stops new deposits, allows refunds
  2. 💰 Claim Forfeited — transfer no-show deposits to organizer USDC account
  3. 🗑 Close Event — reclaim rent, close escrow PDA
- Each step shows ✅ Done / Signing spinner / Sign TX button
- Success/error banner with Solscan link
- Info note: "Order matters — deactivate → claim → close"

Architecture: Uses `action_to_execute` signal + Effect pattern to avoid `FnOnce` closure issues in Leptos 0.7 reactive views.

### 2. Admin Sidebar Integration (`frontend-leptos/src/pages/admin.rs`)

- Added `AdminSection::Escrow` to sidebar enum
- Added "⛓ Escrow Management" sidebar button under "Escrow" heading
- Added `<Show>` block rendering `AdminEscrow` component

### 3. API Functions (`frontend-leptos/src/api.rs`)

Added 3 new API functions following existing pattern:
- `deactivate_event` → POST `/escrow/deactivate-event`
- `claim_forfeited` → POST `/escrow/claim-forfeited`
- `close_event` → POST `/escrow/close-event`

### 4. USDC Mint Config (`worker/src/solana_escrow.rs`)

- Added `USDC_MINT_MAINNET` constant (`EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m`)
- Added `usdc_mint()` function that reads `SOLANA_CLUSTER` env var
- All TX builders now use `usdc_mint()` instead of hardcoded `USDC_MINT_DEVNET`
- Set `SOLANA_CLUSTER=mainnet-beta` for mainnet deployment

## Files Changed

| File | Change |
|------|--------|
| `frontend-leptos/src/pages/admin_escrow.rs` | **New** — 464 lines, escrow management component |
| `frontend-leptos/src/pages/admin.rs` | +19 lines: Escrow sidebar + section |
| `frontend-leptos/src/pages/mod.rs` | +1 line: module registration |
| `frontend-leptos/src/api.rs` | +66 lines: 3 API functions + request/response types |
| `worker/src/solana_escrow.rs` | +28/-8 lines: configurable USDC mint |

## Test Results

| Suite | Count | Status |
|-------|-------|--------|
| Worker unit tests | 39/39 | ✅ |
| Escrow on-chain tests | 22/22 | ✅ |
| Frontend cargo check | — | ✅ |
| Clippy (worker) | — | ✅ Clean |
| Clippy (frontend) | — | ✅ No new warnings |

## Issue 010 Status

| Phase | Status |
|-------|--------|
| Phase 5a: Security Audit + Hardening | ✅ Complete |
| Phase 5b: Worker TX Builders | ✅ Complete |
| Phase 5b: Devnet Deploy + E2E | ✅ Complete |
| Phase 5b: Admin Frontend Buttons | ✅ Complete (this session) |
| Phase 5b: USDC Mint Config | ✅ Complete (this session) |
| Phase 5b: Mainnet Deploy | 🟡 Remaining (need ~1.5 SOL net) |

## How to Test Devnet

1. Open admin dashboard → select event with escrow enabled
2. Click "⛓ Escrow Management" in sidebar
3. Connect Phantom/Solflare wallet
4. Step 1: Click "⚡ Sign TX" for Deactivate → approve in wallet
5. Step 2: Click "⚡ Sign TX" for Claim Forfeited → approve in wallet
6. Step 3: Click "⚡ Sign TX" for Close Event → approve in wallet
7. Each step shows ✅ and Solscan link after success

## Remain Work

- **Mainnet deployment**: `quasar deploy -u mainnet-beta` + set `SOLANA_CLUSTER=mainnet-beta` in worker env
- **Presentation materials**: User wants mermaid flows, user stories, architecture diagram, one-page deck for sponsors
