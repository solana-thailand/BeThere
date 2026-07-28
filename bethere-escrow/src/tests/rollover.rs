use super::*;

#[test]
fn test_rollover_deposit_happy_path() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;

    // Source event (past — attendee checked in)
    let (source_escrow, source_escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (source_deposit, source_deposit_bump) = find_attendee_deposit(&source_escrow, &ATTENDEE);

    // Target event (new — active, accepting deposits)
    let (target_escrow, target_escrow_bump) = find_event_escrow(&ORGANIZER, TARGET_EVENT_ID);
    let (target_deposit, _target_deposit_bump) = find_attendee_deposit(&target_escrow, &ATTENDEE);

    let ix = with_writable(
        with_signers(
            RolloverDepositInstruction {
                attendee: ATTENDEE,
                source_escrow,
                source_deposit,
                source_vault: VAULT,
                target_escrow,
                target_deposit,
                target_vault: TARGET_VAULT,
                deposit_mint: USDC_MINT,
                rent: RENT,
                token_program,
                system_program,
                _source_event_id: EVENT_ID,
                _target_event_id: TARGET_EVENT_ID,
            }
            .into(),
            &[0], // attendee is signer
        ),
        &[5], // target_deposit (init), target_vault (mut)
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ATTENDEE),
            // Source escrow: 1 deposit, 0 refunded, 0 forfeited
            event_escrow_account(
                source_escrow,
                ORGANIZER,
                EVENT_ID,
                USDC_MINT,
                VAULT,
                DEPOSIT_AMOUNT,
                EVENT_END,
                REFUND_DEADLINE,
                DEPOSIT_AMOUNT, // total_deposited
                0,              // total_refunded
                0,              // total_forfeited
                false,          // is_active (past event)
                source_escrow_bump,
            ),
            // Source deposit: checked in, not refunded
            attendee_deposit_account(
                source_deposit,
                ATTENDEE,
                source_escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                true,  // checked in
                false, // not refunded
                source_deposit_bump,
            ),
            // Source vault holds the deposit
            token_account(VAULT, USDC_MINT, source_escrow, DEPOSIT_AMOUNT),
            // Target escrow: active, 0 deposits so far
            event_escrow_account(
                target_escrow,
                ORGANIZER,
                TARGET_EVENT_ID,
                USDC_MINT,
                TARGET_VAULT,
                DEPOSIT_AMOUNT,
                EVENT_END + 86_400 * 30, // far future
                REFUND_DEADLINE + 86_400 * 30,
                0,    // total_deposited
                0,    // total_refunded
                0,    // total_forfeited
                true, // is_active
                target_escrow_bump,
            ),
            // Target deposit: empty (init)
            empty(target_deposit),
            // Target vault: empty (will receive USDC)
            token_account(TARGET_VAULT, USDC_MINT, target_escrow, 0),
            mint_account(USDC_MINT),
        ],
    );

    assert!(
        result.is_ok(),
        "rollover_deposit failed: {:?}",
        result.raw_result
    );
    result.print_logs();

    // Verify source vault is drained
    let source_vault_acct = result.account(&VAULT).unwrap();
    let source_vault_token =
        spl_token_interface::state::Account::unpack(&source_vault_acct.data).unwrap();
    assert_eq!(
        source_vault_token.amount, 0,
        "source vault should be empty after rollover"
    );

    // Verify target vault received the USDC
    let target_vault_acct = result.account(&TARGET_VAULT).unwrap();
    let target_vault_token =
        spl_token_interface::state::Account::unpack(&target_vault_acct.data).unwrap();
    assert_eq!(
        target_vault_token.amount, DEPOSIT_AMOUNT,
        "target vault should receive the deposit amount"
    );

    println!("  ROLLOVER_DEPOSIT CU: {}", result.compute_units_consumed);
}

#[test]
fn test_rollover_deposit_not_checked_in() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;

    let (source_escrow, source_escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (source_deposit, source_deposit_bump) = find_attendee_deposit(&source_escrow, &ATTENDEE);
    let (target_escrow, target_escrow_bump) = find_event_escrow(&ORGANIZER, TARGET_EVENT_ID);
    let (target_deposit, _) = find_attendee_deposit(&target_escrow, &ATTENDEE);

    let ix = with_writable(
        with_signers(
            RolloverDepositInstruction {
                attendee: ATTENDEE,
                source_escrow,
                source_deposit,
                source_vault: VAULT,
                target_escrow,
                target_deposit,
                target_vault: TARGET_VAULT,
                deposit_mint: USDC_MINT,
                rent: RENT,
                token_program,
                system_program,
                _source_event_id: EVENT_ID,
                _target_event_id: TARGET_EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[5],
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ATTENDEE),
            event_escrow_account(
                source_escrow,
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
                false,
                source_escrow_bump,
            ),
            // NOT checked in — should fail
            attendee_deposit_account(
                source_deposit,
                ATTENDEE,
                source_escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                false, // not checked in
                false,
                source_deposit_bump,
            ),
            token_account(VAULT, USDC_MINT, source_escrow, DEPOSIT_AMOUNT),
            event_escrow_account(
                target_escrow,
                ORGANIZER,
                TARGET_EVENT_ID,
                USDC_MINT,
                TARGET_VAULT,
                DEPOSIT_AMOUNT,
                EVENT_END + 86_400 * 30,
                REFUND_DEADLINE + 86_400 * 30,
                0,
                0,
                0,
                true,
                target_escrow_bump,
            ),
            empty(target_deposit),
            token_account(TARGET_VAULT, USDC_MINT, target_escrow, 0),
            mint_account(USDC_MINT),
        ],
    );

    assert!(
        result.is_err(),
        "rollover should fail when attendee not checked in"
    );
    // NotCheckedIn = 2
    let err_code = result.raw_result.unwrap_err();
    assert_eq!(
        err_code,
        quasar_svm::InstructionError::Custom(2),
        "expected NotCheckedIn (2), got {err_code:?}"
    );
}

#[test]
fn test_rollover_deposit_already_refunded() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;

    let (source_escrow, source_escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (source_deposit, source_deposit_bump) = find_attendee_deposit(&source_escrow, &ATTENDEE);
    let (target_escrow, target_escrow_bump) = find_event_escrow(&ORGANIZER, TARGET_EVENT_ID);
    let (target_deposit, _) = find_attendee_deposit(&target_escrow, &ATTENDEE);

    let ix = with_writable(
        with_signers(
            RolloverDepositInstruction {
                attendee: ATTENDEE,
                source_escrow,
                source_deposit,
                source_vault: VAULT,
                target_escrow,
                target_deposit,
                target_vault: TARGET_VAULT,
                deposit_mint: USDC_MINT,
                rent: RENT,
                token_program,
                system_program,
                _source_event_id: EVENT_ID,
                _target_event_id: TARGET_EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[5],
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ATTENDEE),
            event_escrow_account(
                source_escrow,
                ORGANIZER,
                EVENT_ID,
                USDC_MINT,
                VAULT,
                DEPOSIT_AMOUNT,
                EVENT_END,
                REFUND_DEADLINE,
                DEPOSIT_AMOUNT,
                DEPOSIT_AMOUNT, // already refunded
                0,
                false,
                source_escrow_bump,
            ),
            // Already refunded — should fail
            attendee_deposit_account(
                source_deposit,
                ATTENDEE,
                source_escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                true, // checked in
                true, // already refunded
                source_deposit_bump,
            ),
            token_account(VAULT, USDC_MINT, source_escrow, 0), // vault already drained
            event_escrow_account(
                target_escrow,
                ORGANIZER,
                TARGET_EVENT_ID,
                USDC_MINT,
                TARGET_VAULT,
                DEPOSIT_AMOUNT,
                EVENT_END + 86_400 * 30,
                REFUND_DEADLINE + 86_400 * 30,
                0,
                0,
                0,
                true,
                target_escrow_bump,
            ),
            empty(target_deposit),
            token_account(TARGET_VAULT, USDC_MINT, target_escrow, 0),
            mint_account(USDC_MINT),
        ],
    );

    assert!(
        result.is_err(),
        "rollover should fail when deposit already refunded"
    );
    // AlreadyRefunded = 4
    let err_code = result.raw_result.unwrap_err();
    assert_eq!(
        err_code,
        quasar_svm::InstructionError::Custom(4),
        "expected AlreadyRefunded (4), got {err_code:?}"
    );
}

#[test]
fn test_rollover_deposit_target_not_active() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;

    let (source_escrow, source_escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (source_deposit, source_deposit_bump) = find_attendee_deposit(&source_escrow, &ATTENDEE);
    let (target_escrow, target_escrow_bump) = find_event_escrow(&ORGANIZER, TARGET_EVENT_ID);
    let (target_deposit, _) = find_attendee_deposit(&target_escrow, &ATTENDEE);

    let ix = with_writable(
        with_signers(
            RolloverDepositInstruction {
                attendee: ATTENDEE,
                source_escrow,
                source_deposit,
                source_vault: VAULT,
                target_escrow,
                target_deposit,
                target_vault: TARGET_VAULT,
                deposit_mint: USDC_MINT,
                rent: RENT,
                token_program,
                system_program,
                _source_event_id: EVENT_ID,
                _target_event_id: TARGET_EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[5],
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ATTENDEE),
            event_escrow_account(
                source_escrow,
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
                false,
                source_escrow_bump,
            ),
            attendee_deposit_account(
                source_deposit,
                ATTENDEE,
                source_escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                true,
                false,
                source_deposit_bump,
            ),
            token_account(VAULT, USDC_MINT, source_escrow, DEPOSIT_AMOUNT),
            // Target event NOT active
            event_escrow_account(
                target_escrow,
                ORGANIZER,
                TARGET_EVENT_ID,
                USDC_MINT,
                TARGET_VAULT,
                DEPOSIT_AMOUNT,
                EVENT_END + 86_400 * 30,
                REFUND_DEADLINE + 86_400 * 30,
                0,
                0,
                0,
                false, // not active
                target_escrow_bump,
            ),
            empty(target_deposit),
            token_account(TARGET_VAULT, USDC_MINT, target_escrow, 0),
            mint_account(USDC_MINT),
        ],
    );

    assert!(
        result.is_err(),
        "rollover should fail when target event not active"
    );
    // EventNotActive = 7
    let err_code = result.raw_result.unwrap_err();
    assert_eq!(
        err_code,
        quasar_svm::InstructionError::Custom(7),
        "expected EventNotActive (7), got {err_code:?}"
    );
}

#[test]
fn test_rollover_deposit_different_organizer() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;

    let (source_escrow, source_escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (source_deposit, source_deposit_bump) = find_attendee_deposit(&source_escrow, &ATTENDEE);
    // Target event has DIFFERENT organizer
    let (target_escrow, target_escrow_bump) = find_event_escrow(&WRONG_ORGANIZER, TARGET_EVENT_ID);
    let (target_deposit, _) = find_attendee_deposit(&target_escrow, &ATTENDEE);

    let ix = with_writable(
        with_signers(
            RolloverDepositInstruction {
                attendee: ATTENDEE,
                source_escrow,
                source_deposit,
                source_vault: VAULT,
                target_escrow,
                target_deposit,
                target_vault: TARGET_VAULT,
                deposit_mint: USDC_MINT,
                rent: RENT,
                token_program,
                system_program,
                _source_event_id: EVENT_ID,
                _target_event_id: TARGET_EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[5],
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ATTENDEE),
            event_escrow_account(
                source_escrow,
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
                false,
                source_escrow_bump,
            ),
            attendee_deposit_account(
                source_deposit,
                ATTENDEE,
                source_escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                true,
                false,
                source_deposit_bump,
            ),
            token_account(VAULT, USDC_MINT, source_escrow, DEPOSIT_AMOUNT),
            // Target event has WRONG_ORGANIZER
            event_escrow_account(
                target_escrow,
                WRONG_ORGANIZER, // different organizer
                TARGET_EVENT_ID,
                USDC_MINT,
                TARGET_VAULT,
                DEPOSIT_AMOUNT,
                EVENT_END + 86_400 * 30,
                REFUND_DEADLINE + 86_400 * 30,
                0,
                0,
                0,
                true,
                target_escrow_bump,
            ),
            empty(target_deposit),
            token_account(TARGET_VAULT, USDC_MINT, target_escrow, 0),
            mint_account(USDC_MINT),
        ],
    );

    assert!(
        result.is_err(),
        "rollover should fail with different organizer"
    );
    // Unauthorized = 9
    let err_code = result.raw_result.unwrap_err();
    assert_eq!(
        err_code,
        quasar_svm::InstructionError::Custom(9),
        "expected Unauthorized (9), got {err_code:?}"
    );
}

#[test]
fn test_rollover_target_mint_mismatch() {
    // Like the happy path, but the TARGET escrow's stored deposit_mint (WRONG_MINT)
    // differs from the source's (USDC_MINT). Guard: rollover_deposit.rs:59
    // target_escrow.deposit_mint() == source_escrow.deposit_mint() @ MintMismatch.
    let mut svm = setup();

    const WRONG_MINT: Pubkey = Pubkey::new_from_array([13; 32]);

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;

    let (source_escrow, source_escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (source_deposit, source_deposit_bump) = find_attendee_deposit(&source_escrow, &ATTENDEE);
    let (target_escrow, target_escrow_bump) = find_event_escrow(&ORGANIZER, TARGET_EVENT_ID);
    let (target_deposit, _) = find_attendee_deposit(&target_escrow, &ATTENDEE);

    let ix = with_writable(
        with_signers(
            RolloverDepositInstruction {
                attendee: ATTENDEE,
                source_escrow,
                source_deposit,
                source_vault: VAULT,
                target_escrow,
                target_deposit,
                target_vault: TARGET_VAULT,
                deposit_mint: USDC_MINT,
                rent: RENT,
                token_program,
                system_program,
                _source_event_id: EVENT_ID,
                _target_event_id: TARGET_EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[5],
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ATTENDEE),
            event_escrow_account(
                source_escrow,
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
                false,
                source_escrow_bump,
            ),
            attendee_deposit_account(
                source_deposit,
                ATTENDEE,
                source_escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                true,
                false,
                source_deposit_bump,
            ),
            token_account(VAULT, USDC_MINT, source_escrow, DEPOSIT_AMOUNT),
            // Target escrow stores a DIFFERENT deposit_mint than source
            event_escrow_account(
                target_escrow,
                ORGANIZER,
                TARGET_EVENT_ID,
                WRONG_MINT, // != source deposit_mint (USDC_MINT)
                TARGET_VAULT,
                DEPOSIT_AMOUNT,
                EVENT_END + 86_400 * 30,
                REFUND_DEADLINE + 86_400 * 30,
                0,
                0,
                0,
                true,
                target_escrow_bump,
            ),
            empty(target_deposit),
            token_account(TARGET_VAULT, USDC_MINT, target_escrow, 0),
            mint_account(USDC_MINT),
        ],
    );

    assert!(
        result.is_err(),
        "rollover should fail when target mint differs"
    );
    let err_code = result.raw_result.unwrap_err();
    assert_eq!(
        err_code,
        quasar_svm::InstructionError::Custom(11),
        "expected MintMismatch (11), got {err_code:?}"
    );
}

#[test]
fn test_rollover_target_incorrect_amount() {
    // Like the happy path, but the TARGET escrow's deposit_amount differs from the
    // source's. Guard: rollover_deposit.rs:60
    // target_escrow.deposit_amount() == source_escrow.deposit_amount()
    // @ IncorrectDepositAmount.
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;

    let (source_escrow, source_escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (source_deposit, source_deposit_bump) = find_attendee_deposit(&source_escrow, &ATTENDEE);
    let (target_escrow, target_escrow_bump) = find_event_escrow(&ORGANIZER, TARGET_EVENT_ID);
    let (target_deposit, _) = find_attendee_deposit(&target_escrow, &ATTENDEE);

    let ix = with_writable(
        with_signers(
            RolloverDepositInstruction {
                attendee: ATTENDEE,
                source_escrow,
                source_deposit,
                source_vault: VAULT,
                target_escrow,
                target_deposit,
                target_vault: TARGET_VAULT,
                deposit_mint: USDC_MINT,
                rent: RENT,
                token_program,
                system_program,
                _source_event_id: EVENT_ID,
                _target_event_id: TARGET_EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[5],
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ATTENDEE),
            event_escrow_account(
                source_escrow,
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
                false,
                source_escrow_bump,
            ),
            attendee_deposit_account(
                source_deposit,
                ATTENDEE,
                source_escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                true,
                false,
                source_deposit_bump,
            ),
            token_account(VAULT, USDC_MINT, source_escrow, DEPOSIT_AMOUNT),
            // Target escrow requires a DIFFERENT deposit_amount than source
            event_escrow_account(
                target_escrow,
                ORGANIZER,
                TARGET_EVENT_ID,
                USDC_MINT,
                TARGET_VAULT,
                DEPOSIT_AMOUNT + 1, // != source deposit_amount
                EVENT_END + 86_400 * 30,
                REFUND_DEADLINE + 86_400 * 30,
                0,
                0,
                0,
                true,
                target_escrow_bump,
            ),
            empty(target_deposit),
            token_account(TARGET_VAULT, USDC_MINT, target_escrow, 0),
            mint_account(USDC_MINT),
        ],
    );

    assert!(
        result.is_err(),
        "rollover should fail when target amount differs"
    );
    let err_code = result.raw_result.unwrap_err();
    assert_eq!(
        err_code,
        quasar_svm::InstructionError::Custom(0),
        "expected IncorrectDepositAmount (0), got {err_code:?}"
    );
}
