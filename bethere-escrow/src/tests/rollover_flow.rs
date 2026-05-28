use super::*;

#[test]
fn test_rollover_then_refund_from_target() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;

    let (source_escrow, source_escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (source_deposit, source_deposit_bump) = find_attendee_deposit(&source_escrow, &ATTENDEE);
    let (target_escrow, target_escrow_bump) = find_event_escrow(&ORGANIZER, TARGET_EVENT_ID);
    let (target_deposit, _target_deposit_bump) = find_attendee_deposit(&target_escrow, &ATTENDEE);

    // --- Step 1: Rollover deposit (source → target) ---
    let rollover_ix = with_writable(
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
        &[5], // target_deposit (init), target_vault (mut)
    );

    let result = svm.process_instruction(
        &rollover_ix,
        &[
            signer(ATTENDEE),
            // Source escrow: 1 deposit, 0 refunded, inactive (past event)
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
                0,
                0,
                false,
                source_escrow_bump,
            ),
            // Source deposit: checked in, not refunded
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
            // Target escrow: active, 0 deposits so far
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
    assert!(result.is_ok(), "rollover failed: {:?}", result.raw_result);
    println!("  ROLLOVER_REFUND step 1 ROLLOVER OK");

    // Capture post-rollover state
    let target_escrow_after_rollover = result.account(&target_escrow).unwrap().clone();
    let target_deposit_after_rollover = result.account(&target_deposit).unwrap().clone();
    let target_vault_after_rollover = result.account(&TARGET_VAULT).unwrap().clone();

    // Verify target vault received the USDC
    let target_vault_token =
        spl_token_interface::state::Account::unpack(&target_vault_after_rollover.data).unwrap();
    assert_eq!(
        target_vault_token.amount, DEPOSIT_AMOUNT,
        "target vault should have USDC after rollover"
    );

    // Source vault should be drained
    let source_vault_acct = result.account(&VAULT).unwrap();
    let source_vault_token =
        spl_token_interface::state::Account::unpack(&source_vault_acct.data).unwrap();
    assert_eq!(
        source_vault_token.amount, 0,
        "source vault should be empty after rollover"
    );

    // --- Step 2: Warp past target event_end, then Refund from target ---
    svm.warp_to_timestamp(EVENT_END + 86_400 * 30 + 1);

    let refund_ix = with_writable(
        with_signers(
            RefundInstruction {
                attendee: ATTENDEE,
                event_escrow: target_escrow,
                deposit_mint: USDC_MINT,
                attendee_deposit: target_deposit,
                attendee_ta: ATTENDEE_TA,
                vault: TARGET_VAULT,
                rent: RENT,
                token_program,
                system_program,
                _event_id: TARGET_EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[4], // attendee_ta
    );

    let result = svm.process_instruction(
        &refund_ix,
        &[
            signer(ATTENDEE),
            target_escrow_after_rollover,
            mint_account(USDC_MINT),
            target_deposit_after_rollover,
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, 0),
            target_vault_after_rollover,
        ],
    );
    assert!(
        result.is_ok(),
        "refund from target failed: {:?}",
        result.raw_result
    );
    println!("  ROLLOVER_REFUND step 2 REFUND OK");

    // Verify attendee got USDC back
    let attendee_acct = result.account(&ATTENDEE_TA).unwrap();
    let attendee_token = spl_token_interface::state::Account::unpack(&attendee_acct.data).unwrap();
    assert_eq!(
        attendee_token.amount, DEPOSIT_AMOUNT,
        "attendee should have USDC after refund from target"
    );

    // Verify target vault is drained
    let target_vault_final = result.account(&TARGET_VAULT).unwrap();
    let vault_token =
        spl_token_interface::state::Account::unpack(&target_vault_final.data).unwrap();
    assert_eq!(
        vault_token.amount, 0,
        "target vault should be empty after refund"
    );

    println!("  ROLLOVER_THEN_REFUND: USDC round-trip complete!");
}

#[test]
fn test_double_rollover_rejected() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;

    let (source_escrow, source_escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (source_deposit, source_deposit_bump) = find_attendee_deposit(&source_escrow, &ATTENDEE);
    let (target_escrow, target_escrow_bump) = find_event_escrow(&ORGANIZER, TARGET_EVENT_ID);
    let (target_deposit, _target_deposit_bump) = find_attendee_deposit(&target_escrow, &ATTENDEE);

    // --- Step 1: First rollover succeeds ---
    let rollover_ix = with_writable(
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
        &rollover_ix,
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
        result.is_ok(),
        "first rollover failed: {:?}",
        result.raw_result
    );
    println!("  DOUBLE_ROLLOVER step 1 FIRST ROLLOVER OK");

    // Capture post-rollover source state
    let source_escrow_after = result.account(&source_escrow).unwrap().clone();
    let source_deposit_after = result.account(&source_deposit).unwrap().clone();
    let target_deposit_after = result.account(&target_deposit).unwrap().clone();
    let target_escrow_after = result.account(&target_escrow).unwrap().clone();
    let target_vault_after = result.account(&TARGET_VAULT).unwrap().clone();

    // --- Step 2: Second rollover from same source should fail ---
    // The source deposit is now marked refunded=true, so AlreadyRefunded should trigger.
    // We need a new target event to attempt rollover into.
    const TARGET2_EVENT_ID: u64 = 44;
    const TARGET2_VAULT: Pubkey = Pubkey::new_from_array([11; 32]);
    let (target2_escrow, target2_escrow_bump) = find_event_escrow(&ORGANIZER, TARGET2_EVENT_ID);
    let (target2_deposit, _) = find_attendee_deposit(&target2_escrow, &ATTENDEE);

    let rollover2_ix = with_writable(
        with_signers(
            RolloverDepositInstruction {
                attendee: ATTENDEE,
                source_escrow,
                source_deposit,
                source_vault: VAULT,
                target_escrow: target2_escrow,
                target_deposit: target2_deposit,
                target_vault: TARGET2_VAULT,
                deposit_mint: USDC_MINT,
                rent: RENT,
                token_program,
                system_program,
                _source_event_id: EVENT_ID,
                _target_event_id: TARGET2_EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[5],
    );

    let result = svm.process_instruction(
        &rollover2_ix,
        &[
            signer(ATTENDEE),
            // Source escrow now has total_refunded = DEPOSIT_AMOUNT
            source_escrow_after,
            // Source deposit now has refunded = true
            source_deposit_after.clone(),
            // Source vault is now empty
            result.account(&VAULT).unwrap().clone(),
            event_escrow_account(
                target2_escrow,
                ORGANIZER,
                TARGET2_EVENT_ID,
                USDC_MINT,
                TARGET2_VAULT,
                DEPOSIT_AMOUNT,
                EVENT_END + 86_400 * 60,
                REFUND_DEADLINE + 86_400 * 60,
                0,
                0,
                0,
                true,
                target2_escrow_bump,
            ),
            empty(target2_deposit),
            token_account(TARGET2_VAULT, USDC_MINT, target2_escrow, 0),
            mint_account(USDC_MINT),
        ],
    );

    assert!(
        result.is_err(),
        "second rollover should fail (double spend)"
    );
    // AlreadyRefunded = 4
    let err_code = result.raw_result.unwrap_err();
    assert_eq!(
        err_code,
        quasar_svm::InstructionError::Custom(4),
        "expected AlreadyRefunded (4), got {err_code:?}"
    );
    println!("  DOUBLE_ROLLOVER step 2 SECOND ROLLOVER CORRECTLY REJECTED");

    // First target was NOT in the second instruction's accounts, so its state
    // is unchanged — TARGET_VAULT still holds the original rollover amount.
    // We just need to consume the captured values to avoid warnings.
    let _ = (
        target_vault_after,
        target_deposit_after,
        target_escrow_after,
        source_deposit_after,
    );

    println!("  DOUBLE_ROLLOVER: Double-spend prevention verified!");
}

#[test]
fn test_rollover_then_claim_forfeited_target() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;

    // Second attendee for the no-show scenario
    const ATTENDEE2: Pubkey = Pubkey::new_from_array([10; 32]);

    let (target_escrow, target_escrow_bump) = find_event_escrow(&ORGANIZER, TARGET_EVENT_ID);
    let (target_deposit2, target_deposit2_bump) = find_attendee_deposit(&target_escrow, &ATTENDEE2);

    // Source event PDAs not needed — this test directly sets up target event state
    // as-if attendee A had rolled over (deposited, not checked in, not refunded).
    let _ = token_program;

    // Warp past refund_deadline of target event
    let target_refund_deadline = REFUND_DEADLINE + 86_400 * 30;
    svm.warp_to_timestamp(target_refund_deadline + 1);

    // --- Claim Forfeited for ATTENDEE2 (no-show on target) ---
    let claim_ix = with_writable(
        with_signers(
            ClaimForfeitedInstruction {
                organizer: ORGANIZER,
                event_escrow: target_escrow,
                attendee_deposit: target_deposit2, // claim B's deposit specifically
                organizer_ta: ORGANIZER_TA,
                deposit_mint: USDC_MINT,
                vault: TARGET_VAULT,
                rent: RENT,
                token_program,
                system_program,
                _event_id: TARGET_EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[1, 2, 3, 5], // event_escrow, attendee_deposit, organizer_ta, vault
    );

    // Target escrow state: 2 deposits (1 rollover + 1 direct), 0 refunded, 0 forfeited
    let total_deposited = DEPOSIT_AMOUNT * 2;

    let result = svm.process_instruction(
        &claim_ix,
        &[
            signer(ORGANIZER),
            event_escrow_account(
                target_escrow,
                ORGANIZER,
                TARGET_EVENT_ID,
                USDC_MINT,
                TARGET_VAULT,
                DEPOSIT_AMOUNT,
                EVENT_END + 86_400 * 30,
                target_refund_deadline,
                total_deposited,
                0,
                0,
                true,
                target_escrow_bump,
            ),
            // Attendee B: no-show (not checked in, not refunded)
            attendee_deposit_account(
                target_deposit2,
                ATTENDEE2,
                target_escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                false, // not checked in (no-show)
                false, // not refunded
                target_deposit2_bump,
            ),
            token_account(ORGANIZER_TA, USDC_MINT, ORGANIZER, 0),
            mint_account(USDC_MINT),
            // Vault has both deposits
            token_account(TARGET_VAULT, USDC_MINT, target_escrow, total_deposited),
        ],
    );
    assert!(
        result.is_ok(),
        "claim_forfeited failed: {:?}",
        result.raw_result
    );
    result.print_logs();
    println!("  ROLLOVER_CLAIM_FORFEITED step 1 CLAIM OK");

    // Verify organizer received attendee B's deposit
    let org_ta = result.account(&ORGANIZER_TA).unwrap();
    let org_token = spl_token_interface::state::Account::unpack(&org_ta.data).unwrap();
    assert_eq!(
        org_token.amount, DEPOSIT_AMOUNT,
        "organizer should receive no-show's deposit"
    );

    // Verify vault still has attendee A's rollover deposit
    let vault_acct = result.account(&TARGET_VAULT).unwrap();
    let vault_token = spl_token_interface::state::Account::unpack(&vault_acct.data).unwrap();
    assert_eq!(
        vault_token.amount, DEPOSIT_AMOUNT,
        "vault should still hold attendee A's rollover deposit"
    );

    println!(
        "  ROLLOVER_CLAIM_FORFEITED: Organizer correctly claimed no-show, rollover untouched!"
    );
}

#[test]
fn test_rollover_then_close_source() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;

    let (source_escrow, source_escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (source_deposit, source_deposit_bump) = find_attendee_deposit(&source_escrow, &ATTENDEE);
    let (target_escrow, target_escrow_bump) = find_event_escrow(&ORGANIZER, TARGET_EVENT_ID);
    let (target_deposit, _target_deposit_bump) = find_attendee_deposit(&target_escrow, &ATTENDEE);

    // --- Step 1: Rollover deposit ---
    let rollover_ix = with_writable(
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
        &rollover_ix,
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
                false, // source inactive (past)
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
    assert!(result.is_ok(), "rollover failed: {:?}", result.raw_result);
    println!("  ROLLOVER_CLOSE step 1 ROLLOVER OK");

    let source_escrow_after = result.account(&source_escrow).unwrap().clone();
    let source_vault_after = result.account(&VAULT).unwrap().clone();

    // Consume unused references (target state is verified by vault balance check below)
    let _ = result.account(&target_escrow);
    let _ = result.account(&TARGET_VAULT);

    // --- Step 2: Close source event ---
    // Source escrow has total_deposited = DEPOSIT_AMOUNT, total_refunded = DEPOSIT_AMOUNT
    // (rollover counts as refunded from source's perspective). Vault is empty.
    let close_ix = with_signers(
        CloseEventInstruction {
            organizer: ORGANIZER,
            event_escrow: source_escrow,
            vault: VAULT,
            token_program,
            _event_id: EVENT_ID,
        }
        .into(),
        &[0],
    );

    let result = svm.process_instruction(
        &close_ix,
        &[signer(ORGANIZER), source_escrow_after, source_vault_after],
    );
    assert!(
        result.is_ok(),
        "close source event failed: {:?}",
        result.raw_result
    );
    println!("  ROLLOVER_CLOSE step 2 CLOSE SOURCE OK");

    // --- Step 3: Verify target event still holds deposit ---
    let target_vault_final = result.account(&TARGET_VAULT).unwrap();
    let target_vault_token =
        spl_token_interface::state::Account::unpack(&target_vault_final.data).unwrap();
    assert_eq!(
        target_vault_token.amount, DEPOSIT_AMOUNT,
        "target vault should still hold rollover deposit after source closed"
    );

    println!("  ROLLOVER_CLOSE: Source closed, target still holds deposit!");
}
