use super::*;

#[test]
fn test_claim_forfeited() {
    let mut svm = setup();

    // Warp clock past refund_deadline
    svm.warp_to_timestamp(REFUND_DEADLINE + 1);

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    // Accounts that need explicit writable: event_escrow(1), attendee_deposit(2), organizer_ta(3), vault(5)
    let ix = with_writable(
        with_signers(
            ClaimForfeitedInstruction {
                organizer: ORGANIZER,
                event_escrow: escrow,
                attendee_deposit: deposit,
                organizer_ta: ORGANIZER_TA,
                deposit_mint: USDC_MINT,
                vault: VAULT,
                rent: RENT,
                token_program,
                system_program,
                _event_id: EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[3], // organizer_ta
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
                DEPOSIT_AMOUNT, // total_deposited (1 attendee deposited)
                0,              // total_refunded (nobody refunded)
                0,              // total_forfeited
                true,
                escrow_bump,
            ),
            attendee_deposit_account(
                deposit,
                ATTENDEE,
                escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                false, // not checked in (no-show)
                false, // not refunded
                deposit_bump,
            ),
            token_account(ORGANIZER_TA, USDC_MINT, ORGANIZER, 0), // init_if_needed
            mint_account(USDC_MINT),
            token_account(VAULT, USDC_MINT, escrow, DEPOSIT_AMOUNT),
        ],
    );

    assert!(
        result.is_ok(),
        "claim_forfeited failed: {:?}",
        result.raw_result
    );
    result.print_logs();

    // Verify vault is drained
    let vault_account = result.account(&VAULT).unwrap();
    let vault_token = spl_token_interface::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_token.amount, 0, "vault should be empty after claim");

    // Verify organizer received USDC
    let org_ta_account = result.account(&ORGANIZER_TA).unwrap();
    let org_token = spl_token_interface::state::Account::unpack(&org_ta_account.data).unwrap();
    assert_eq!(
        org_token.amount, DEPOSIT_AMOUNT,
        "organizer should receive forfeited amount"
    );

    println!("  CLAIM_FORFEITED CU: {}", result.compute_units_consumed);
}

#[test]
fn test_claim_forfeited_before_deadline() {
    let mut svm = setup();
    // Clock NOT warped — default is early, before refund_deadline

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    let ix = with_writable(
        with_signers(
            ClaimForfeitedInstruction {
                organizer: ORGANIZER,
                event_escrow: escrow,
                attendee_deposit: deposit,
                organizer_ta: ORGANIZER_TA,
                deposit_mint: USDC_MINT,
                vault: VAULT,
                rent: RENT,
                token_program,
                system_program,
                _event_id: EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[3],
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
                false,
                false,
                deposit_bump,
            ),
            token_account(ORGANIZER_TA, USDC_MINT, ORGANIZER, 0),
            mint_account(USDC_MINT),
            token_account(VAULT, USDC_MINT, escrow, DEPOSIT_AMOUNT),
        ],
    );

    assert!(
        result.is_err(),
        "claim_forfeited should fail before refund deadline"
    );
    println!("  CLAIM_FORFEITED_BEFORE_DEADLINE: correctly rejected");
}

#[test]
fn test_claim_forfeited_nothing_to_claim() {
    let mut svm = setup();
    svm.warp_to_timestamp(REFUND_DEADLINE + 1);

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    let ix = with_writable(
        with_signers(
            ClaimForfeitedInstruction {
                organizer: ORGANIZER,
                event_escrow: escrow,
                attendee_deposit: deposit,
                organizer_ta: ORGANIZER_TA,
                deposit_mint: USDC_MINT,
                vault: VAULT,
                rent: RENT,
                token_program,
                system_program,
                _event_id: EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[3],
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
                DEPOSIT_AMOUNT, // total_refunded = total_deposited → nothing forfeited
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
                true, // already refunded — AlreadyRefunded error
                deposit_bump,
            ),
            token_account(ORGANIZER_TA, USDC_MINT, ORGANIZER, 0),
            mint_account(USDC_MINT),
            token_account(VAULT, USDC_MINT, escrow, 0), // vault empty
        ],
    );

    assert!(
        result.is_err(),
        "claim_forfeited should fail when nothing to claim"
    );
    println!("  CLAIM_FORFEITED_NOTHING: correctly rejected");
}

#[test]
fn test_claim_forfeited_checked_in_rejected() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    // Warp past refund_deadline
    svm.warp_to_timestamp(REFUND_DEADLINE + 1);

    // Organizer tries to claim forfeited for an attendee who WAS checked in
    let ix = with_writable(
        with_signers(
            ClaimForfeitedInstruction {
                organizer: ORGANIZER,
                event_escrow: escrow,
                attendee_deposit: deposit,
                organizer_ta: ORGANIZER_TA,
                deposit_mint: USDC_MINT,
                vault: VAULT,
                rent: RENT,
                token_program,
                system_program,
                _event_id: EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[3], // organizer_ta
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
                DEPOSIT_AMOUNT, // total_deposited
                0,              // total_refunded
                0,              // total_forfeited
                true,
                escrow_bump,
            ),
            attendee_deposit_account(
                deposit,
                ATTENDEE,
                escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                true,  // CHECKED IN — organizer should NOT be able to claim
                false, // not refunded
                deposit_bump,
            ),
            token_account(ORGANIZER_TA, USDC_MINT, ORGANIZER, 0),
            mint_account(USDC_MINT),
            token_account(VAULT, USDC_MINT, escrow, DEPOSIT_AMOUNT),
        ],
    );

    assert!(
        result.raw_result.is_err(),
        "claim_forfeited should FAIL for checked-in attendee — expected AttendeeCheckedIn (0x5)"
    );
    println!("  CLAIM_FORFEITED_CHECKED_IN: correctly rejected with AttendeeCheckedIn");
}

#[test]
fn test_close_event() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    let ix = with_signers(
        CloseEventInstruction {
            organizer: ORGANIZER,
            event_escrow: escrow,
            vault: VAULT,
            token_program,
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
                DEPOSIT_AMOUNT, // all refunded
                0,
                false, // is_active = false
                escrow_bump,
            ),
            token_account(VAULT, USDC_MINT, escrow, 0), // vault empty
        ],
    );

    assert!(
        result.is_ok(),
        "close_event failed: {:?}",
        result.raw_result
    );
    result.print_logs();
    println!("  CLOSE_EVENT CU: {}", result.compute_units_consumed);
}

#[test]
fn test_close_event_still_active() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    let ix = with_signers(
        CloseEventInstruction {
            organizer: ORGANIZER,
            event_escrow: escrow,
            vault: VAULT,
            token_program,
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
                DEPOSIT_AMOUNT,
                0,
                true, // is_active = true — should fail
                escrow_bump,
            ),
            token_account(VAULT, USDC_MINT, escrow, 0),
        ],
    );

    assert!(
        result.is_err(),
        "close_event should fail when event still active"
    );
    println!("  CLOSE_EVENT_STILL_ACTIVE: correctly rejected");
}

#[test]
fn test_close_event_vault_not_empty() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    let ix = with_signers(
        CloseEventInstruction {
            organizer: ORGANIZER,
            event_escrow: escrow,
            vault: VAULT,
            token_program,
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
                DEPOSIT_AMOUNT, // total_deposited
                0,              // total_refunded
                0,              // total_forfeited → 15M unsettled!
                false,          // is_active = false
                escrow_bump,
            ),
            // Vault still has tokens
            token_account(VAULT, USDC_MINT, escrow, DEPOSIT_AMOUNT),
        ],
    );

    assert!(
        result.is_err(),
        "close_event should fail when vault still has tokens"
    );
    println!("  CLOSE_EVENT_VAULT_NOT_EMPTY: correctly rejected");
}

#[test]
fn test_close_event_vault_griefing() {
    // Dust-griefing case: accounting IS settled (total_deposited == total_refunded +
    // total_forfeited), so the accounting check (close_event.rs:54) passes — but the
    // vault token account still holds 1 unit of dust, so the vault-balance check
    // (close_event.rs:61) rejects with VaultNotEmpty. This exercises a DIFFERENT branch
    // than test_close_event_vault_not_empty (which trips the accounting check).
    let mut svm = setup();

    // Warp past refund_deadline as the close-flow lifecycle does.
    svm.warp_to_timestamp(REFUND_DEADLINE + 1);

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    let ix = with_signers(
        CloseEventInstruction {
            organizer: ORGANIZER,
            event_escrow: escrow,
            vault: VAULT,
            token_program,
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
                DEPOSIT_AMOUNT, // total_deposited
                DEPOSIT_AMOUNT, // total_refunded — accounting IS settled
                0,              // total_forfeited
                false,          // is_active = false
                escrow_bump,
            ),
            // Vault holds 1 unit of dust despite settled accounting (external airdrop)
            token_account(VAULT, USDC_MINT, escrow, 1),
        ],
    );

    assert!(
        result.is_err(),
        "close_event should fail on non-empty vault (dust griefing)"
    );
    let err_code = result.raw_result.unwrap_err();
    assert_eq!(
        err_code,
        quasar_svm::InstructionError::Custom(15),
        "expected VaultNotEmpty (15) from the vault-balance check, got {err_code:?}"
    );

    // Positive control: the SAME settled-accounting fixture WITH the dust removed
    // (vault balance 0) must SUCCEED. This proves the accounting check (close_event.rs:54)
    // was passed and the rejection above is UNIQUELY the vault-balance branch (line 61) —
    // not the accounting branch (which also returns Custom(15)). Without this control a
    // future fixture change could silently pass the griefing assert via the line-54 path.
    let mut svm_ok = setup();
    svm_ok.warp_to_timestamp(REFUND_DEADLINE + 1);
    let ok = svm_ok.process_instruction(
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
                DEPOSIT_AMOUNT, // total_deposited
                DEPOSIT_AMOUNT, // total_refunded — accounting settled (same as above)
                0,              // total_forfeited
                false,          // is_active
                escrow_bump,
            ),
            token_account(VAULT, USDC_MINT, escrow, 0), // dust removed → empty vault
        ],
    );
    assert!(
        ok.is_ok(),
        "same settled fixture with an EMPTY vault should close OK (proves the reject was \
         uniquely the vault-balance branch): {:?}",
        ok.raw_result
    );
    println!("  CLOSE_EVENT_VAULT_GRIEFING: rejected via vault-balance branch (accounting-settled control passes)");
}

#[test]
fn test_deactivate_event() {
    let mut svm = setup();

    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    let ix = with_signers(
        DeactivateEventInstruction {
            organizer: ORGANIZER,
            event_escrow: escrow,
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
                true, // is_active = true
                escrow_bump,
            ),
        ],
    );

    assert!(
        result.is_ok(),
        "deactivate_event failed: {:?}",
        result.raw_result
    );

    // Verify is_active set to false (offset: 1(disc) + 1(version) + 32 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8)
    let escrow_account = result.account(&escrow).unwrap();
    let is_active_offset = 1 + 1 + 32 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8;
    assert_eq!(
        escrow_account.data[is_active_offset], 0,
        "is_active should be false"
    );

    println!("  DEACTIVATE_EVENT: OK");
}

#[test]
fn test_deactivate_event_wrong_organizer() {
    let mut svm = setup();

    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    let ix = with_signers(
        DeactivateEventInstruction {
            organizer: WRONG_ORGANIZER,
            event_escrow: escrow,
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
        ],
    );

    assert!(
        result.is_err(),
        "deactivate_event should fail with wrong organizer"
    );
    println!("  DEACTIVATE_EVENT_WRONG_ORGANIZER: correctly rejected");
}

#[test]
fn test_deactivate_event_already_inactive() {
    let mut svm = setup();

    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    let ix = with_signers(
        DeactivateEventInstruction {
            organizer: ORGANIZER,
            event_escrow: escrow,
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
                false, // already inactive
                escrow_bump,
            ),
        ],
    );

    assert!(
        result.is_err(),
        "deactivate_event should fail when already inactive"
    );
    println!("  DEACTIVATE_EVENT_ALREADY_INACTIVE: correctly rejected");
}

#[test]
fn test_close_deposit_after_refund() {
    let mut svm = setup();

    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    let ix = with_signers(
        CloseDepositInstruction {
            signer: ATTENDEE,
            event_escrow: escrow,
            attendee_deposit: deposit,
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
            // event_escrow still exists (not closed) — but has data
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
                DEPOSIT_AMOUNT, // total_refunded == total_deposited
                0,
                false, // inactive
                escrow_bump,
            ),
            attendee_deposit_account(
                deposit,
                ATTENDEE,
                escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                true, // checked in
                true, // refunded
                deposit_bump,
            ),
        ],
    );

    assert!(
        result.is_ok(),
        "close_deposit after refund failed: {:?}",
        result.raw_result
    );
    result.print_logs();

    // Verify deposit account was closed (zero lamports)
    let deposit_account = result.account(&deposit).unwrap();
    assert_eq!(
        deposit_account.lamports, 0,
        "deposit should have 0 lamports after close"
    );

    // Verify signer received rent
    let signer_account = result.account(&ATTENDEE).unwrap();
    assert!(
        signer_account.lamports > 10_000_000_000,
        "signer should have received rent lamports"
    );

    println!(
        "  CLOSE_DEPOSIT_AFTER_REFUND CU: {}",
        result.compute_units_consumed
    );
}

#[test]
fn test_close_deposit_not_refunded() {
    let mut svm = setup();

    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    let ix = with_signers(
        CloseDepositInstruction {
            signer: ATTENDEE,
            event_escrow: escrow,
            attendee_deposit: deposit,
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
                DEPOSIT_AMOUNT,
                0, // nothing refunded
                0,
                false,
                escrow_bump,
            ),
            attendee_deposit_account(
                deposit,
                ATTENDEE,
                escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                true,
                false, // NOT refunded — should fail
                deposit_bump,
            ),
        ],
    );

    assert!(
        result.is_err(),
        "close_deposit should fail when deposit not refunded"
    );
    println!("  CLOSE_DEPOSIT_NOT_REFUNDED: correctly rejected");
}

#[test]
fn test_close_deposit_wrong_signer() {
    let mut svm = setup();

    const WRONG_ATTENDEE: Pubkey = Pubkey::new_from_array([8; 32]);

    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    // WRONG_ATTENDEE tries to close ATTENDEE's deposit while escrow exists
    let ix = with_signers(
        CloseDepositInstruction {
            signer: WRONG_ATTENDEE,
            event_escrow: escrow,
            attendee_deposit: deposit,
            system_program,
            _event_id: EVENT_ID,
        }
        .into(),
        &[0],
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(WRONG_ATTENDEE),
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
                DEPOSIT_AMOUNT,
                0,
                false,
                escrow_bump,
            ),
            attendee_deposit_account(
                deposit,
                ATTENDEE,
                escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                true,
                true, // refunded
                deposit_bump,
            ),
        ],
    );

    assert!(
        result.is_err(),
        "close_deposit should fail when wrong signer tries to close while escrow exists"
    );
    println!("  CLOSE_DEPOSIT_WRONG_SIGNER: correctly rejected");
}

#[test]
fn test_close_deposit_gc_after_event_closed() {
    let mut svm = setup();

    const RANDOM_USER: Pubkey = Pubkey::new_from_array([9; 32]);

    let system_program = quasar_svm::system_program::ID;
    let (escrow, _escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    // RANDOM_USER closes ATTENDEE's deposit after event_escrow has been closed
    let ix = with_signers(
        CloseDepositInstruction {
            signer: RANDOM_USER,
            event_escrow: escrow,
            attendee_deposit: deposit,
            system_program,
            _event_id: EVENT_ID,
        }
        .into(),
        &[0],
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(RANDOM_USER),
            // event_escrow is closed — empty account (zero data)
            empty(escrow),
            attendee_deposit_account(
                deposit,
                ATTENDEE,
                escrow,
                DEPOSIT_AMOUNT,
                1_699_000_000,
                false, // not checked in
                false, // not refunded — but GC allows closing anyway
                deposit_bump,
            ),
        ],
    );

    assert!(
        result.is_ok(),
        "GC close_deposit should succeed when event_escrow is closed: {:?}",
        result.raw_result
    );
    result.print_logs();

    // Verify deposit account was closed
    let deposit_account = result.account(&deposit).unwrap();
    assert_eq!(
        deposit_account.lamports, 0,
        "deposit should have 0 lamports after GC close"
    );

    // Verify GC caller received rent
    let gc_account = result.account(&RANDOM_USER).unwrap();
    assert!(
        gc_account.lamports > 10_000_000_000,
        "GC caller should have received rent lamports"
    );

    println!(
        "  CLOSE_DEPOSIT_GC_AFTER_EVENT_CLOSED CU: {}",
        result.compute_units_consumed
    );
}
