# 012 — Escrow Panel: Migrate Frontend to Single-TX Flow

## Summary

The `EscrowInitState` enum was simplified from 7 variants to 5, and a new combined single-TX backend endpoint `POST /api/escrow/init` was added. The **frontend match arms** in `events_page.rs` still reference the old 7 variants and must be updated.

## Status: IN PROGRESS

- ✅ Enum updated (5 variants: `Idle`, `WalletConnected`, `Initializing`, `Done`, `Error`)
- ✅ Backend `POST /api/escrow/init` implemented and compiles (`worker/` clean)
- ✅ Frontend `api::init_escrow()` function added
- ❌ Frontend match arms in `events_page.rs` NOT yet updated — **8 compile errors**

## Current Compile Errors

```
error[E0599]: no variant named `CreatingVault` found for enum `EscrowInitState`
error[E0599]: no variant named `VaultCreated` found for enum `EscrowInitState`
error[E0599]: no variant named `CreatingEscrow` found for enum `EscrowInitState`
error[E0599]: no variant named `EscrowCreated` found for enum `EscrowInitState`
```

## New Enum (already in place, `events_page.rs` L69-95)

```rust
#[derive(Debug, Clone, PartialEq)]
enum EscrowInitState {
    Idle,
    WalletConnected { wallet_name: String, public_key: String },
    Initializing { wallet_name: String },
    Done { escrow_address: String, vault_address: String, on_chain_event_id: u64, signature: String },
    Error { message: String },
}
```

## What Needs to Change

### File: `frontend-leptos/src/pages/events_page.rs`

**Location:** Match arms at ~L1282–L1531 (the `match &state { ... }` block inside the escrow panel section)

**Old match arms (to be replaced):**
1. `EscrowInitState::WalletConnected` — has 2-step UI with "Step 1: Create Vault ATA" button calling `api::create_vault_ata()`
2. `EscrowInitState::CreatingVault` — spinner
3. `EscrowInitState::VaultCreated` — has "Step 2: Initialize Escrow" button calling `api::create_event_escrow()`
4. `EscrowInitState::CreatingEscrow` — spinner
5. `EscrowInitState::EscrowCreated` — success display

**New match arms needed:**

1. **`WalletConnected`** — Show connected wallet info + single "Initialize Escrow" button. On click:
   - Set state to `Initializing { wallet_name }`
   - Call `api::init_escrow(&InitEscrowRequest { event_id })`
   - Sign TX with `sign_and_send_tx_js(&wn, &resp.transaction)`
   - On success: update form fields (`escrow_address`, `on_chain_event_id`, `vault_address`), set state to `Done`
   - On error: set state to `Error { message }`

2. **`Initializing`** — Spinner: "Initializing escrow via {wallet_name}... Approve the transaction in your wallet."

3. **`Done`** — Success panel showing:
   - ✅ Escrow initialized on-chain
   - Escrow address (code block)
   - Vault address (code block)
   - On-chain event ID
   - Solscan TX link
   - These form fields should be set: `escrow_address`, `on_chain_event_id`

4. **`Error`** — Same as current (error message + retry button)

### Key API differences

| Old (2-step) | New (single-TX) |
|---|---|
| `api::create_vault_ata()` → `api::sign_and_send_tx_js()` → `api::create_event_escrow()` → `api::sign_and_send_tx_js()` | `api::init_escrow()` → `api::sign_and_send_tx_js()` |
| 2 wallet signatures | 1 wallet signature |
| `CreateVaultAtaResponse { transaction, message, vault_address }` | `InitEscrowResponse { transaction, message, escrow_address, vault_address, on_chain_event_id }` |

### Additional tasks (after match arms compile)

- [ ] Lock `escrow_address` field: `readonly` when non-empty
- [ ] Lock `on_chain_event_id` field: `readonly` after escrow initialized
- [ ] Rebuild: `trunk build --release`, copy `lazy_assets.js`
- [ ] Test end-to-end on devnet

## Refs

- Backend: `worker/src/solana_escrow.rs` (`build_init_escrow_transaction`, `merge_message_accounts`)
- Backend: `worker/src/handlers/deposit.rs` (`init_escrow_tx_handler`)
- Backend: `worker/src/handlers/mod.rs` (route `POST /api/escrow/init`)
- Frontend API: `frontend-leptos/src/api.rs` L1280-1302 (`init_escrow`)
- Issue 010 (deposit/refund escrow architecture)
- Handover 041 (escrow security hardening), 042 (admin escrow UI)
