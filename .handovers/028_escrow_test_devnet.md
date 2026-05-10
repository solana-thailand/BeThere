# 028 — BeThere Escrow Program Tests & Devnet Deploy

## What Happened

Continued Issue 010 Phase 1 — fixed compilation errors in 16 QuasarSVM unit tests for the BeThere escrow program, got all tests passing, then deployed the program to Solana devnet.

## Branch

- `feature/010_deposit_refund_escrow` on `bethere-escrow/`

## Changes Summary

### Fix: `set_clock` → `warp_to_timestamp`
- Previous session left 7 calls to `svm.set_clock(Clock { ... })` which doesn't exist on `QuasarSvm`
- Correct API: `svm.warp_to_timestamp(timestamp)` — sets `sysvars.clock.unix_timestamp` directly
- Found by reading quasar-svm source at `~/.cargo/git/checkouts/quasar-svm-*/svm/src/svm.rs`
- Removed unused `Clock` import from test file

### Fix: unused variable warnings
- Prefixed unused `deposit_bump` with `_` in `test_deposit` (not needed — deposit account passed as `empty()`)
- Prefixed unused `escrow_bump` / `deposit_bump` in `test_full_happy_path`

### Devnet Deploy
- Updated `declare_id!` from placeholder `ADahS...` to actual keypair `2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo`
- Rebuilt with `quasar build` (57.6 KB .so)
- Deployed with `quasar deploy --url devnet`
- Program confirmed on-chain at slot 459824590

## Code/Plan Location

- Tests: `bethere-escrow/src/tests.rs` (16 tests, ~1500 lines)
- Program: `bethere-escrow/src/` (6 instructions, ~400 lines)
- Issue: `.issues/010_deposit_refund_escrow.md`

## Test Results — 16/16 Passing

| Test | Description | CU |
|------|------------|-----|
| `test_create_event` | Happy path | 3,067 |
| `test_create_event_bad_deadline` | refund_deadline ≤ event_end fails | — |
| `test_deposit` | Happy path with vault verification | 10,138 |
| `test_deposit_event_not_active` | is_active=false fails | — |
| `test_mark_checked_in` | Happy path | 1,945 |
| `test_mark_checked_in_wrong_organizer` | Unauthorized fails | — |
| `test_refund` | Happy path — checked-in refund | 8,177 |
| `test_refund_not_checked_in` | Not checked in fails | — |
| `test_refund_already_refunded` | Already refunded fails | — |
| `test_claim_forfeited` | Happy path — no-show claim | 7,164 |
| `test_claim_forfeited_before_deadline` | Before deadline fails | — |
| `test_claim_forfeited_nothing_to_claim` | Nothing to claim fails | — |
| `test_close_event` | Happy path | 5,108 |
| `test_close_event_still_active` | Still active fails | — |
| `test_full_happy_path` | 5-step chained flow | — |
| `test_no_show_path` | Multi-attendee forfeiture | 7,164 |

All CU well within 1.4M compute unit budget.

## Reflection / Struggling / Solved

- **Struggling:** `set_clock` method doesn't exist on `QuasarSvm` — the previous session guessed at the API
- **Solved:** Read the actual source in cargo git checkout — `warp_to_timestamp()` is the correct method
- **Note:** Quasar testing docs (`quasar-lang.com/docs/clients/testing`) show `with_slot()` builder but don't document `warp_to_timestamp()` — it's only discoverable from source

## Key QuasarSVM Patterns Learned

| Issue | Fix |
|-------|-----|
| Clock timestamp | `svm.warp_to_timestamp(ts)` not `set_clock()` |
| Account mutability | Generated client marks `init(idempotent)` accounts as `readonly` — patch with `with_writable()` |
| Discriminator | 1 byte (not 8 like Anchor), prepended before wincode data |
| `init(idempotent)` | Pre-create token accounts with `create_keyed_token_account()` — don't pass `empty()` |
| PDA seeds | Use wallet address (Signer), never the token account address |

## Remain Work

Phase 1 is complete. Next phases from Issue 010:

- **Phase 2 — Worker Deposit/Refund API** (~3 days):
  - Add deposit config fields to `EventConfig`
  - `GET /api/deposit/status/{attendee_id}` — check deposit status
  - `POST /api/deposit/usdc` — build Solana Pay deposit TX
  - Slip upload/verify/reject endpoints
  - Refund queue endpoints
  - KV schema for THB deposit tracking

- **Phase 3 — Frontend Deposit/Refund Flow** (~3 days)
- **Phase 4 — Integration + Devnet E2E** (~2 days)
- **Phase 5 — Mainnet** (~2 days)

## Issues Ref

- `.issues/010_deposit_refund_escrow.md`

## How to Dev/Test

```bash
cd bethere-escrow
quasar build                          # build .so + generate client
cargo test --lib -- tests --nocapture # run 16 tests with CU output
quasar deploy --url devnet            # deploy to devnet
```

## Commits

1. `5236388` — `feat: add 16 QuasarSVM unit tests for escrow program`
2. `7945a79` — `feat: update declare_id to match deployed devnet keypair`
