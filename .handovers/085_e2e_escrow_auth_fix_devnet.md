# Handover #085: E2E Escrow Auth Fix + Devnet Test Pass

## What Happened

Fixed missing authentication headers and missing escrow state sync in E2E test scripts, enabling the full `test_escrow_devnet.sh` to pass all 31 tests on Solana devnet with real USDC.

## Problems Found & Fixed

### 1. Missing `Authorization: Bearer dev-token` on Attendee-Authed Endpoints

Four E2E scripts were calling attendee-authenticated endpoints without auth headers:

| Endpoint | Route Group | Auth Required | Scripts Fixed |
|----------|-------------|---------------|---------------|
| `POST /api/deposit/usdc` | attendee_authed | JWT identity | escrow, rollover, full-lifecycle |
| `POST /api/deposit/usdc/webhook` | public (dual auth) | WEBHOOK_SECRET or JWT | escrow, rollover, full-lifecycle |
| `POST /api/deposit/thb/upload` | attendee_authed | JWT identity | escrow |

In DEV_MODE, the server accepts literal string `"dev-token"` as a valid JWT bypass (returns `Claims` with `dev_email`). The scripts were already using this pattern for protected (staff) endpoints but missed it for attendee-authed endpoints.

**Root Cause**: The VULN-002 security fix moved deposit endpoints from public to attendee-authed, but the E2E scripts weren't updated at the same time.

### 2. Missing `confirm-init` Call in `test_escrow_devnet.sh`

The script manually updated the event KV with `escrow_address` and `on_chain_event_id` via `PUT /api/events/{id}`, but never called `POST /api/escrow/confirm-init`. This handler does critical work:
- Verifies the escrow exists on-chain
- Sets `escrow_status: Initialized` in KV
- Creates an audit log entry

Without this, the deactivate/close/claim handlers all rejected requests with `"escrow is not in initialized state (current: none)"`.

**Fix**: Added `confirm-init` call after init TX confirmation.

### 3. Missing Escrow Status Sync After Deactivation

After submitting a deactivation TX on-chain, the server's KV-cached `escrow_status` still showed `initialized`. Added `PUT /api/events/{id}` with `{"escrow_status": "deactivated"}` to sync (same pattern as `test_lifecycle.sh`).

## Test Results

### `test_escrow_devnet.sh` — 31/31 PASS ✅

Full USDC round-trip validated on devnet:
1. Create event with deposit config
2. Init escrow (ATA + CreateEvent in single TX)
3. Confirm init (server-side escrow_status = initialized)
4. Deposit 1 USDC → vault
5. Webhook notification (accepted with dev-token)
6. Deposit verified (on-chain confirmation via confirm endpoint)
7. Wait for event_end to pass
8. Refund TX (USDC back to attendee + close deposit PDA)
9. THB slip upload test
10. Deactivate event + sync server status
11. Claim forfeited deposits
12. Close event + reclaim rent

### `test_lifecycle.sh` — 14/14 PASS ✅

Still passing (regression-free).

### Rollover Scripts — BLOCKED

`test_rollover_devnet.sh` and `test_rollover_full_lifecycle.sh` fail at the rollover step because `rollover_deposit_tx_handler` calls `sheets::get_attendee_by_id()` to verify the attendee's email matches the authenticated user (VULN-009). With dummy sheet_ids, this returns Google Sheets 404.

## Files Modified

| File | Change |
|------|--------|
| `scripts/e2e/test_escrow_devnet.sh` | Added `Authorization: Bearer dev-token` to deposit/usdc, webhook, and thb/upload calls. Added `confirm-init` call after init. Added escrow status sync after deactivate. |
| `scripts/e2e/test_rollover_devnet.sh` | Added auth header to deposit/usdc and webhook calls. |
| `scripts/e2e/test_rollover_full_lifecycle.sh` | Added auth header to deposit/usdc and webhook calls (2 attendee deposits). |
| `.issues/040_premainnet_escrow_test_coverage.md` | Updated Phase B status, added session 2 fixes and test results. |

## Key Learnings

1. **Three-tier auth in E2E scripts**: The worker has three auth levels — public (no auth), attendee_authed (JWT identity), protected (staff JWT). E2E scripts must match the correct auth level per endpoint.

2. **`confirm-init` is mandatory**: Just updating KV with escrow_address isn't enough — the `confirm-init` handler verifies on-chain existence AND sets `escrow_status`, which downstream handlers depend on.

3. **Server-side escrow status is not auto-synced**: On-chain state changes (deactivate, close) don't automatically update the KV-cached `escrow_status`. The frontend admin panel handles this sync; E2E scripts must do the same.

## Remain Work

### Issue #040 Phase B (rollover scripts)
- Option A: Add DEV_MODE bypass in `rollover_deposit_tx_handler` — skip sheet lookup, use wallet from request body directly
- Option B: Create a real Google Sheet with test attendee data
- Option C: Add a test-mode sheet mock (higher effort, more correct)

### Issue #040 Phase C
- Manual browser test of escrow lifecycle in admin panel

### Pre-Mainnet Priorities (unchanged)
1. ~~Devnet E2E escrow testing~~ → Mostly done, rollover blocked by sheets
2. D1 primary data store migration (#046)
3. PDPA Phase D (data deletion API)
4. VULN-007/013/014 (remaining security items)

## How to Dev/Test

```bash
# Start worker
cd worker && npx wrangler dev --port 8787

# Run individual tests
bash scripts/e2e/test_lifecycle.sh        # No USDC needed
bash scripts/e2e/test_escrow_devnet.sh    # Needs devnet USDC in attendee wallet

# Run orchestrated suite
bash scripts/e2e/run_all_e2e.sh           # All 4 scripts
bash scripts/e2e/run_all_e2e.sh --only lifecycle
bash scripts/e2e/run_all_e2e.sh --skip-lifecycle
```

## Issue Refs
- #040 — Pre-mainnet escrow test coverage (this issue)
- #045 — Security audit remediation (auth middleware context)
- Handover #084 — Previous E2E lifecycle fix session
