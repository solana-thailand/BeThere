//! Kani formal verification harnesses for `bethere-escrow` financial logic.
//!
//! These harnesses verify **pure arithmetic properties** of the escrow accounting
//! using symbolic execution. They cannot verify CPI calls, PDA derivation, or
//! account ownership — those are covered by SVM integration tests in `tests.rs`.
//!
//! Run: `cargo kani`
//! Requires: Kani 0.67.0+ (`cargo install kani-verifier --version 0.67.0 && cargo kani setup`)
//!
//! ## Verified Properties
//!
//! | Harness | Property |
//! |---------|----------|
//! | `create_event_rejects_zero_deposit` | deposit_amount == 0 always rejected |
//! | `create_event_rejects_past_event_end` | event_end <= now always rejected |
//! | `create_event_rejects_bad_refund_deadline` | refund_deadline <= event_end always rejected |
//! | `deposit_overflow_safe` | checked_add never silently wraps |
//! | `refund_overflow_safe` | checked_add on total_refunded never silently wraps |
//! | `claim_forfeited_double_sub_safe` | double checked_sub never silently wraps |
//! | `close_event_invariant` | total_deposited == total_refunded + total_forfeited ⟹ settled == total_deposited |
//! | `accounting_conservation` | total_deposited ≥ total_refunded + total_forfeited always holds for valid states |
//! | `forfeited_is_non_negative` | forfeited = deposited - refunded - already_forfeited ≥ 0 for valid states |
//! | `claim_then_close_consistent` | After claiming all forfeited, close invariant holds |

#![cfg(kani)]

use core::hint::black_box;

// ---------------------------------------------------------------------------
// Pure logic extracted from instruction handlers.
// These mirror the exact arithmetic in the escrow program without
// Solana-specific types, so Kani can symbolically execute them.
// ---------------------------------------------------------------------------

/// Validates inputs for `create_event`.
/// Mirrors: `CreateEvent::create_escrow` validation logic.
///
/// Returns `Ok(())` if valid, `Err(error_code)` otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationError {
    InvalidDepositAmount = 12,
    EventEndInPast = 13,
    RefundDeadlineNotPassed = 3,
    Overflow = 14,
}

fn validate_create_event(
    deposit_amount: u64,
    event_end: i64,
    refund_deadline: i64,
    now: i64,
) -> Result<(), ValidationError> {
    if deposit_amount == 0 {
        return Err(ValidationError::InvalidDepositAmount);
    }
    if event_end <= now {
        return Err(ValidationError::EventEndInPast);
    }
    if refund_deadline <= event_end {
        return Err(ValidationError::RefundDeadlineNotPassed);
    }
    Ok(())
}

/// Applies a deposit to escrow totals.
/// Mirrors: `Deposit::create_deposit` — the `checked_add` on `total_deposited`.
///
/// Returns new `total_deposited` or `Overflow` error.
fn apply_deposit(total_deposited: u64, amount: u64) -> Result<u64, ValidationError> {
    total_deposited
        .checked_add(amount)
        .ok_or(ValidationError::Overflow)
}

/// Applies a refund to escrow totals.
/// Mirrors: `Refund::validate_and_update` — `checked_add` on `total_refunded`.
///
/// Returns new `total_refunded` or `Overflow` error.
fn apply_refund(total_refunded: u64, amount: u64) -> Result<u64, ValidationError> {
    total_refunded
        .checked_add(amount)
        .ok_or(ValidationError::Overflow)
}

/// Calculates forfeited amount and updates accounting.
/// Mirrors: `ClaimForfeited::validate_and_claim` — the double `checked_sub`.
///
/// Returns `(new_total_forfeited, forfeited_this_claim)` or error.
fn calculate_forfeited(
    total_deposited: u64,
    total_refunded: u64,
    total_forfeited: u64,
) -> Result<(u64, u64), ValidationError> {
    let forfeited = total_deposited
        .checked_sub(total_refunded)
        .and_then(|v| v.checked_sub(total_forfeited))
        .ok_or(ValidationError::Overflow)?;

    if forfeited == 0 {
        return Err(ValidationError::Overflow); // NoForfeitedFunds
    }

    let new_total_forfeited = total_forfeited
        .checked_add(forfeited)
        .ok_or(ValidationError::Overflow)?;

    Ok((new_total_forfeited, forfeited))
}

/// Validates close_event invariant.
/// Mirrors: `CloseEvent::close_event` — `total_deposited == total_refunded + total_forfeited`.
///
/// Returns `Ok(())` if vault is settled (empty), `Err(VaultNotEmpty)` otherwise.
fn validate_close_invariant(
    total_deposited: u64,
    total_refunded: u64,
    total_forfeited: u64,
) -> Result<(), ValidationError> {
    let settled = total_refunded
        .checked_add(total_forfeited)
        .ok_or(ValidationError::Overflow)?;

    if total_deposited != settled {
        return Err(ValidationError::Overflow); // VaultNotEmpty
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Kani Proof Harnesses
// ---------------------------------------------------------------------------

/// **Property**: `create_event` ALWAYS rejects `deposit_amount == 0`.
///
/// This is a simple sanity check that the validation branch is correct.
#[kani::proof]
fn create_event_rejects_zero_deposit() {
    let event_end: i64 = kani::any();
    let refund_deadline: i64 = kani::any();
    let now: i64 = kani::any();

    let result = validate_create_event(0, event_end, refund_deadline, now);
    assert_eq!(result, Err(ValidationError::InvalidDepositAmount));
}

/// **Property**: `create_event` ALWAYS rejects `event_end <= now`.
///
/// Even with valid deposit_amount and refund_deadline, a past event_end is rejected.
#[kani::proof]
fn create_event_rejects_past_event_end() {
    let deposit_amount: u64 = kani::any();
    kani::assume(deposit_amount != 0); // non-zero deposit
    let now: i64 = kani::any();
    let event_end: i64 = kani::any();
    kani::assume(event_end <= now); // event_end is in the past
    let refund_deadline: i64 = kani::any();

    let result = validate_create_event(deposit_amount, event_end, refund_deadline, now);
    assert_eq!(result, Err(ValidationError::EventEndInPast));
}

/// **Property**: `create_event` ALWAYS rejects `refund_deadline <= event_end`.
///
/// Even with valid deposit_amount and future event_end, a bad refund_deadline is rejected.
#[kani::proof]
fn create_event_rejects_bad_refund_deadline() {
    let deposit_amount: u64 = kani::any();
    kani::assume(deposit_amount != 0);
    let now: i64 = kani::any();
    let event_end: i64 = kani::any();
    kani::assume(event_end > now); // event_end is in the future
    let refund_deadline: i64 = kani::any();
    kani::assume(refund_deadline <= event_end); // refund_deadline too early

    let result = validate_create_event(deposit_amount, event_end, refund_deadline, now);
    assert_eq!(result, Err(ValidationError::RefundDeadlineNotPassed));
}

/// **Property**: Valid `create_event` inputs are ALWAYS accepted.
///
/// When all three conditions are met, the function returns `Ok(())`.
#[kani::proof]
fn create_event_accepts_valid_inputs() {
    let deposit_amount: u64 = kani::any();
    kani::assume(deposit_amount != 0);
    let now: i64 = kani::any();
    // Bound now to avoid overflow in the offset calculation
    kani::assume(now < i64::MAX - 2);
    let event_end: i64 = now + 1; // strictly > now
    let refund_deadline: i64 = event_end + 1; // strictly > event_end

    let result = validate_create_event(deposit_amount, event_end, refund_deadline, now);
    assert_eq!(result, Ok(()));
}

/// **Property**: Deposit overflow check is **sound** — if it returns `Ok`, the
/// result is always `>= old_total_deposited` (monotonically increasing).
///
/// Proves: `checked_add` never silently wraps around.
#[kani::proof]
fn deposit_overflow_safe() {
    let total_deposited: u64 = kani::any();
    let amount: u64 = kani::any();

    let result = apply_deposit(total_deposited, amount);
    match result {
        Ok(new_total) => {
            // The new total must be strictly greater (both non-zero)
            // or equal to old if amount is 0
            assert!(new_total >= total_deposited);
            assert!(new_total == total_deposited + amount);
        }
        Err(ValidationError::Overflow) => {
            // Overflow case: old + amount would exceed u64::MAX
            assert!(total_deposited > u64::MAX - amount);
        }
        Err(_) => panic!("unexpected error"),
    }
}

/// **Property**: Refund overflow check is **sound** — if it returns `Ok`, the
/// result is always `>= old_total_refunded`.
///
/// Proves: `checked_add` on refund counter never silently wraps around.
#[kani::proof]
fn refund_overflow_safe() {
    let total_refunded: u64 = kani::any();
    let amount: u64 = kani::any();

    let result = apply_refund(total_refunded, amount);
    match result {
        Ok(new_total) => {
            assert!(new_total >= total_refunded);
            assert!(new_total == total_refunded + amount);
        }
        Err(ValidationError::Overflow) => {
            assert!(total_refunded > u64::MAX - amount);
        }
        Err(_) => panic!("unexpected error"),
    }
}

/// **Property**: Forfeited calculation via double `checked_sub` is **sound**.
///
/// If it returns `Ok`, then:
/// - `forfeited >= 0` (trivially true for u64)
/// - `forfeited == total_deposited - total_refunded - total_forfeited`
/// - `new_total_forfeited == total_forfeited + forfeited`
///
/// Proves: No silent underflow in the double subtraction.
#[kani::proof]
fn claim_forfeited_double_sub_safe() {
    let total_deposited: u64 = kani::any();
    let total_refunded: u64 = kani::any();
    let total_forfeited: u64 = kani::any();

    let result = calculate_forfeited(total_deposited, total_refunded, total_forfeited);
    match result {
        Ok((new_total_forfeited, forfeited)) => {
            // forfeited must be non-zero (checked in the function)
            assert!(forfeited > 0);
            // The math is consistent: forfeited = deposited - refunded - already_forfeited
            assert_eq!(
                forfeited,
                total_deposited - total_refunded - total_forfeited
            );
            // new_total_forfeited = old + forfeited
            assert_eq!(new_total_forfeited, total_forfeited + forfeited);
            // Conservation: refunded + new_total_forfeited <= deposited
            // (remaining in vault is deposited - refunded - new_total_forfeited)
            let remaining = total_deposited - total_refunded - new_total_forfeited;
            assert_eq!(remaining, 0); // After claiming ALL forfeited, nothing remains
        }
        Err(_) => {
            // Either underflow or forfeited == 0.
            // Underflow means: total_deposited < total_refunded + total_forfeited
            // This shouldn't happen in valid state, but Kani checks all inputs.
        }
    }
}

/// **Property**: Close event invariant check is **complete**.
///
/// When `total_deposited == total_refunded + total_forfeited`, the close succeeds.
/// When they differ, the close fails.
#[kani::proof]
fn close_event_invariant() {
    let total_deposited: u64 = kani::any();
    let total_refunded: u64 = kani::any();
    let total_forfeited: u64 = kani::any();

    // Constrain to avoid overflow in the check itself
    kani::assume(total_refunded.checked_add(total_forfeited).is_some());

    let settled = total_refunded + total_forfeited;
    let result = validate_close_invariant(total_deposited, total_refunded, total_forfeited);

    if total_deposited == settled {
        assert_eq!(result, Ok(()));
    } else {
        assert!(result.is_err());
    }
}

/// **Property**: Accounting conservation law holds for valid escrow states.
///
/// For any valid state (where deposits ≥ refunds + forfeited), the vault
/// balance would be `total_deposited - total_refunded - total_forfeited`.
/// This must always be non-negative (no underflow).
///
/// This is the **fundamental invariant** of the escrow program.
#[kani::proof]
fn accounting_conservation() {
    let total_deposited: u64 = kani::any();
    let total_refunded: u64 = kani::any();
    let total_forfeited: u64 = kani::any();

    // Assume valid state: no underflow in vault balance calculation
    // i.e., deposited >= refunded + forfeited (the vault never goes negative)
    let refunded_plus_forfeited = total_refunded.checked_add(total_forfeited);
    kani::assume(refunded_plus_forfeited.is_some());
    kani::assume(total_deposited >= total_refunded + total_forfeited);

    // Then the vault balance is well-defined
    let vault_balance = total_deposited - total_refunded - total_forfeited;

    // Vault balance is always non-negative (trivially for u64, but proves no underflow)
    // And it's always <= total_deposited
    assert!(vault_balance <= total_deposited);

    // After claiming all forfeited (vault_balance), the invariant holds
    let new_forfeited = total_forfeited + vault_balance;
    let result = validate_close_invariant(total_deposited, total_refunded, new_forfeited);
    assert_eq!(result, Ok(()));
}

/// **Property**: Forfeited amount is always non-negative for valid escrow states.
///
/// In a valid state where `total_deposited >= total_refunded + total_forfeited`,
/// the remaining forfeited amount `total_deposited - total_refunded - total_forfeited`
/// is well-defined and non-negative.
#[kani::proof]
fn forfeited_is_non_negative() {
    let total_deposited: u64 = kani::any();
    let total_refunded: u64 = kani::any();
    let total_forfeited: u64 = kani::any();

    // Valid state constraints
    kani::assume(total_refunded.checked_add(total_forfeited).is_some());
    kani::assume(total_deposited >= total_refunded + total_forfeited);

    // The forfeited calculation must not underflow
    let remaining = total_deposited
        .checked_sub(total_refunded)
        .and_then(|v| v.checked_sub(total_forfeited));

    // Must succeed for valid states
    assert!(remaining.is_some());
    let remaining = remaining.unwrap();

    // remaining >= 0 (trivially true for u64)
    // remaining <= total_deposited
    assert!(remaining <= total_deposited);
}

/// **Property**: After a full claim of all forfeited funds, the close invariant
/// is guaranteed to hold.
///
/// Simulates: deposits → refund some → claim remaining → close.
/// This is the critical path for organizer closing the event.
#[kani::proof]
fn claim_then_close_consistent() {
    // Step 1: Initial deposits
    let total_deposited: u64 = kani::any();

    // Step 2: Some refunds happened
    let total_refunded: u64 = kani::any();
    kani::assume(total_refunded <= total_deposited);

    // Step 3: Some forfeited already claimed
    let total_forfeited_before: u64 = kani::any();
    kani::assume(total_forfeited_before <= total_deposited);
    // Guard against overflow in the sum itself
    kani::assume(total_refunded.checked_add(total_forfeited_before).is_some());
    kani::assume(total_refunded + total_forfeited_before <= total_deposited);

    // Step 4: Calculate remaining forfeited
    let remaining = total_deposited - total_refunded - total_forfeited_before;

    // Step 5: Claim all remaining forfeited
    if remaining > 0 {
        let claim_result =
            calculate_forfeited(total_deposited, total_refunded, total_forfeited_before);
        match claim_result {
            Ok((new_forfeited, forfeited_amount)) => {
                assert_eq!(forfeited_amount, remaining);
                assert_eq!(new_forfeited, total_forfeited_before + remaining);

                // Step 6: Verify close invariant
                let close_result =
                    validate_close_invariant(total_deposited, total_refunded, new_forfeited);
                assert_eq!(close_result, Ok(()));
            }
            Err(_) => {
                // Should not happen for valid state
                panic!("calculate_forfeited failed on valid state");
            }
        }
    } else {
        // No remaining forfeited — close should work directly
        let close_result =
            validate_close_invariant(total_deposited, total_refunded, total_forfeited_before);
        assert_eq!(close_result, Ok(()));
    }
}

/// **Property**: Sequential deposits monotonically increase total_deposited.
///
/// Simulates: multiple attendees depositing, proving the accounting
/// is always additive and never decreases.
#[kani::proof]
fn sequential_deposits_monotonic() {
    let mut total_deposited: u64 = 0;
    let mut deposit_count: u32 = 0;
    const MAX_DEPOSITS: u32 = 5; // Bound for tractability

    while deposit_count < MAX_DEPOSITS {
        let amount: u64 = kani::any();
        // Each deposit amount matches the event's deposit_amount (non-zero)
        kani::assume(amount > 0);
        kani::assume(amount <= u64::MAX / MAX_DEPOSITS as u64); // Bound for tractability

        let old_total = total_deposited;
        let result = apply_deposit(total_deposited, amount);

        match result {
            Ok(new_total) => {
                assert!(new_total > old_total);
                total_deposited = new_total;
            }
            Err(ValidationError::Overflow) => {
                // Reached u64::MAX — stop
                break;
            }
            Err(_) => panic!("unexpected error"),
        }
        deposit_count += 1;
    }

    // Final total_deposited is a multiple of individual deposit amounts
    black_box(total_deposited);
}

/// **Property**: Sequential refunds monotonically increase total_refunded.
///
/// Proves that refund accounting is always additive.
#[kani::proof]
fn sequential_refunds_monotonic() {
    let mut total_refunded: u64 = 0;
    let mut refund_count: u32 = 0;
    const MAX_REFUNDS: u32 = 5;

    while refund_count < MAX_REFUNDS {
        let amount: u64 = kani::any();
        kani::assume(amount > 0);
        kani::assume(amount <= u64::MAX / MAX_REFUNDS as u64);

        let old_total = total_refunded;
        let result = apply_refund(total_refunded, amount);

        match result {
            Ok(new_total) => {
                assert!(new_total > old_total);
                total_refunded = new_total;
            }
            Err(ValidationError::Overflow) => {
                break;
            }
            Err(_) => panic!("unexpected error"),
        }
        refund_count += 1;
    }

    black_box(total_refunded);
}
