# Handover 053: Program ID Alignment, Version Validation & Doc Updates

## What Happened
Continued from session "BeThere Escrow Version Field and Padding" which implemented `version: u8` + padding on both account structs. This session focused on aligning the program ID across the codebase and correcting the migration strategy document.

## Changes Made

### 1. Program ID Alignment (Critical Fix)
Discovered a mismatch between `declare_id!()` and the deployment keypair:
- **`declare_id!()`** in `lib.rs` was set to `2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo` (old deployment)
- **Deployment keypair** at `target/deploy/bethere_escrow-keypair.json` resolves to `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`
- The old keypair is no longer present in the project

Updated 3 files:
| File | Change |
|------|--------|
| `bethere-escrow/src/lib.rs` | `declare_id!` → `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` |
| `worker/src/solana_escrow.rs` | `ESCROW_PROGRAM_ID` constant → new program ID + updated 3 PDA test vectors + bump fix (252 not 255) |
| `scripts/verify_pda.rs` | `ESCROW_PROGRAM_ID` constant → new program ID |

### 2. Version-Aware Deserialization (New)
Added `validate_version()` method to both `EventEscrow` and `AttendeeDeposit` structs.
Called at the start of every instruction that reads existing accounts (7 instruction handlers).
Rejects accounts with version != current (error codes 20 and 21).
Added 2 tests: `test_deposit_escrow_version_mismatch` and `test_mark_checked_in_deposit_version_mismatch`.

### 3. Migration Strategy Doc Corrections
`docs/pda_migration_strategy.md` had several inaccuracies:

| Issue | Before | After |
|-------|--------|-------|
| EventEscrow v0 size | "157 bytes (8 discriminator + 149 data)" | 149 bytes (1 disc + 148 fields) |
| AttendeeDeposit v0 size | "84 bytes (8 discriminator + 76 data)" | 84 bytes (1 disc + 83 fields) — correct size, wrong breakdown |
| EventEscrow padding | +35 bytes | +36 bytes |
| AttendeeDeposit padding | +12 bytes | +11 bytes |
| Discriminator size | 8 bytes (Anchor-style) | 1 byte (Quasar) |
| Checklist items | All unchecked | 4 items marked [x] (version, constants, padding, docs) |
| Version history | Missing v1 details | Full v1 description with sizes |

### 4. Devnet Deployment
- Rebuilt with `quasar build` after `declare_id!` update
- Redeployed to `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` on devnet
- Program data: 66,936 bytes, slot 462113150
- Devnet balance remaining: ~3.07 SOL

## Test Results
| Suite | Result |
|-------|--------|
| bethere-escrow (quasar test) | 27/27 passed (+2 version mismatch tests) |
| worker (cargo test) | 39/39 passed |
| domain (cargo test) | 14/14 passed |
| Diagnostics | 0 errors, 0 warnings |

## New PDA Test Vectors (for organizer=9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b, event_id=1)
| PDA | Address | Bump |
|-----|---------|------|
| EventEscrow | `3CzSgvftMgjQE1Du9uyamJe6xVCMmu1tvEhHc172Z4JD` | 255 |
| AttendeeDeposit | `EwGrFaXTJdY8cv3T4d93shtASJZdp1t34Y7rGtbf5Fhi` | 252 |
| Vault ATA | `DXiJimCs3Rzv1i3W93oeRSoxcT8Coeo2YqA7iUaQKndQ` | 255 |

### 5. Full Program ID Audit
Updated old program ID `2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo` → new `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T` across:
- README.md (4 changes including test count update)
- DISCUSSION.md (1 change)
- docs/devnet_e2e_walkthrough.md (4 changes)
- docs/devnet_testing_guide.md (3 changes)
- docs/pda_migration_strategy.md (1 change in Program Identity table)
- scripts/e2e/test_escrow_devnet.sh (1 change)
- scripts/e2e_devnet_test.sh (1 change)
Historical .handovers/ files were intentionally left unchanged.
- The v0 size calculation in the doc was wrong because it used Anchor's 8-byte discriminator but Quasar uses 1-byte discriminators. The original "157 bytes" was never the actual on-chain size.
- The old program (`2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo`) still exists on devnet but its keypair is lost. It's orphaned and can be ignored — the "let it close" rule applies here too.

## Remain Work (from action items)
- [x] Add version-aware deserialization — `validate_version()` on every instruction that reads existing accounts, rejects v0 accounts
- [ ] Run 29 E2E tests against devnet (worker + frontend integration) — requires `npx wrangler dev` running
- [ ] Load test at 100+ concurrent check-ins
- [ ] Write migrate instructions (even if unused)
- [ ] Get mainnet keypair from CTO (~1.5 SOL for rent exemption)
- [ ] Configure production secrets (Helius API key, Google SA, JWT secret)
- [ ] Deploy escrow to mainnet
- [ ] Multi-sig upgrade authority for mainnet

## Issues Ref
- .issues/010_deposit_refund_escrow.md
- .issues/013_escrow_rug_pull_prevention.md

## How to Dev/Test
```bash
# Escrow build + test
cd bethere-escrow && quasar build && quasar test

# Worker tests (run from worker dir, not workspace)
cd worker && cargo test

# Deploy to devnet
cd bethere-escrow && quasar deploy --url devnet

# Verify program on-chain
solana program show C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T --url devnet
```
