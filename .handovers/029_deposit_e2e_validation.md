# 029 — Deposit E2E Validation & Bug Fixes

## What Happened

Continued Issue 010 Phase 4 — validated the full deposit/refund flow against a local dev server with authentication. Fixed bugs discovered during testing, improved E2E test coverage, and achieved **17 PASS, 0 FAIL** on the full test suite.

## Branch

- `feature/010_deposit_refund_escrow` on `event-checkin/`

## Changes Summary

### Fix: E2E Test Auth Header
- All protected endpoint tests used `Cookie: auth_token=$AUTH_TOKEN` — incorrect cookie name
- The worker expects `event_checkin_token` cookie, but tests should use `Authorization: Bearer $TOKEN` (checked first by middleware)
- Replaced all 7 occurrences across tests 6 (adventure config) and 8d-8h (deposit/refund)

### Fix: Separate Attendee IDs for USDC/THB Tests
- USDC deposit (test 8b) and THB upload (test 8c) both used `e2e-test-attendee-deposit`
- After USDC deposit succeeds, THB upload fails with "attendee already has a deposit" (duplicate prevention working correctly)
- Split into `DEPOSIT_ATTENDEE` (USDC) and `DEPOSIT_ATTENDEE_THB` (THB flow through verify → refund)

### Fix: USDC Mint Address
- `deposit_usdc_handler` hardcoded `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m` with comment "devnet USDC"
- That address is **mainnet** USDC. Devnet USDC is `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`
- Fixed to devnet mint with clear comment for mainnet swap

## E2E Test Results (with AUTH_TOKEN)

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  PASS: 17  FAIL: 0  SKIP: 1
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

| Test | Endpoint | Result |
|------|----------|--------|
| 1 | `GET /api/health` | ✅ PASS |
| 2a | `GET /` | ✅ PASS (frontend served) |
| 2b | HTML loads | ✅ PASS |
| 3a | `GET /api/claim/{fake}` | ✅ PASS (not found) |
| 3b | Error message check | ✅ PASS |
| 4 | `GET /api/adventure/{fake}` | ✅ PASS |
| 5 | Helius mintCompressedNft | ✅ PASS (cNFT minted on devnet) |
| 6a | `GET /api/admin/adventure` | ✅ PASS |
| 6b | `PUT /api/admin/adventure` | ✅ PASS |
| 7 | Adventure save | ⏭️ SKIP (needs CLAIM_TOKEN) |
| 8a | `GET /api/deposit/status/{id}` | ✅ PASS (deposit_enabled=true) |
| 8b | `POST /api/deposit/usdc` | ✅ PASS (Solana Pay URL) |
| 8c | `POST /api/deposit/thb/upload` | ✅ PASS (slip uploaded) |
| 8d | `GET /api/deposit/thb/pending` | ✅ PASS (1 slip) |
| 8e | `GET /api/refund/queue` | ✅ PASS (0 pending) |
| 8f | `POST /api/deposit/thb/verify` | ✅ PASS (verified) |
| 8g | `GET /api/refund/queue` | ✅ PASS (THB attendee in queue) |
| 8h | `POST /api/refund/mark/{id}` | ✅ PASS (refund complete) |

## Full THB Flow Validated

```
Upload slip → List pending (1) → Verify → Queue (1 pending) → Mark refund → Done
```

## Duplicate Deposit Prevention Validated

Both USDC and THB deposits reject with `"attendee already has a deposit"` if the attendee already deposited.

## Code/Plan Location

- E2E tests: `scripts/e2e/test_devnet.sh`
- Deposit handler: `worker/src/handlers/deposit.rs`
- Issue: `.issues/010_deposit_refund_escrow.md`

## Reflection / Struggling / Solved

- **Struggling:** E2E test used wrong cookie name (`auth_token` vs `event_checkin_token`), causing all admin tests to skip or fail silently
- **Solved:** Switched to `Authorization: Bearer` header (checked first by middleware, more reliable for testing)
- **Struggling:** THB upload test failed after USDC deposit because both used same attendee ID
- **Solved:** Split into separate attendee IDs — `DEPOSIT_ATTENDEE` for USDC, `DEPOSIT_ATTENDEE_THB` for THB
- **Struggling:** USDC mint was hardcoded to mainnet address despite comment saying devnet
- **Solved:** Fixed to devnet USDC mint `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`

## Remain Work

### 🔴 Phase 4 Remaining
- [ ] **USDC on-chain deposit TX building** — Worker returns descriptive Solana Pay URL; need `bethere-escrow-client` to build actual on-chain `deposit` instruction
- [ ] **Solana Pay wallet adapter** — Frontend needs wallet connection (Phantom, etc.) to sign/submit deposit transaction
- [ ] **On-chain deposit confirmation** — Webhook/listener to confirm USDC deposits landed on-chain, update `verified: true`
- [ ] **R2 image storage for slips** — THB slip upload currently accepts URL string; need R2 bucket for actual file upload
- [ ] **Deposit link in claim page** — Claim page should show deposit status and link to `/deposit/{attendee_id}`
- [ ] **Test no-show scenario** — deposit → don't check-in → organizer claims forfeited via escrow program

### 🟡 Phase 5 — Mainnet (~2 days)
- [ ] Switch USDC mint to `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m` (mainnet)
- [ ] Security review of escrow program
- [ ] Deploy program + worker to mainnet
- [ ] Run next Solana Thailand event with deposits enabled

## Issues Ref

- `.issues/010_deposit_refund_escrow.md`

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

# Run E2E
AUTH_TOKEN="$TOKEN" bash scripts/e2e/test_devnet.sh
```

## Commits

1. `21eb1b2` — `fix: E2E auth header, separate USDC/THB attendee IDs, correct devnet USDC mint (Issue 010)`
