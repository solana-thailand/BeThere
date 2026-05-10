# Handover 041: Escrow Security Audit + Hardening

## What Happened

Completed a full security audit of the BeThere escrow program (`bethere-escrow/`) and fixed all HIGH and MEDIUM findings before mainnet deployment.

Also diagnosed and fixed a blank page bug caused by missing `lazy_assets.js` in the frontend dist.

## Security Audit Findings (24 total)

### Fixed (Blockers / Must-Fix)

| # | Severity | Finding | Fix |
|---|----------|---------|-----|
| F01 | CRITICAL | No `deactivate_event` instruction — `close_event` unreachable in production | Added `deactivate_event` (discriminator 6) with `has_one(organizer)` + `is_active` check |
| F02 | HIGH | No vault validation in `close_event` — wrong vault could be closed | Added `constraints(*vault.address() == *event_escrow.vault())` |
| F03 | HIGH | No vault validation in `claim_forfeited` — attacker could drain unrelated vault | Same constraint |
| F04 | HIGH | No mint validation in `claim_forfeited` — mint confusion | Added `constraints(*usdc_mint.address() == *event_escrow.usdc_mint())` |
| F05 | HIGH | No mint validation in `refund` | Same constraint |
| F06 | HIGH | No mint validation in `deposit` | Same constraint |
| F07 | MEDIUM | No vault validation in `deposit` — USDC goes to wrong vault | Same vault constraint |
| F08 | MEDIUM | No vault validation in `refund` — refund from wrong vault | Same vault constraint |
| F09 | MEDIUM | `claim_forfeited` used raw `+` for `total_forfeited + forfeited` | Replaced with `checked_add` |
| F10 | MEDIUM | `deposit` used raw `+` for `total_deposited + amount` | Replaced with `checked_add` |
| F11 | MEDIUM | `refund` used raw `+` for `total_refunded + amount` | Replaced with `checked_add` |
| F12 | MEDIUM | No `deposit_amount > 0` check in `create_event` | Added validation |
| F13 | MEDIUM | No `event_end > now` check in `create_event` | Added Clock check |
| F14 | MEDIUM | No vault balance check in `close_event` — tokens could be permanently lost | Added accounting check: `total_deposited == total_refunded + total_forfeited` |

### Not Fixed (Accepted / By Design)

| # | Severity | Finding | Reason |
|---|----------|---------|--------|
| F15 | LOW | `claim_forfeited` callable multiple times | Correct — second call computes 0 forfeited, returns NoForfeitedFunds |
| F16 | LOW | PDA seed uses attendee_deposit.attendee() | Validated by PDA derivation itself — safe |
| F17 | LOW | Refund only checks event_end, not start_time | By design — refunds allowed after event ends |
| F18 | INFO | No USDC mint verification in create_event | Standard — off-chain passes correct mint |
| F19-24 | INFO | Various informational notes | No action needed |

## Files Changed

| File | Change |
|------|--------|
| `bethere-escrow/src/errors.rs` | Added 6 new error variants (VaultMismatch, MintMismatch, InvalidDepositAmount, EventEndInPast, Overflow, VaultNotEmpty) |
| `bethere-escrow/src/instructions/deposit.rs` | Added vault + mint constraints, checked_add for total_deposited |
| `bethere-escrow/src/instructions/refund.rs` | Added vault + mint constraints, checked_add for total_refunded |
| `bethere-escrow/src/instructions/claim_forfeited.rs` | Added vault + mint constraints, checked_add for total_forfeited |
| `bethere-escrow/src/instructions/close_event.rs` | Added vault constraint, accounting-based vault balance check |
| `bethere-escrow/src/instructions/create_event.rs` | Added deposit_amount > 0, event_end > now checks |
| `bethere-escrow/src/instructions/deactivate_event.rs` | **New** — deactivate_event instruction |
| `bethere-escrow/src/instructions/mod.rs` | Added deactivate_event module |
| `bethere-escrow/src/lib.rs` | Added deactivate_event instruction (discriminator 6) |
| `bethere-escrow/src/events.rs` | Added EventDeactivated event (discriminator 6) |
| `bethere-escrow/src/tests.rs` | Added 5 new tests (17→22 total) |
| `frontend-leptos/build.sh` | Copy `lazy_assets.js` to dist (fix blank page) |

## Test Results

| Suite | Result |
|-------|--------|
| `bethere-escrow` | 22/22 passed |
| `event-checkin-worker` | 39/39 passed |
| Worker build (wasm32) | OK |
| Frontend build | OK (after lazy_assets fix) |

### New Tests Added

| Test | Purpose |
|------|---------|
| `test_deactivate_event` | Happy path — is_active set to false |
| `test_deactivate_event_wrong_organizer` | Access control — wrong signer rejected |
| `test_deactivate_event_already_inactive` | Double-deactivate rejected |
| `test_close_event_vault_not_empty` | Accounting check blocks close with unsettled funds |
| `test_full_lifecycle_with_deactivate` | 6-step E2E: create→deposit→check_in→refund→deactivate→close |

## Blank Page Fix

**Root cause**: `scanner.js` has a static `import { loadQrLibraries } from "./lazy_assets.js"` but trunk only copies files directly referenced by `#[wasm_bindgen(module = "...")]`. `lazy_assets.js` was missing from dist, causing the entire module chain to fail silently.

**Fix**: `build.sh` now copies `js/lazy_assets.js` to the snippets dist directory after trunk builds.

## Issues Ref

- Issue 010: Deposit/Refund Escrow — Phase 5a security audit complete

## How to Dev/Test

```bash
# Escrow tests
cd bethere-escrow
quasar build
cargo test  # 22/22

# Worker tests
cargo test -p event-checkin-worker  # 39/39

# Build hardened .so for deployment
quasar build  # outputs to target/deploy/bethere_escrow.so
```

## Remaining Work

### Phase 5b — Deploy to Devnet → Mainnet
- Deploy hardened `bethere_escrow.so` to devnet (re-test full escrow flow)
- Deploy to mainnet with USDC mint `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m`
- Update worker `solana_escrow.rs` with `deactivate_event` TX builder
- Run next Solana Thailand event on BeThere with deposits

### Worker Update Needed
The worker's `solana_escrow.rs` needs a new `build_deactivate_event_tx()` function matching the new discriminator 6 instruction. This is needed for the admin UI to close events.

## Reflection

The security audit found real, exploitable vulnerabilities — the vault/mint substitution attacks (F02-F08) could have led to fund loss on mainnet. The fix pattern is straightforward: constrain every account against the stored escrow state. The `deactivate_event` instruction was a design gap that would have made event cleanup impossible in production.

The blank page debug was a good reminder that WASM module import chains fail silently — there's no partial loading, and a single missing dependency kills the entire app.
