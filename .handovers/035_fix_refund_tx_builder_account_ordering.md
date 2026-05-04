# Handover 035: Full Escrow E2E Devnet Validation

## What Happened

Continued from handover 034. Fixed the `build_refund_transaction` account ordering bug, fixed multiple E2E script issues, and successfully validated the **complete 5-step escrow flow on Solana devnet**.

### Bug Fix: `build_refund_transaction` Account Ordering (6 issues)

| # | Issue | Fix |
|---|-------|-----|
| 1 | Account ordering wrong — `event_escrow` first | `attendee` (Signer) first, matching program struct |
| 2 | Missing `usdc_mint` | Added — required for `init(idempotent)` on `attendee_ta` |
| 3 | Missing `rent` sysvar | Added — required for `init(idempotent)` on `attendee_ta` |
| 4 | Extra `organizer` account | Removed — NOT in the Refund struct |
| 5 | Missing ATA program | Added to message accounts for CPI |
| 6 | Missing `event_id` in instruction data | Changed from `vec![3u8]` to include `event_id.to_le_bytes()` |

### E2E Script Fixes (7 issues)

| # | Issue | Fix |
|---|-------|-----|
| 1 | `spl-token balance --address` wrong flag | Changed to positional `MINT` arg |
| 2 | `spl-token create-account --owner` used pubkey | Changed to keypair file path |
| 3 | Missing `sheet_id` in event creation | Added `sheet_id: "e2e-test-dummy"` |
| 4 | `event_end_ms` in future blocked refund | Changed to `now - 3600` (1 hour past) |
| 5 | Static `TEST_ATTENDEE_ID` caused "already has deposit" | Made unique per run: `e2e-attendee-$(date +%s)` |
| 6 | Deposit flow didn't call webhook | Added `POST /api/deposit/usdc/webhook` with TX signature |
| 7 | Refund wrapped response not submitted | Added signing+submission for `data.transaction` wrapper |

### Refund `init(idempotent)` on `attendee_ta`

Unlike `create_event`'s vault (which failed with `signer privilege escalated`), the refund's `init(idempotent)` works because:
- The `payer` is `attendee` (a top-level Signer, not a PDA)
- The `authority` is `attendee` (same Signer)
- No PDA signing needed — attendee's signature propagates through CPI

## Devnet E2E Results (Final Run)

All 24 tests pass. Complete escrow flow validated:

| Step | Result | TX Signature |
|------|--------|-------------|
| `create_vault_ata` | ✅ Confirmed | `3rAWL2JYMoD7wXWogEsDdDY2HGgHexHq3Y7TfVwJWoqjLLL3G21psfLXMDkW4nBezAV52sGQ6mstcHy4n3p4ffhX` |
| `create_event` | ✅ Confirmed | `5FtLnePoUXbWbTvgycrVtVbmXRryhvJpYKHoR7fGay2rDptjJSk9uMk9rSGqxp8VGTsz5mANQ4RUR2eHDrRy6LNB` |
| `deposit` | ✅ Confirmed | `3AR41X9AdmAYbs9KSwmGVAxroVUvAMKWgGEy7GHvW4n7znMnvttsi48DoE3knRkbtav8QwwhKVJYqWPSYtgoa8sL` |
| `mark_checked_in` | ✅ Confirmed | `LZn3mwLk28838G2dxFANX7M7sMom9TaoTrzgxPLkp543EXZq6MSrjTGMDctrz1FHgNHim6HmYYkHMoQt2yL3C28` |
| `refund` | ✅ Confirmed | `jFqUQdzMtUuNL3G28q2EWRte2mxRLnrPqNGQTNruN9edi5GeE153Tq7knscoZfNLHx6gyYHzxZervJ9fRwWjCwf` |

**Final balances**: Vault=0 USDC, Attendee=18 USDC (deposit refunded successfully)

## Changes Made

### `worker/src/solana_escrow.rs`
1. Fixed refund account ordering to match program's `Refund` struct
2. Added missing `usdc_mint`, `rent_sysvar`, ATA program
3. Removed extra `organizer` account
4. Added `event_id` to instruction data
5. Added ATA program to message accounts for CPI

### `scripts/e2e/test_escrow_devnet.sh`
1. Fixed `spl-token balance` syntax (7 occurrences)
2. Fixed `spl-token address/create-account` — use keypair path
3. Added `sheet_id` to event creation request
4. Set `event_end_ms` to 1 hour in the past for refund validation
5. Made `TEST_ATTENDEE_ID` unique per run
6. Rewrote deposit flow: initiate → callback → webhook → confirm
7. Added refund TX submission for wrapped response format
8. Fixed webhook/confirm response parsing (`data` wrapper)

## Test Results

- `cargo check -p event-checkin-worker`: **clean**
- `cargo test -p event-checkin-worker`: **37/37 pass**
- `cargo clippy --workspace`: **0 warnings**
- **Devnet E2E**: 24/24 pass — full escrow flow validated ✅

## Validated Escrow Flow

```
1. create_vault_ata  →  Organizer signs  →  Vault ATA created       ✅ devnet
2. create_event      →  Organizer signs  →  EventEscrow PDA init    ✅ devnet
3. deposit           →  Attendee signs   →  USDC → vault            ✅ devnet
4. mark_checked_in   →  Organizer signs  →  Attendee checked-in     ✅ devnet
5. refund            →  Attendee signs   →  USDC → attendee         ✅ devnet
```

## Remaining Work

### 🔴 High Priority
1. **Fix `verify_tx_on_chain` confirmation** — The confirm endpoint returned `False` even when the TX was confirmed. Likely a `searchTransactionHistory` issue or Helius RPC latency.

### 🟡 Medium Priority
2. **Add `event_end > now` validation to deposit handler** — Deposits should be rejected after event ends
3. **Clean up Escrow owner display in Step 5** — `solana account` output parsing is broken (`Owner: ?`)
4. **Remove ATA program from `create_event` message accounts** — Since vault is always pre-created, the extra CPI account is unnecessary
5. **Automate USDC faucet** — Currently requires manual browser visit to faucet.circle.com

### 🟢 Code Quality
6. **Refactor duplicated message account building** — The 4-pass ordering loop is repeated in every builder function
7. **Add E2E test for duplicate vault creation** — Verify `CreateIdempotent` is truly idempotent
8. **Update frontend admin UI** — Show the two-step escrow initialization

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

# 4. Run full E2E test
ATTENDEE_WALLET=<wallet-with-usdc> bash scripts/e2e/test_escrow_devnet.sh

# 5. Verify on explorer
# https://explorer.solana.com/tx/<signature>?cluster=devnet
```

## Reflection

The refund builder had 6 bugs — significantly more than expected. The root cause was that it was written before the program was fully specified, with guessed account ordering. Cross-referencing against the program's `#[derive(Accounts)]` struct is essential.

The E2E script required 7 fixes, mostly around CLI command syntax and the Solana Pay two-step flow (initiate → callback). The webhook endpoint was the missing piece for connecting the on-chain deposit confirmation back to the server-side deposit status.

The full escrow flow is now validated on devnet: create_vault_ata → create_event → deposit → mark_checked_in → refund. All 5 steps confirmed on-chain.
