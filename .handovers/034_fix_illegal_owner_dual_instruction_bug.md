# Handover 034: Fix IllegalOwner Dual Instruction Bug

## What Happened

Investigated and fixed the root cause of the `IllegalOwner` error in the escrow flow, then validated on devnet.

### Root Cause (3 bugs found)

**Bug 1 — Missing ATA discriminator**: `build_create_vault_ata_transaction()` passed empty `Vec::new()` as instruction data. The ATA program interprets empty data as `Create` (non-idempotent), which fails if the account already exists. Fix: use `vec![1]` for `CreateIdempotent`.

**Bug 2 — Wrong writability for owner account**: The ATA program expects the wallet/owner (`event_escrow`) as `readonly`, but the code had `is_writable: true`. Fix: changed to `is_writable: false`.

**Bug 3 — Wrong account ordering in `create_event`**: The escrow program's `CreateEvent` struct expects accounts in order: `organizer, event_escrow, usdc_mint, vault, rent, ...`. The code had `vault` and `usdc_mint` swapped. Fix: corrected the ordering to match the program's struct definition.

### Devnet Validation Results

| Test | Result | TX Signature |
|------|--------|-------------|
| `create_vault_ata` (with discriminator fix) | ✅ Confirmed | `673fYP1FDbSbY6Fi7JJNGxfZVGqJG2ZT77yyZkFDAn2QRjcYNNErkJ2nmTKPPfMb7qEYrvwiFg7ttu1w2MfWnKZc` |
| `create_event` (with pre-created vault) | ✅ Confirmed | `3QogwsJkM947FE5oPyocbVyxs6kXeyFLEauhwukKTqZ3tnmyRjcMq3tYWW8DuvPRp8yo9G8YMHWsExz42LU3JeuY` |
| `create_event` (without pre-created vault) | ❌ `signer privilege escalated` | CPI to ATA program fails |

### Key Discovery: Two-Step Approach IS Required

The `init(idempotent)` on the vault in the escrow program's `CreateEvent` struct **cannot** create the vault via CPI. The CPI fails with `"signer privilege escalated"` because the framework (quasar_lang) can't correctly sign the ATA creation with the `event_escrow` PDA seeds when the `event_escrow` is using `init` (not `seeds`) constraint.

**Required flow:**
1. `create_vault_ata` — Pre-creates the vault ATA (organizer signs, uses ATA program's `CreateIdempotent`)
2. `create_event` — Creates EventEscrow PDA + validates vault exists (init(idempotent) is no-op)

## Changes Made

### `worker/src/solana_escrow.rs`
1. **Fixed ATA discriminator**: `Vec::new()` → `vec![1]` for `CreateIdempotent`
2. **Fixed event_escrow writability**: `is_writable: true` → `false`
3. **Fixed account ordering**: Swapped `vault` and `usdc_mint` to match program struct
4. **Added ATA program to message accounts**: Extra CPI-only account for the ATA program
5. **Updated comments**: Documented the required two-step approach
6. **Added unit test**: `test_ata_create_idempotent_discriminator`

### `scripts/e2e/test_escrow_devnet.sh` (refactored by sub-agent)
1. **Extracted signing helper**: Created `scripts/e2e/sign_and_submit.py` (67 lines)
2. **Added `sign_and_submit_tx()` bash function**: Replaced 5 inline Python blocks
3. **Reverted `create_vault_ata` to required step**: Removed `--with-vault-ata` flag
4. **Reduced from 1209 → 883 lines**: 326 lines removed through deduplication

### `scripts/e2e/sign_and_submit.py` (new)
- Shared Python helper for signing and submitting Solana transactions
- Takes: `tx_b64`, `keypair_json`, `rpc_url`
- Uses `skipPreflight: True` for idempotent operations

## Test Results

- `cargo test --workspace`: **51/51 pass** (14 domain + 37 worker)
- `cargo clippy --workspace`: **0 warnings**
- `cargo check -p event-checkin-worker`: **clean**
- **Devnet E2E**: `create_vault_ata` ✅ + `create_event` ✅

## Escrow Flow (Validated)

```
1. create_vault_ata  →  Organizer signs  →  Vault ATA created (REQUIRED)
2. create_event      →  Organizer signs  →  EventEscrow PDA initialized
3. deposit           →  Attendee signs   →  USDC transferred to vault
4. mark_checked_in   →  Organizer signs  →  Attendee marked checked-in
5. refund            →  Attendee signs   →  USDC returned to attendee
```

## Remaining Work

### 🔴 High Priority
1. **Test deposit on devnet** — Need attendee with devnet USDC from faucet.circle.com
2. **Test mark_checked_in on devnet** — Organizer signs after deposit
3. **Test refund on devnet** — Complete the full 5-step flow

### 🟡 Medium Priority
4. **Fix escrow program** — The `init(idempotent)` CPI failure is a program-level bug. Consider changing to `has_one` constraint on vault instead of `init(idempotent)` since the vault is pre-created
5. **Update handler** — The `create_event` handler should automatically call `create_vault_ata` first (or verify the vault exists)
6. **Event end time handling** — Refund requires `clock.unix_timestamp > event_end` + `refund_deadline > event_end`

### 🟢 Code Quality
7. **Remove ATA program from message accounts** — Since `create_event` doesn't CPI to ATA (vault is pre-created), the extra account can be removed
8. **Add E2E test for duplicate vault creation** — Verify idempotent behavior
9. **Automate USDC faucet** — Currently requires manual browser visit to faucet.circle.com

## Issues Ref
- Issue 010: Deposit/Refund Escrow
- Issue 007: Devnet E2E Test

## How to Dev/Test

```bash
# 1. Run all tests
cargo test --workspace

# 2. Run escrow-specific tests
cargo test -p event-checkin-worker --lib -- solana_escrow

# 3. Start worker
cd worker && npx wrangler dev --port 8787

# 4. Run E2E script
bash scripts/e2e/test_escrow_devnet.sh

# 5. Quick devnet test (manual)
# Step 1: Create event via API
# Step 2: Build + submit create_vault_ata TX
# Step 3: Build + submit create_event TX
```

## Reflection

The investigation revealed 3 stacked bugs:
1. **ATA discriminator** — Empty data used `Create` instead of `CreateIdempotent`
2. **Account writability** — ATA program expects owner as readonly
3. **Account ordering** — `usdc_mint` and `vault` were swapped in the instruction

Each bug was masked by the next one — fixing just the discriminator revealed the writability issue, and fixing that revealed the ordering issue. The devnet testing was essential to uncover all three.

The two-step approach (create_vault_ata → create_event) is confirmed as required. The escrow program's `init(idempotent)` CPI to the ATA program fails because the quasar_lang framework can't correctly sign the ATA creation with the event_escrow PDA seeds in the context of an `init` constraint. This is a known limitation when using `init` + `init(idempotent)` with token accounts in the same instruction.
