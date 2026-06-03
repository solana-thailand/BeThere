# Handover #086: Rollover DEV_MODE Bypass + E2E Test Pass

## What Happened

Implemented the DEV_MODE bypass in `rollover_deposit_tx_handler`, fixed three bugs in the E2E test script, and successfully ran `test_rollover_devnet.sh` to a **29/29 pass** on Solana devnet with real USDC.

### Bugs Fixed in This Session

| Bug | Root Cause | Fix |
|-----|-----------|-----|
| Google Sheets 404 in rollover handler | VULN-009 requires sheet lookup, but DEV_MODE had no bypass | Added DEV_MODE conditional — synthesize `Attendee` with `claims.email` when `dev_mode == true` |
| Deposit TX not confirming (SignatureFailure) | Script hardcoded `ATTENDEE_KEYPAIR` to wrong keypair file, overwriting env var | Changed to `if [ -z "$ATTENDEE_KEYPAIR" ]` conditional |
| Close events fails after deactivate | Server-side escrow status not synced after on-chain deactivation | Added `PUT /api/events/{id}` with `escrow_status: deactivated` after each deactivate TX |
| Deposit not verified before rollover | Missing `confirm-init` and deposit confirm calls | Added `confirm-init` for both escrows + deposit confirm with retry loop |
| Source event deadline too short | 90s event_end_ms expired before deposit could land | Increased to 10 minutes |

### Test Results (Devnet)

| Script | Result | Details |
|--------|--------|--------|
| `test_lifecycle.sh` | ✅ 14/14 | No USDC needed |
| `test_escrow_devnet.sh` | ✅ 31/31 | Full USDC round-trip (deposit → refund) |
| `test_rollover_devnet.sh` | ✅ 29/29 | Full rollover USDC round-trip (deposit → rollover → refund → deactivate) |
| `test_rollover_full_lifecycle.sh` | ❌ Blocked | Needs 2 funded wallets |

### Stale Blockhash Discovery

During debugging, discovered that the KV-cached blockhash was corrupted (contained the escrow program ID instead of a real blockhash). The 30-second TTL eventually expired and fresh blockhashes resolved the issue. The root cause of the corruption is unclear — possibly from a previous run that wrote the wrong value. The `get_latest_blockhash` function should be reviewed for robustness.

### ATTENDEE_KEYPAIR Override Bug

The most impactful bug: the script unconditionally set `ATTENDEE_KEYPAIR="/tmp/bethere-rollover-e2e-attendee.json"` even when the env var was already set to the funded wallet's keypair. This caused `sign_and_submit.py` to sign TXs with the wrong key, producing `SignatureFailure` on-chain.

## Files Modified

| File | Change |
|------|--------|
| `worker/src/handlers/deposit/escrow/status.rs` | Added DEV_MODE bypass for sheet lookup in `rollover_deposit_tx_handler`. Added `Attendee`/`CheckInStatus` import. |
| `scripts/e2e/test_rollover_devnet.sh` | Fixed `ATTENDEE_KEYPAIR` override. Added `confirm-init` for both escrows. Added deposit confirm with retry loop. Increased source event_end_ms to 10 min. Added escrow status sync after deactivate. |
| `scripts/e2e/sign_and_submit.py` | Temporarily changed to `skipPreflight: False` for debugging, reverted to `True`. |
| `.issues/040_premainnet_escrow_test_coverage.md` | Updated Phase B status. |
| `.handovers/086_rollover_dev_mode_bypass.md` | This handover. |

## Validation

- `cargo check -p worker` — clean compile
- `test_rollover_devnet.sh` — 29/29 pass on devnet with real USDC

## Wallet State

| Item | Value |
|------|-------|
| Attendee wallet | `Cx62DNVtVRa5f4n3cZ5DpVR1JXUe41guJFCJfjsrRbik` |
| Attendee SOL | ~0.49 SOL |
| Attendee USDC | ~15 (started at 20, consumed by test runs) |
| Attendee USDC ATA | `BhwNmMhFtQG7gUdTkBUDgSHCP5kJJ65UeftFk25KAZQc` |
| Organizer wallet | `9Bz7p4RWdX7eaR4hFUeCc7aSZjDHsie8q1u8imwavkBN` |

## Remaining Work

### Issue #040 Completion
- [ ] `test_rollover_full_lifecycle.sh` — needs 2 funded wallets (or adaptation for single wallet)
- [ ] Phase C: Manual browser test of escrow lifecycle in admin panel

### Pre-Mainnet Priorities
- [ ] Issue #046: D1 primary data store migration assessment
- [ ] Issue #043 Phase D: Data retention & deletion API (PDPA compliance)
- [ ] VULN-007: Replace FNV-1a with blake3/SHA-256 for on-chain event ID
- [ ] VULN-013: CSP hardening
- [ ] VULN-014: Scope Google Sheet role fallback
- [ ] Issue #047: Instruction introspection for escrow program

## Issues Ref
- #040 — Pre-Mainnet Escrow Test Coverage
- VULN-009 — Rollover email verification
- VULN-006 — DEV_MODE production guard
