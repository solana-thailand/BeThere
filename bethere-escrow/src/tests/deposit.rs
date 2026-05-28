use super::*;

#[test]
fn test_deposit() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, _deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    let ix = with_signers(
        DepositInstruction {
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
                0,    // total_deposited
                0,    // total_refunded
                0,    // total_forfeited
                true, // is_active
                escrow_bump,
            ),
            mint_account(USDC_MINT),
            empty(deposit),
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, DEPOSIT_AMOUNT),
            token_account(VAULT, USDC_MINT, escrow, 0),
        ],
    );

    assert!(result.is_ok(), "deposit failed: {:?}", result.raw_result);
    result.print_logs();

    // Verify vault received USDC
    let vault_account = result.account(&VAULT).unwrap();
    let vault_token = spl_token_interface::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(
        vault_token.amount, DEPOSIT_AMOUNT,
        "vault should have deposit amount"
    );

    // Verify deposit account created
    let deposit_account = result.account(&deposit).unwrap();
    let deposit_data = &deposit_account.data;
    assert_eq!(
        deposit_data[0], 2,
        "discriminator should be 2 for AttendeeDeposit"
    );

    // Verify escrow total_deposited updated
    let escrow_account = result.account(&escrow).unwrap();
    let escrow_data = &escrow_account.data;
    // total_deposited at offset: 1(disc) + 1(version) + 32(org) + 8(event_id) + 32(mint) + 32(vault) + 8(deposit_amount) + 8(event_end) + 8(refund_deadline)
    let total_deposited_offset = 1 + 1 + 32 + 8 + 32 + 32 + 8 + 8 + 8;
    let total_deposited = u64::from_le_bytes(
        escrow_data[total_deposited_offset..total_deposited_offset + 8]
            .try_into()
            .unwrap(),
    );
    assert_eq!(
        total_deposited, DEPOSIT_AMOUNT,
        "total_deposited should be updated"
    );

    println!("  DEPOSIT CU: {}", result.compute_units_consumed);
}

#[test]
fn test_deposit_event_not_active() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, _deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    let ix = with_signers(
        DepositInstruction {
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
                0,
                0,
                0,
                false, // is_active = false
                escrow_bump,
            ),
            mint_account(USDC_MINT),
            empty(deposit),
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, DEPOSIT_AMOUNT),
            token_account(VAULT, USDC_MINT, escrow, 0),
        ],
    );

    assert!(
        result.is_err(),
        "deposit should fail when event is not active"
    );
    println!("  DEPOSIT_NOT_ACTIVE: correctly rejected");
}

#[test]
fn test_deposit_escrow_version_mismatch() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let rent = RENT;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, _deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    // Create a v0 EventEscrow (version = 0 instead of ESCROW_VERSION = 1)
    let escrow_data = EventEscrowData {
        version: 0, // v0 — should be rejected
        organizer: ORGANIZER,
        event_id: EVENT_ID,
        deposit_mint: USDC_MINT,
        vault: VAULT,
        deposit_amount: DEPOSIT_AMOUNT,
        event_end: EVENT_END,
        refund_deadline: REFUND_DEADLINE,
        total_deposited: 0,
        total_refunded: 0,
        total_forfeited: 0,
        is_active: true,
        bump: escrow_bump,
        _padding: [0; 36],
    };
    let mut escrow_account_data = vec![1]; // discriminator
    escrow_account_data.extend(wincode::serialize(&escrow_data).unwrap());
    let v0_escrow = Account {
        address: escrow,
        lamports: 2_000_000,
        data: escrow_account_data,
        owner: crate::ID,
        executable: false,
    };

    let ix = with_writable(
        with_signers(
            DepositInstruction {
                attendee: ATTENDEE,
                event_escrow: escrow,
                deposit_mint: USDC_MINT,
                attendee_deposit: deposit,
                attendee_ta: ATTENDEE_TA,
                vault: VAULT,
                rent,
                token_program,
                system_program,
                _event_id: EVENT_ID,
            }
            .into(),
            &[0], // attendee is signer
        ),
        &[4, 5], // attendee_ta, vault writable
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ATTENDEE),
            v0_escrow,
            mint_account(USDC_MINT),
            empty(deposit),
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, DEPOSIT_AMOUNT),
            token_account(VAULT, USDC_MINT, escrow, 0),
        ],
    );

    assert!(result.is_err(), "deposit should fail with version mismatch");
    let err_code = result.raw_result.unwrap_err();
    // EscrowVersionMismatch = 20 (Quasar SVM uses raw error code)
    assert_eq!(
        err_code,
        quasar_svm::InstructionError::Custom(20),
        "expected EscrowVersionMismatch (20), got {err_code:?}"
    );
    println!(
        "  DEPOSIT_ESCROW_VERSION_MISMATCH CU: {}",
        result.compute_units_consumed
    );
}
