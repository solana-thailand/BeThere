use super::*;

#[test]
fn test_refund() {
    let mut svm = setup();

    // Warp clock past event_end
    svm.warp_to_timestamp(EVENT_END + 1);

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    // attendee_ta (idx 4) needs writable for init(idempotent)
    let ix = with_writable(
        with_signers(
            RefundInstruction {
                attendee: ATTENDEE,
                event_escrow: escrow,
                deposit_mint: USDC_MINT,
                attendee_deposit: deposit,
                attendee_ta: ATTENDEE_TA,
                vault: VAULT,
                rent: RENT,
                token_program,
                system_program,
                _event_id: EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[4], // attendee_ta
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ATTENDEE),
            event_escrow_account(
                escrow,
                ORGANIZER,
                EVENT_ID,
                USDC_MINT,
                VAULT,
                DEPOSIT_AMOUNT,
                EVENT_END,
                REFUND_DEADLINE,
                DEPOSIT_AMOUNT, // total_deposited
                0,
                0,
                true,
                escrow_bump,
            ),
            mint_account(USDC_MINT),
            attendee_deposit_account(
                deposit,
                ATTENDEE,
                escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                true,  // checked in
                false, // not refunded yet
                deposit_bump,
            ),
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, 0), // init_if_needed — will be created
            token_account(VAULT, USDC_MINT, escrow, DEPOSIT_AMOUNT),
        ],
    );

    assert!(result.is_ok(), "refund failed: {:?}", result.raw_result);
    result.print_logs();

    // Verify vault balance decreased
    let vault_account = result.account(&VAULT).unwrap();
    let vault_token = spl_token_interface::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_token.amount, 0, "vault should be empty after refund");

    // Verify attendee received USDC
    let attendee_account = result.account(&ATTENDEE_TA).unwrap();
    let attendee_token =
        spl_token_interface::state::Account::unpack(&attendee_account.data).unwrap();
    assert_eq!(
        attendee_token.amount, DEPOSIT_AMOUNT,
        "attendee should have deposit amount"
    );

    println!("  REFUND CU: {}", result.compute_units_consumed);
}

#[test]
fn test_refund_not_checked_in() {
    // SEC-001 fix: refunds are now allowed regardless of check-in status.
    // Previously this test asserted refund fails when not checked in.
    // Now we assert refund SUCCEEDS when not checked in (after event_end).
    let mut svm = setup();
    svm.warp_to_timestamp(EVENT_END + 1);

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    let ix = with_writable(
        with_signers(
            RefundInstruction {
                attendee: ATTENDEE,
                event_escrow: escrow,
                deposit_mint: USDC_MINT,
                attendee_deposit: deposit,
                attendee_ta: ATTENDEE_TA,
                vault: VAULT,
                rent: RENT,
                token_program,
                system_program,
                _event_id: EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[4],
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ATTENDEE),
            event_escrow_account(
                escrow,
                ORGANIZER,
                EVENT_ID,
                USDC_MINT,
                VAULT,
                DEPOSIT_AMOUNT,
                EVENT_END,
                REFUND_DEADLINE,
                DEPOSIT_AMOUNT,
                0,
                0,
                true,
                escrow_bump,
            ),
            mint_account(USDC_MINT),
            attendee_deposit_account(
                deposit,
                ATTENDEE,
                escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                false, // NOT checked in — but refund should still succeed (SEC-001)
                false,
                deposit_bump,
            ),
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, 0),
            token_account(VAULT, USDC_MINT, escrow, DEPOSIT_AMOUNT),
        ],
    );

    assert!(
        result.is_ok(),
        "refund should succeed even when attendee not checked in (SEC-001 fix)"
    );
    println!("  REFUND_NOT_CHECKED_IN: correctly allowed (SEC-001 fix — refund without check-in)");
}

#[test]
fn test_refund_already_refunded() {
    let mut svm = setup();
    svm.warp_to_timestamp(EVENT_END + 1);

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    let ix = with_writable(
        with_signers(
            RefundInstruction {
                attendee: ATTENDEE,
                event_escrow: escrow,
                deposit_mint: USDC_MINT,
                attendee_deposit: deposit,
                attendee_ta: ATTENDEE_TA,
                vault: VAULT,
                rent: RENT,
                token_program,
                system_program,
                _event_id: EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[4],
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ATTENDEE),
            event_escrow_account(
                escrow,
                ORGANIZER,
                EVENT_ID,
                USDC_MINT,
                VAULT,
                DEPOSIT_AMOUNT,
                EVENT_END,
                REFUND_DEADLINE,
                DEPOSIT_AMOUNT,
                0,
                0,
                true,
                escrow_bump,
            ),
            mint_account(USDC_MINT),
            attendee_deposit_account(
                deposit,
                ATTENDEE,
                escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                true,
                true, // ALREADY refunded
                deposit_bump,
            ),
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, 0),
            token_account(VAULT, USDC_MINT, escrow, DEPOSIT_AMOUNT),
        ],
    );

    assert!(result.is_err(), "refund should fail when already refunded");
    println!("  REFUND_ALREADY_REFUNDED: correctly rejected");
}

#[test]
fn test_refund_checked_in_after_deadline() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    // Warp past refund_deadline (not just past event_end)
    svm.warp_to_timestamp(REFUND_DEADLINE + 1);

    let ix = with_writable(
        with_signers(
            RefundInstruction {
                attendee: ATTENDEE,
                event_escrow: escrow,
                deposit_mint: USDC_MINT,
                attendee_deposit: deposit,
                attendee_ta: ATTENDEE_TA,
                vault: VAULT,
                rent: RENT,
                token_program,
                system_program,
                _event_id: EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[4], // attendee_ta
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ATTENDEE),
            event_escrow_account(
                escrow,
                ORGANIZER,
                EVENT_ID,
                USDC_MINT,
                VAULT,
                DEPOSIT_AMOUNT,
                EVENT_END,
                REFUND_DEADLINE,
                DEPOSIT_AMOUNT,
                0,
                0,
                true,
                escrow_bump,
            ),
            mint_account(USDC_MINT),
            attendee_deposit_account(
                deposit,
                ATTENDEE,
                escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                true,  // CHECKED IN — should bypass refund_deadline
                false, // not refunded
                deposit_bump,
            ),
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, 0),
            token_account(VAULT, USDC_MINT, escrow, DEPOSIT_AMOUNT),
        ],
    );

    assert!(
        result.is_ok(),
        "checked-in attendee should be able to refund after refund_deadline: {:?}",
        result.raw_result
    );
    result.print_logs();

    // Verify vault is drained
    let vault_account = result.account(&VAULT).unwrap();
    let vault_token = spl_token_interface::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_token.amount, 0, "vault should be empty after refund");

    // Verify attendee received tokens
    let attendee_ta_account = result.account(&ATTENDEE_TA).unwrap();
    let attendee_token =
        spl_token_interface::state::Account::unpack(&attendee_ta_account.data).unwrap();
    assert_eq!(
        attendee_token.amount, DEPOSIT_AMOUNT,
        "checked-in attendee should receive full deposit after deadline"
    );

    println!(
        "  REFUND_CHECKED_IN_AFTER_DEADLINE CU: {}",
        result.compute_units_consumed
    );
}
