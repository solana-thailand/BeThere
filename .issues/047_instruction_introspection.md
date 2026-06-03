# 047: Instruction Introspection for Escrow Program

## Summary

Add Instructions sysvar (`Sysvar1nstructions1111111111111111111111111`) to the on-chain escrow program so instructions can inspect sibling instructions in the same transaction and enforce structural invariants.

## Motivation

Currently each escrow instruction validates in isolation. It cannot see what other instructions are in the same transaction. This creates gaps:

- **SEC-010 rent leak**: a bare `refund` without `close_deposit` leaves the AttendeeDeposit PDA alive. Currently enforced client-side only (Worker builds `refund_and_close` combined TX). A program-level enforcement would be a stronger guarantee.
- **Multi-deposit sandwich**: nothing prevents multiple `deposit` instructions for the same event in one TX.
- **CPI attack surface**: other programs can CPI into the escrow, bypassing frontend validation.
- **Walk-in flow**: no way to atomically deposit + check-in in a single TX because neither instruction can verify the other is present.

## What Instruction Introspection Is

- A transaction is an ordered list of instructions.
- Each instruction has: program ID, account metas, data bytes.
- The Instructions sysvar is a read-only account exposing the current transaction's instruction list.
- Programs load instructions by index and know which one is currently executing (last 2 bytes of sysvar data).
- Only same-transaction top-level instructions are visible (no CPI inner instructions, no transaction history).

## Plan

### 1. Refund + Close Enforcement (P0)

Add Instructions sysvar to the `refund` instruction handler. After validating refund eligibility, scan remaining instructions in the TX for a `close_deposit` (discriminator 7) targeting the same attendee. Reject if not found.

**Why P0**: Fixes SEC-010 at the program level instead of relying on client-side TX building.

### 2. Multi-Deposit Prevention (P1)

Add Instructions sysvar to the `deposit` instruction handler. Scan all instructions in the TX. If another `deposit` (discriminator 1) for the same event escrow exists, reject.

### 3. CPI Detection (P2)

Use `get_stack_height()` syscall. If stack height > 1 (CPI context), either reject or apply stricter validation. This prevents external programs from composing with escrow instructions in unexpected ways.

### 4. Atomic Deposit + Check-In (P3)

Enable walk-in attendees to deposit and get checked in within a single TX. The `deposit` handler checks that a `mark_checked_in` (discriminator 2) for the same attendee is also present in the transaction. Requires the organizer to co-sign (both attendee and organizer are signers in the combined TX).

## Code Changes

All changes are in the **on-chain program** (`bethere-escrow/`).

### Per-instruction changes

Each instruction that needs introspection:

1. Add `instruction_sysvar: AccountInfo` to its accounts struct
2. Validate sysvar address: `Sysvar1nstructions1111111111111111111111111`
3. Load current index and scan sibling instructions
4. Enforce constraint (reject if violated)

### Example (refund + close enforcement)

```rust
// In refund instruction handler:
let current_index = load_current_index_checked(&instruction_sysvar)?;
let num_instructions = /* read from sysvar header */;
let has_close = (current_index..num_instructions).any(|i| {
    let ix = load_instruction_at_checked(i, &instruction_sysvar)?;
    ix.program_id == ESCROW_PROGRAM_ID 
        && ix.data.first() == Some(&7) // close_deposit discriminator
        && ix.accounts contains attendee_deposit
});
require!(has_close, EscrowError::RefundRequiresClose);
```

### No client-side changes needed

The Worker TX builders already build correct multi-instruction transactions (e.g., `refund_and_close`). Introspection adds program-level enforcement that these pairings always happen.

## Tests

- Unit tests via quasar-svm: build multi-instruction transactions, verify reject/accept behavior
- Test cases per use case:
  - `refund` alone → reject
  - `refund` + `close_deposit` → accept
  - Two `deposit` for same event in one TX → reject
  - Single `deposit` → accept
  - CPI invocation at stack height > 1 → reject (if P2 implemented)
  - `deposit` + `mark_checked_in` in one TX → accept (if P4 implemented)

## References

- `docs/escrow_protocol.md` §8 — Instruction Introspection section
- `docs/security_audit.md` — Vulnerability Category Mapping + Program-Side Checklist (📋 Planned rows)
- Solana docs: https://solana.com/docs/core/instructions/instruction-introspection
- Source: https://github.com/anza-xyz/solana-sdk/blob/HEAD/instructions-sysvar/src/lib.rs
- Example (Jupiter DAMM v2): https://github.com/jup-ag/damm-v2/blob/HEAD/programs/cp-amm/src/instructions/swap/ix_p_swap.rs
- Example (Kamino Scope): https://github.com/Kamino-Finance/scope/blob/HEAD/programs/scope/src/handlers/handler_refresh_prices.rs

## Status

- [x] Research and documentation
- [x] P0: Refund + close enforcement — **COMPLETE**. Full inline scanning active. `refund` must be paired with `close_deposit` in the same transaction or the instruction is rejected.
- [ ] P1: Multi-deposit prevention
- [ ] P2: CPI detection
- [ ] P3: Atomic deposit + check-in

### BPF Call Depth Constraint

The refund instruction was already at the 64-frame BPF call depth limit. Adding the `instruction_sysvar: UncheckedAccount` account (which adds 1+ frames from quasar's account deserialization) plus the introspection scanning logic exceeded the limit, causing `exceeded max BPF to BPF call depth`.

**Workaround**: The handler validates the sysvar address only. The Worker already builds `refund+close_deposit` atomically, so the sysvar presence requirement acts as a structural constraint. Full instruction scanning (matching `close_deposit` discriminator in sibling instructions) is implemented in `introspection.rs` but disabled due to call depth.

**Resolution path**: Optimize BPF call depth by:
1. Removing `require_distinct` calls (PDA seeds guarantee distinct addresses)
2. Removing `validate_version` calls (PDA seeds guarantee account type)
3. Removing `emit_event` calls from the handler
4. Upgrading quasar-lang for more efficient account deserialization

### Session 3 Findings (2026-06-03)

**Root cause of scanning failure found and fixed:**
- The Instructions sysvar address was **WRONG** — the hardcoded bytes `[6, 167, 213, 23, 24, 53, 110, ...]` encoded `SysvaqrH7Quhid8SvXtYVGsxdv8pWAXdQmEHE37yVGB` instead of the canonical `Sysvar1nstructions1111111111111111111111111` (`[6, 167, 213, 23, 24, 123, 209, 102, ...]`)
- This caused the SVM to map the wrong address to an empty default account while the correctly-built sysvar data sat unused in fallbacks
- Additionally fixed a quasar-svm bug: `compile_accounts` didn't populate the Instructions sysvar data when the sysvar was in the message's account_keys but not in provided accounts

**Full instruction scanning now ACTIVE:**
- Inline scanning in `require_close_deposit_pair()` fits within BPF call depth (87.8 KB .so)
- Scans sibling instructions for `close_deposit` (discriminator 7) from the same program targeting the same attendee deposit
- Standalone refund without close_deposit is rejected with `RefundRequiresClose`
- Refund + close_deposit chain works correctly

**All 43 tests passing.**

### Files Modified

| File | Change |
|------|--------|
| `bethere-escrow/src/instructions/refund.rs` | Fixed sysvar ID bytes. Enabled full inline scanning. Removed `require_distinct` + `validate_version`. |
| `bethere-escrow/src/instructions/introspection.rs` | Fixed sysvar ID bytes. Scanning module intact and ready for P1/P2. |
| `bethere-escrow/src/tests/introspection.rs` | 4 tests: sysvar account required, refund+close chain, wrong sysvar rejected, standalone refund rejected. |
| `bethere-escrow/src/tests/mod.rs` | Fixed `INSTRUCTIONS_SYSVAR` constant to use canonical address. |
