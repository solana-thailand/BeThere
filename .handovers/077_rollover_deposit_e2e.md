# 077 — Rollover Deposit E2E Implementation

## What Happened

Implemented the full `rollover_deposit` instruction across all three layers (on-chain, worker, frontend) per Issue 032 Architecture Decision (Option B: On-chain Rollover Instruction). This enables a checked-in attendee to atomically move their USDC deposit from a past event's vault to a new event's vault without any off-chain intermediary.

## Plan / Code / Test

### On-chain (`bethere-escrow/`)
- **New instruction**: `rollover_deposit` (discriminator 8) in `src/instructions/rollover_deposit.rs` (173 lines)
- **Event**: `DepositRolledOver` with `source_escrow`, `target_escrow`, `attendee`, `amount`
- **Guards**: checked-in, not-refunded, target-active, same-organizer, same-deposit-amount
- **Tests**: 5 new SVM tests (35 total pass):
  - `test_rollover_deposit_happy_path`
  - `test_rollover_deposit_not_checked_in`
  - `test_rollover_deposit_already_refunded`
  - `test_rollover_deposit_target_not_active`
  - `test_rollover_deposit_different_organizer`

### Worker API (`worker/`)
- `build_rollover_deposit_transaction()` in `tx_builders.rs` — dual `EscrowCtx` resolution
- `rollover_deposit_tx_handler` — `POST /api/escrow/rollover-deposit` (attendee-authed)
- `EscrowRolloverInitiated` audit action
- `find_rollover_target()` helper in `attendee.rs` — finds eligible target events
- `rollover_target_event` + `event_id` added to public ticket response
- `RolloverDeposit` variant in `escrow_indexer.rs` (discriminator 8)

### Frontend (`frontend-leptos/`)
- `RolloverTargetEvent` type, `event_id` on `AttendeeData`
- `event_id` + `rollover_target_event` on `TicketViewData`
- Self-contained `RolloverActionCard` with wallet signing flow:
  - State machine: Ready → ChooseWallet → WalletConnected → Signing → Confirmed/Error
  - Uses existing wallet helpers: connect, sign+send, cluster check, pre-sign simulation
  - Shows Solscan link on success, retry on error

### Build Verification
- `bethere-escrow`: `quasar test` → 35/35 pass
- `worker`: `cargo check` → clean
- `frontend-leptos`: diagnostics clean

## Reflection / Struggles / Solved

- **Git namespace conflict**: `develop` branch blocked `develop/feature/32_rollover_deposit` — had to delete remote `develop` first
- **Missing event_id**: `AttendeeData` didn't include source `event_id` — added to both worker JSON response and frontend types
- **Wallet flow on ticket page**: Ticket page is simple (no wallet infra). Solved by making `RolloverActionCard` self-contained with its own wallet detection, connection, and signing — reusing `escrow_init` module's wallet helpers

## Remain Work

### Devnet Testing
- [ ] Redeploy escrow program to devnet with new instruction (discriminator 8)
- [ ] E2E test: deposit → check-in → event ends → rollover card appears → sign TX → verify new deposit on target event
- [ ] Verify indexer picks up `DepositRolledOver` events

### Mainnet Deployment
- [ ] Obtain mainnet authority keypair with SOL (~3-5 SOL)
- [ ] Build + deploy updated escrow program
- [ ] Set up Helius mainnet webhook for discriminator 8
- [ ] Add `rollover_deposit` parser to production indexer

### Post-Deployment
- [ ] Complete D1 migration of attendee state (Issue 037 Phase 2)
- [ ] Evaluate eliminating `AttendeeDeposit` PDAs (move all state to D1)
- [ ] Consider Option C (Org-Level Vault) when 3+ organizers active

## Issues Ref
- Issue 032 — Rolling Deposit / Credit
- Issue 010 — Escrow operations (architecture decision)

## How to Dev/Test

### On-chain
```bash
cd bethere-escrow
quasar build   # builds program
quasar test    # runs 35 SVM tests
```

### Worker
```bash
cd worker
cargo check    # verify compilation
wrangler dev   # local dev (needs .dev.vars)
```

### Frontend
```bash
cd frontend-leptos
cargo check --target wasm32-unknown-unknown
trunk serve    # local dev
```

### E2E on devnet
1. Create two events with same organizer + same `deposit_amount_usdc`
2. Attendee deposits USDC on event A → gets checked in
3. Event A ends (or mock `event_end_ms` in past)
4. Attendee opens ticket page → `RolloverActionCard` appears with target event B
5. Click "Roll to Next Event" → connect wallet → sign TX
6. Verify deposit appears on event B via admin escrow panel
