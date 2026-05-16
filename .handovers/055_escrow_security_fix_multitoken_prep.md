# Handover 055: Escrow Security Fix, Multi-Token Prep & Devnet Redeploy

## What Happened
Continued from session thread (Vault vs Escrow concept clarification). Three interconnected security and architecture changes to the `bethere-escrow` on-chain program:

1. **Security fix**: Per-attendee `claim_forfeited` — organizer must pass specific no-show's `AttendeeDeposit`, with `checked_in == false` guard
2. **Fairness fix**: Checked-in attendees can refund anytime after `event_end`, bypassing `refund_deadline`
3. **Multi-token prep**: Renamed `usdc_mint` → `deposit_mint`, dynamic mint decimals

## Changes Made

### Commit: `fix(escrow): per-attendee forfeit, checked-in refund bypass, generic token naming`
8 files, +356/-89 lines

| File | Changes |
|------|---------|
| `src/state.rs` | `usdc_mint` → `deposit_mint`, updated doc comments |
| `src/errors.rs` | Updated comments, added `AttendeeCheckedIn` (0x5), `RefundDeadlinePassed` (0x13) |
| `src/instructions/claim_forfeited.rs` | Per-attendee forfeit with `checked_in == false` guard, `refunded == false` guard, marks deposit as `refunded = true` |
| `src/instructions/refund.rs` | Checked-in attendees bypass `refund_deadline`; no-shows still subject to deadline |
| `src/instructions/create_event.rs` | `usdc_mint` → `deposit_mint` |
| `src/instructions/deposit.rs` | `usdc_mint` → `deposit_mint`, dynamic `deposit_mint.decimals()` |
| `src/lib.rs` | Updated all instruction doc comments |
| `src/tests.rs` | Added 2 new tests, updated 27 existing tests. Total: 29 passing |

### New Tests
- `test_claim_forfeited_checked_in_rejected` — organizer cannot forfeit a checked-in attendee's deposit (AttendeeCheckedIn error)
- `test_refund_checked_in_after_deadline` — checked-in attendee CAN refund after `refund_deadline`

### Devnet Deployment
- Program redeployed to `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`
- Data length: 67,280 → 68,320 bytes
- Slot: 462656637
- TX: `5d4eAHgMSbc4mCVKeNraAUPadcFWLLMH7JSmMdFDN3h18ARQpTztms9nzk1PqK4bxfgUnBSCiE57sH6Uyf5xXfoc`

## Key Concept: Vault vs Escrow
- **Vault** = storage container (token account). No rules.
- **Escrow** = conditional holding between opposing parties with timeout + release conditions.
- BeThere is correctly named "escrow" because: opposing interests (attendee vs organizer), conditional release (checked_in), timeout (refund_deadline), possibility of forfeiture.

## Build Workflow (Quasar)
```bash
cd bethere-escrow
quasar build          # → target/deploy/bethere_escrow.so (66.7 KB)
quasar test           # auto-builds then runs tests (29 pass)
quasar deploy --url devnet  # builds then deploys
```

**Important**: `cargo test` does NOT rebuild the `.so`. Always use `quasar test` or run `quasar build` first. The SVM loads the `.so` binary.

## Remain Work
- [ ] **E2E test on devnet** — full lifecycle: create → deposit → check-in → refund → forfeit → close with new behavior
- [ ] **On-chain CPI event indexing** — Subscribe to escrow program events via Helius websocket
- [ ] **Issue #013 Phase 5**: UX improvements (search/filter, progressive disclosure, hide on_chain_event_id)
- [ ] **Worker/frontend deploy** — if backend needs updates for `deposit_mint` field rename
- [ ] **Multi-token per event** — one escrow per token type, same `event_id`. No on-chain changes needed

## Issues Ref
- Issue #013 — Escrow Rug Pull Prevention (SEC-001, SEC-009 addressed)
- Thread: Vault vs Escrow concept difference explanation

## How to Dev/Test
```bash
cd bethere-escrow
quasar test           # 29 tests, ~9s
quasar build          # .so binary
solana program deploy target/deploy/bethere_escrow.so --url devnet
```
