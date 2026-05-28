use super::*;

#[test]
fn test_mark_checked_in() {
    let mut svm = setup();

    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    let ix = with_signers(
        MarkCheckedInInstruction {
            organizer: ORGANIZER,
            event_escrow: escrow,
            attendee_deposit: deposit,
            _event_id: EVENT_ID,
        }
        .into(),
        &[0],
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ORGANIZER),
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
            attendee_deposit_account(
                deposit,
                ATTENDEE,
                escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                false, // not checked in yet
                false,
                deposit_bump,
            ),
        ],
    );

    assert!(
        result.is_ok(),
        "mark_checked_in failed: {:?}",
        result.raw_result
    );
    result.print_logs();

    // Verify checked_in flag set (offset: 1 disc + 1 version + 32 attendee + 32 event + 8 amount + 8 deposited_at)
    let deposit_account = result.account(&deposit).unwrap();
    let data = &deposit_account.data;
    let checked_in_offset = 1 + 1 + 32 + 32 + 8 + 8;
    assert_eq!(data[checked_in_offset], 1, "checked_in should be true");

    println!("  MARK_CHECKED_IN CU: {}", result.compute_units_consumed);
}

#[test]
fn test_mark_checked_in_wrong_organizer() {
    let mut svm = setup();

    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    let ix = with_signers(
        MarkCheckedInInstruction {
            organizer: WRONG_ORGANIZER,
            event_escrow: escrow,
            attendee_deposit: deposit,
            _event_id: EVENT_ID,
        }
        .into(),
        &[0],
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(WRONG_ORGANIZER),
            event_escrow_account(
                escrow,
                ORGANIZER, // real organizer
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
            attendee_deposit_account(
                deposit,
                ATTENDEE,
                escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                false,
                false,
                deposit_bump,
            ),
        ],
    );

    assert!(
        result.is_err(),
        "mark_checked_in should fail with wrong organizer"
    );
    println!("  MARK_CHECKED_IN_WRONG_ORGANIZER: correctly rejected");
}

#[test]
fn test_mark_checked_in_deposit_version_mismatch() {
    let mut svm = setup();

    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    // Create a v0 AttendeeDeposit (version = 0)
    let deposit_data = AttendeeDepositData {
        version: 0, // v0 — should be rejected
        attendee: ATTENDEE,
        event: escrow,
        amount: DEPOSIT_AMOUNT,
        deposited_at: 1_699_900_000,
        checked_in: false,
        refunded: false,
        bump: deposit_bump,
        _padding: [0; 11],
    };
    let mut deposit_account_data = vec![2]; // discriminator
    deposit_account_data.extend(wincode::serialize(&deposit_data).unwrap());
    let v0_deposit = Account {
        address: deposit,
        lamports: 1_500_000,
        data: deposit_account_data,
        owner: crate::ID,
        executable: false,
    };

    let ix = with_signers(
        MarkCheckedInInstruction {
            organizer: ORGANIZER,
            event_escrow: escrow,
            attendee_deposit: deposit,
            _event_id: EVENT_ID,
        }
        .into(),
        &[0], // organizer is signer
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ORGANIZER),
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
            v0_deposit,
        ],
    );

    assert!(
        result.is_err(),
        "mark_checked_in should fail with version mismatch"
    );
    let err_code = result.raw_result.unwrap_err();
    // DepositVersionMismatch = 21 (Quasar SVM uses raw error code)
    assert_eq!(
        err_code,
        quasar_svm::InstructionError::Custom(21),
        "expected DepositVersionMismatch (21), got {err_code:?}"
    );
    println!(
        "  MARK_CHECKED_IN_DEPOSIT_VERSION_MISMATCH CU: {}",
        result.compute_units_consumed
    );
}
