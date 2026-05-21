# Handover #071: Pre-Mainnet Tooling Assessment & Integration Tests

## What Happened

Reviewed pre-mainnet tooling options (MagicBlock, DFlow) and executed the recommended action plan from the Quasar documentation audit:

1. **Tooling Assessment** — Evaluated MagicBlock (Ephemeral Rollups) and DFlow (DEX execution layer) for the BeThere escrow. Concluded neither is needed — the escrow is event-driven, not real-time/trading.

2. **CU Baseline** — Ran `quasar profile` and captured per-instruction CU metrics from all 30 unit tests.

3. **Integration Test Setup** — Created a bash-based integration test suite (`tests/integration/run.sh`) that validates the escrow program against a local `solana-test-validator`.

## Where Is the Plan/Code/Test

### CU Baseline
- **File**: `bethere-escrow/target/profile/cu-baseline-001.md`
- **Key findings**:
  - `deposit` is the most expensive instruction at 10,327 CU (5.2% of 200K limit)
  - `mark_checked_in` is cheapest at 1,237 CU
  - Full happy path totals 28,522 CU across 5 instructions
  - No CU optimization needed — all instructions are well within budget

### Integration Tests
- **File**: `tests/integration/run.sh`
- **Requires**: `solana-test-validator` running with the escrow `.so` deployed
- **Tests**: 7 checks covering RPC health, program deployment, USDC mint creation, wallet funding, PDA derivation

### Key Discovery
- `solana-test-validator` v3.1.14 does NOT finalize blocks — `Finalized Slot` stays at 0
- All RPC calls must use `{"commitment": "confirmed"}` to read state correctly
- This is a known behavior; the fix is commitment-aware RPC queries

## Reflection / Struggling / Solved

### Solved
- **Surfpool program deploy crash**: Surfpool crashes when deploying the 73.5KB `.so` via `solana program deploy`. Fell back to `solana-test-validator` which handles it fine.
- **Finalized slot issue**: Identified that test validators don't finalize blocks. Added `"commitment": "confirmed"` to all RPC calls.
- **Keypair extraction**: `solana-keygen new | head -1` gives "Generating a new keypair", not the pubkey. Fixed to `grep "^pubkey:" | awk '{print $2}'`.

### Struggled
- **Faucet vs transfer**: `requestAirdrop` RPC and `solana airdrop` don't actually credit new accounts on this validator version. Used `solana transfer --allow-unfunded-recipient` from the genesis-funded default keypair instead.

## Remain Work

### Before Mainnet (Priority Order)
1. **Kora gasless refund** — Set up Kora server so attendees don't need SOL for refunds. Requires: deploy Kora, modify TX builder to set different fee payer, manage server wallet SOL balance.
2. **Full lifecycle integration test** — Extend `tests/integration/run.sh` to test `create_event → deposit → check-in → refund → close` using `solana transfer` with memo instruction or direct program invocation.
3. **Surfpool with mainnet fork** — Once Surfpool fixes the program deploy crash, run integration tests against real USDC mint state from mainnet.
4. **`quasar profile --diff`** — After any code change, re-run and compare against the saved baseline.
5. **Devnet E2E validation** — Full lifecycle test against devnet with the new `require_distinct` checks.

### Technical Debt (From Prior Sessions)
- Fix `claim_forfeited` TX builder (missing `attendee_deposit` account)
- Unify escrow UI
- Consider Audit Arena submission

## Issues Ref

## How to Dev/Test

### Run CU profiling
```bash
cd bethere-escrow
quasar profile                    # Binary-level overhead
cargo test -- --nocapture 2>&1 | grep "CU:"  # Per-instruction CU
```

### Run integration tests
```bash
# Terminal 1: Start validator
cd event-checkin
rm -rf /tmp/bethere-test-ledger
solana-test-validator --reset \
  --rpc-port 8899 \
  --bpf-program C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T bethere-escrow/target/deploy/bethere_escrow.so \
  --ledger /tmp/bethere-test-ledger

# Terminal 2: Run tests
bash tests/integration/run.sh
```

### Compare CU regression
```bash
cd bethere-escrow
# Make code changes, rebuild
quasar build
cargo test -- --nocapture 2>&1 | grep "CU:" > /tmp/new-cu.txt
# Compare against baseline
diff target/profile/cu-baseline-001.md /tmp/new-cu.txt
```
