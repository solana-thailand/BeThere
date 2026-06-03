# Handover 084: E2E Lifecycle Fix + Devnet Testing

## What Happened

Continued from security audit remediation (13/14 done). Focused on Issue #040 — pre-mainnet escrow E2E test coverage.

### Discovery: `test_lifecycle.sh` was broken

The lifecycle E2E script (`scripts/e2e/test_lifecycle.sh`) referenced two routes that no longer exist:
- `POST /api/escrow/create-vault-ata`
- `POST /api/escrow/create-event`

These were replaced by a combined `POST /api/escrow/init` endpoint (ATA + CreateEvent in one TX) in a previous refactor. The other E2E scripts (`test_escrow_devnet.sh`, `test_rollover_devnet.sh`, `test_rollover_full_lifecycle.sh`) were already updated.

### Discovery: Server-side escrow_status not synced after on-chain deactivation

After submitting a deactivation TX on-chain, the server's cached `escrow_status` in KV still showed `Initialized`. The frontend's admin escrow panel handles this by calling `PUT /api/events/{id}` with `escrow_status: "deactivated"` after the TX confirms. The E2E script was missing this step.

### Fix Applied

1. **`scripts/e2e/test_lifecycle.sh`** — replaced Steps 2+3 (create-vault-ata + create-event) with combined `/escrow/init` + `/escrow/confirm-init`
2. Added `warn()` helper function
3. After deactivation TX confirms, added `PUT /api/events/{id}` with `{"escrow_status": "deactivated"}` to sync server state
4. Re-numbered steps (6 → 5)

### Test Results

```
🧪 BeThere Escrow Lifecycle Test
  ✅ PASS Worker health check
  ✅ PASS Event created
  ✅ PASS Init escrow TX built (combined ATA + CreateEvent)
  ✅ PASS Init escrow submitted
  ✅ PASS Escrow PDA confirmed on-chain
  ✅ PASS Escrow init confirmed with server
  ✅ PASS Deactivate event TX built
  ✅ PASS Deactivate event confirmed on-chain
  ✅ PASS Escrow status updated to deactivated
  ℹ️  INFO Claim forfeited: no USDC deposits (expected — no deposits in lifecycle test)
  ✅ PASS Close event TX built
  ✅ PASS Close event confirmed on-chain
  ✅ PASS Escrow account closed — rent reclaimed
```

### Blocker: Devnet USDC

Circle faucet API changed — the old `POST /api/v1/usdc/solana/devnet` endpoint now returns 307 redirect to not-found. The faucet is now a server-side rendered Next.js SPA.

- **Attendee wallet**: `Cx62DNVtVRa5f4n3cZ5DpVR1JXUe41guJFCJfjsrRbik`
- **USDC ATA**: `BhwNmMhFtQG7gUdTkBUDgSHCP5kJJ65UeftFk25KAZQc` (created this session)
- **USDC balance**: 0 — need manual visit to https://faucet.circle.com

The other 3 E2E scripts (escrow, rollover, full lifecycle) all require devnet USDC and cannot run until the attendee wallet is funded.

## Files Changed

| File | Change |
|------|--------|
| `scripts/e2e/test_lifecycle.sh` | Fixed outdated routes, added status sync, renumbered steps |
| `.issues/040_premainnet_escrow_test_coverage.md` | Updated status with Phase B progress |

## Validation

- `cargo test` in bethere-escrow: **39/39 pass**
- `test_lifecycle.sh` on devnet: **all steps pass** ✅
- Worker health check: ✅
- On-chain TXs confirmed on Solana devnet via `solana confirm`

## Remaining Work

### Immediate (manual)
- [ ] Visit https://faucet.circle.com → connect Solana devnet → send USDC to `Cx62DNVtVRa5f4n3cZ5DpVR1JXUe41guJFCJfjsrRbik`
- [ ] Run `bash scripts/e2e/test_escrow_devnet.sh`
- [ ] Run `bash scripts/e2e/test_rollover_devnet.sh`
- [ ] Run `bash scripts/e2e/test_rollover_full_lifecycle.sh`
- [ ] Run `bash scripts/e2e/run_all_e2e.sh` for full suite

### Backlog
| Priority | Issue | Notes |
|----------|-------|-------|
| 🔴 P1 | #043 Phase D | Data retention & deletion API (PDPA) |
| 🔴 P1 | #046 | D1 primary data store migration |
| 🟡 P2 | #045 VULN-007/013/014 | Remaining security vulnerabilities |
| 🟢 P3 | #040 Phase C | Manual browser test of rollover flow |

## How to Dev/Test

1. `cd worker && npx wrangler dev --port 8787` — start worker
2. `bash scripts/e2e/test_lifecycle.sh` — runs without USDC
3. For USDC tests: visit https://faucet.circle.com first, then run other scripts
4. Full suite: `bash scripts/e2e/run_all_e2e.sh`
