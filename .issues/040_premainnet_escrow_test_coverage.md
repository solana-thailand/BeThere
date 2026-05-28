# Issue #040: Pre-Mainnet Escrow Test Coverage

## Summary

Comprehensive test plan to validate every escrow instruction dimension before mainnet deployment (Phase 10). The rollover deposit feature (Issue 032) added 5 on-chain SVM tests but none cover lifecycle integration — deposit → rollover → refund/claim → close. This issue tracks the missing tests across all three test layers.

## Current Coverage

### On-chain SVM Tests (`bethere-escrow/src/tests.rs`) — 35 tests

| Instruction | Happy Path | Error Cases | Lifecycle Integration |
|-------------|-----------|-------------|----------------------|
| `create_event` | ✅ | ✅ bad deadline | ✅ (full lifecycle) |
| `deposit` | ✅ | ✅ event not active | ✅ |
| `mark_checked_in` | ✅ | ✅ wrong organizer | ✅ |
| `refund` | ✅ | ✅ not checked in, already refunded, after deadline | ✅ |
| `claim_forfeited` | ✅ | ✅ before deadline, nothing to claim, checked-in rejected | ✅ |
| `deactivate_event` | ✅ | ✅ wrong organizer, already inactive | ✅ |
| `close_event` | ✅ | ✅ still active, vault not empty | ✅ |
| `close_deposit` | ✅ | ✅ not refunded, wrong signer, GC after close | ✅ |
| `rollover_deposit` | ✅ | ✅ not checked in, already refunded, target not active, diff org | **❌ no lifecycle** |

### Worker Unit Tests — 48 tests
Covers: escrow indexer, wallet validation, sheets encoding, auth, cleanup. No gaps.

### Devnet E2E Scripts — 4 scripts

| Script | Flow |
|--------|------|
| `test_escrow_devnet.sh` | Init → deposit → check-in → refund → THB → deactivate → claim forfeited → close |
| `test_rollover_devnet.sh` | Source deposit → check-in → rollover → verify target deposit (**stops here**) |
| `test_lifecycle.sh` | Create → deactivate → claim forfeited → close (no deposits) |
| `test_full_e2e.sh` | Browser flow: auth → check-in → quiz → adventure → mint cNFT |

## Gaps to Fill

### Phase A: On-chain SVM Tests (this issue)

Add 4 lifecycle integration tests to `bethere-escrow/src/tests.rs`:

#### 1. `test_rollover_then_refund_from_target`
- Create source + target events
- Deposit → check-in on source
- Rollover to target
- Refund attendee from **target** event (after deadline)
- Verify: source vault = 0, target vault = 0, attendee TA = original amount
- Verify: source deposit marked refunded, target deposit marked refunded

#### 2. `test_double_rollover_rejected`
- Create source + target events
- Deposit → check-in on source
- First rollover succeeds
- Second rollover from **same source** fails (source deposit already refunded via rollover)

#### 3. `test_rollover_then_claim_forfeited_target`
- Create source + target events, two attendees
- Attendee A: deposit → check-in → rollover to target
- Attendee B: deposit → check-in on target → **no show** (doesn't claim refund)
- Deactivate target → claim_forfeited
- Verify: organizer receives attendee B's deposit, not attendee A's

#### 4. `test_rollover_then_close_source`
- Create source + target events
- Deposit → check-in → rollover
- Deactivate source → close source event
- Verify: source escrow closed, rent reclaimed
- Verify: target escrow still holds rollover deposit

### Phase B: Devnet E2E Scripts (this issue)

#### 5. Extend `test_rollover_devnet.sh`
- Add steps after rollover verification:
  - Build & submit refund from target event
  - Verify vault balances
  - Deactivate + close both events

#### 6. Create `scripts/e2e/test_rollover_full_lifecycle.sh`
- Full lifecycle with two events:
  - Source: create → deposit → check-in → deactivate
  - Rollover deposit (attendee signs)
  - Target: refund attendee → deactivate → claim forfeited → close
  - Source: close
- Expected: 20+ assertions

#### 7. Create `scripts/e2e/run_all_e2e.sh`
- Orchestrator: runs all E2E scripts sequentially with shared state
- Reports pass/fail summary
- Usage: `bash scripts/e2e/run_all_e2e.sh`

### Phase C: Manual Browser Test (from Handover 077)
- Already documented in Handover 077 action items

## Acceptance Criteria

### Phase A
- [x] 4 new SVM tests added to `bethere-escrow/src/tests.rs`
- [x] All 38 SVM tests pass (`quasar test`)
- [x] No changes to existing tests (pure additions)

### Phase B
- [x] `test_rollover_devnet.sh` extended with refund-from-target + deactivate + close steps
- [x] `test_rollover_full_lifecycle.sh` created (2 attendees, USDC round-trip, forfeited claim)
- [x] `run_all_e2e.sh` orchestrator created (4 scripts, pass/fail/skip summary)
- [ ] All scripts pass on devnet

### Phase C
- [ ] Manual browser test of rollover flow documented

## Files to Modify

| File | Change |
|------|--------|
| `bethere-escrow/src/tests.rs` | Add 4 lifecycle tests (~400 lines) |
| `scripts/e2e/test_rollover_devnet.sh` | Add refund + close steps |
| `scripts/e2e/test_rollover_full_lifecycle.sh` | New file |
| `scripts/e2e/run_all_e2e.sh` | New file |

## Dependencies
- Issue 032 — Rollover deposit (completed)
- Issue 008 — NFT config + production readiness (mainnet pre-req)
- Issue 039 — Cloudflare platform improvements (P0 items done)

## Refs
- Handover 077 — Rollover deposit E2E implementation
- Handover 077 test gap analysis section
- `bethere-escrow/src/tests.rs` — existing 35 SVM tests
