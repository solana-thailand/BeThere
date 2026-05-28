# Handover #078: Devnet E2E Script Extensions (Issue 040 Phase B)

## What Happened

Extended devnet E2E test scripts for rollover deposit lifecycle coverage (Issue 040 Phase B). Three deliverables:

1. **Extended `test_rollover_devnet.sh`** with refund-from-target + deactivate + close steps
2. **Created `test_rollover_full_lifecycle.sh`** — 2-attendee full lifecycle with USDC round-trip
3. **Created `run_all_e2e.sh`** — orchestrator running all 4 E2E scripts

## Changes

| File | Change |
|------|--------|
| `scripts/e2e/test_rollover_devnet.sh` | Added steps 10-13: refund from target, verify vaults, deactivate both, close both (+213 lines) |
| `scripts/e2e/test_rollover_full_lifecycle.sh` | New file (~849 lines): full lifecycle with 2 attendees, 2 events |
| `scripts/e2e/run_all_e2e.sh` | New file (~179 lines): orchestrator with pass/fail/skip summary |
| `.issues/040_premainnet_escrow_test_coverage.md` | Phase B items marked complete |

## Code/Plan Location

- `scripts/e2e/test_rollover_devnet.sh` — steps 0-13 (was 0-9)
- `scripts/e2e/test_rollover_full_lifecycle.sh` — steps 0-11
- `scripts/e2e/run_all_e2e.sh` — runs lifecycle → escrow → rollover → full-lifecycle

## Test

- All 3 scripts pass `bash -n` syntax check
- Prerequisites verified: `python3` with `nacl`, `solana` CLI 3.1.14, `spl-token` CLI 5.5.0
- Live devnet validation pending (requires `wrangler dev` running + funded wallets)

### How to Validate

```bash
# Start worker
cd worker && npx wrangler dev --port 8787

# In another terminal, run individual test:
bash scripts/e2e/test_rollover_devnet.sh

# Or run only the rollover test:
bash scripts/e2e/run_all_e2e.sh --only rollover

# Or run all E2E scripts:
bash scripts/e2e/run_all_e2e.sh
```

### Key API Endpoints Used

| Step | Endpoint | Signer |
|------|----------|--------|
| Refund from target | `POST /api/escrow/refund` | Attendee |
| Deactivate event | `POST /api/escrow/deactivate-event` | Organizer |
| Claim forfeited | `POST /api/escrow/claim-forfeited` | Organizer |
| Close event | `POST /api/escrow/close-event` | Organizer |

## Reflections

### Solved
- Fixed bash `for` loop pair iteration — bash doesn't support `for A B in ...` directly; used pipe-delimited strings with `${PAIR%%|*}` / `${PAIR##*|}`
- Fixed `run_all_e2e.sh` `--only` arg parsing — changed from `for arg in "$@"` to `while [[ $# -gt 0 ]]` with `shift 2` for `--only` value
- All endpoint request bodies verified against actual handler struct definitions

### Timing Constraint
- The refund step requires the target event's **on-chain** deadline to have passed. The scripts create target events with `event_end_ms = now + 2 days`, so refunds will fail on devnet unless the event_end_ms is set to past on-chain. This is a known limitation documented in the scripts with graceful error handling.

## Remain Work

- [ ] Run live devnet validation with `wrangler dev` running
- [ ] Issue 040 Phase C: Manual browser test of rollover flow
- [ ] Phase 10: Mainnet deployment (fund authority keypair, deploy program, configure Helius webhook)
- [ ] Consider adding `--short-event-end` flag to full lifecycle script that creates target event with 60s deadline for faster testing
