//! Tests for instruction introspection (SEC-010).
//!
//! The refund handler scans sibling instructions via the Instructions sysvar
//! to enforce that `close_deposit` follows `refund` in the same transaction.
//! This prevents rent leaks from orphaned AttendeeDeposit PDAs.
//!
//! The scanning is inlined in the refund handler to avoid BPF call depth
//! issues. The `require_distinct` and `validate_version` calls were removed
//! from the refund handler to free call depth frames (PDA seeds already
//! guarantee distinct addresses and correct account types).
//!
//! A quasar-svm patch ensures the Instructions sysvar is correctly populated
//! when the sysvar address IS in the message account keys (via UncheckedAccount)
//! but NOT in the caller-provided accounts. Without this patch, the SVM gave
//! the sysvar position an empty default account.

use super::*;

#[test]
fn test_refund_requires_instruction_sysvar_account() {
    // Verify that the RefundInstruction struct includes instruction_sysvar.
    // The generated client includes it, so the SVM will reject transactions
    // that don't provide it (NotEnoughAccountKeys).
    // This is implicitly tested by all refund tests — they all pass the
    // INSTRUCTIONS_SYSVAR address.
    //
    // This test documents the requirement.
    let ix: Instruction = RefundInstruction {
        attendee: ATTENDEE,
        event_escrow: Pubkey::default(),
        deposit_mint: USDC_MINT,
        attendee_deposit: Pubkey::default(),
        attendee_ta: ATTENDEE_TA,
        vault: VAULT,
        instruction_sysvar: INSTRUCTIONS_SYSVAR,
        rent: RENT,
        token_program: quasar_svm::SPL_TOKEN_PROGRAM_ID,
        system_program: quasar_svm::system_program::ID,
        _event_id: EVENT_ID,
    }
    .into();

    // The instruction should have 10 account metas (9 original + instruction_sysvar)
    assert_eq!(
        ix.accounts.len(),
        10,
        "refund should have 10 accounts including instruction_sysvar"
    );

    // The instruction_sysvar should be at index 6 (after vault, before rent)
    assert_eq!(ix.accounts[6].pubkey, INSTRUCTIONS_SYSVAR);
    assert!(!ix.accounts[6].is_signer);
    assert!(!ix.accounts[6].is_writable);
}

#[test]
fn test_refund_close_deposit_chain_succeeds() {
    // Verify that refund + close_deposit in a single atomic transaction works.
    // This is the intended production flow — Worker builds both instructions.
    let mut svm = setup();
    svm.warp_to_timestamp(EVENT_END + 1);

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    // Refund instruction
    let refund_ix = with_writable(
        with_signers(
            RefundInstruction {
                attendee: ATTENDEE,
                event_escrow: escrow,
                deposit_mint: USDC_MINT,
                attendee_deposit: deposit,
                attendee_ta: ATTENDEE_TA,
                vault: VAULT,
                instruction_sysvar: INSTRUCTIONS_SYSVAR,
                rent: RENT,
                token_program,
                system_program,
                _event_id: EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[4], // attendee_ta needs writable
    );

    // Close deposit instruction
    let close_deposit_ix = with_signers(
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

    let result = svm.process_instruction_chain(
        &[refund_ix, close_deposit_ix],
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
                false,
                deposit_bump,
            ),
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, 0),
            token_account(VAULT, USDC_MINT, escrow, DEPOSIT_AMOUNT),
        ],
    );

    assert!(
        result.is_ok(),
        "refund+close_deposit chain failed: {:?}",
        result.raw_result
    );

    // Verify vault drained
    let vault_account = result.account(&VAULT).unwrap();
    let vault_token = spl_token_interface::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_token.amount, 0, "vault should be empty");

    // Verify attendee got USDC
    let attendee_acct = result.account(&ATTENDEE_TA).unwrap();
    let attendee_token = spl_token_interface::state::Account::unpack(&attendee_acct.data).unwrap();
    assert_eq!(
        attendee_token.amount, DEPOSIT_AMOUNT,
        "attendee should have USDC"
    );

    // Verify deposit account was closed (0 lamports)
    let deposit_acct = result.account(&deposit).unwrap();
    assert_eq!(
        deposit_acct.lamports, 0,
        "deposit should be closed (rent reclaimed)"
    );
}

#[test]
fn test_refund_wrong_sysvar_address_rejected() {
    // If someone passes the wrong address as instruction_sysvar,
    // the handler should reject with UnsupportedSysvar.
    let mut svm = setup();
    svm.warp_to_timestamp(EVENT_END + 1);

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    // Use a wrong address for instruction_sysvar
    let wrong_sysvar: Pubkey = Pubkey::new_from_array([99; 32]);

    let refund_ix = with_writable(
        with_signers(
            RefundInstruction {
                attendee: ATTENDEE,
                event_escrow: escrow,
                deposit_mint: USDC_MINT,
                attendee_deposit: deposit,
                attendee_ta: ATTENDEE_TA,
                vault: VAULT,
                instruction_sysvar: wrong_sysvar,
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
        &refund_ix,
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
                false,
                deposit_bump,
            ),
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, 0),
            token_account(VAULT, USDC_MINT, escrow, DEPOSIT_AMOUNT),
            // Wrong sysvar account — just an empty account
            empty(wrong_sysvar),
        ],
    );

    assert!(
        result.is_err(),
        "refund with wrong sysvar address should fail"
    );
}

#[test]
fn test_refund_without_close_deposit_rejected() {
    // SEC-010: The refund handler scans sibling instructions via the Instructions
    // sysvar to enforce that close_deposit follows refund. A standalone refund
    // without close_deposit should be rejected with RefundRequiresClose.
    let mut svm = setup();
    svm.warp_to_timestamp(EVENT_END + 1);

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    let refund_ix = with_writable(
        with_signers(
            RefundInstruction {
                attendee: ATTENDEE,
                event_escrow: escrow,
                deposit_mint: USDC_MINT,
                attendee_deposit: deposit,
                attendee_ta: ATTENDEE_TA,
                vault: VAULT,
                instruction_sysvar: INSTRUCTIONS_SYSVAR,
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

    // Single instruction chain — refund only, no close_deposit
    let result = svm.process_instruction_chain(
        &[refund_ix],
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
                false,
                deposit_bump,
            ),
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, 0),
            token_account(VAULT, USDC_MINT, escrow, DEPOSIT_AMOUNT),
        ],
    );

    assert!(
        result.is_err(),
        "standalone refund without close_deposit should be rejected"
    );
}
