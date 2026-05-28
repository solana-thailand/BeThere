use super::*;

#[test]
fn test_create_event() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    // vault (idx 3) needs writable for init(idempotent)
    let ix = with_writable(
        with_signers(
            CreateEventInstruction {
                organizer: ORGANIZER,
                event_escrow: escrow,
                deposit_mint: USDC_MINT,
                vault: VAULT,
                rent: RENT,
                token_program,
                system_program,
                event_id: EVENT_ID,
                deposit_amount: DEPOSIT_AMOUNT,
                event_end: EVENT_END,
                refund_deadline: REFUND_DEADLINE,
            }
            .into(),
            &[0],
        ),
        &[3], // vault
    );

    let result = svm.process_instruction(
        &ix,
        &[
            signer(ORGANIZER),
            empty(escrow),
            mint_account(USDC_MINT),
            // Pre-create vault as initialized token account for init(idempotent)
            token_account(VAULT, USDC_MINT, escrow, 0),
        ],
    );

    assert!(
        result.is_ok(),
        "create_event failed: {:?}",
        result.raw_result
    );
    result.print_logs();

    // Verify escrow account data
    let escrow_account = result.account(&escrow).unwrap();
    let data = &escrow_account.data;
    // discriminator at [0] should be 1
    assert_eq!(data[0], 1, "discriminator should be 1 for EventEscrow");
    // organizer at [2..34] (1 disc + 1 version)
    assert_eq!(&data[2..34], ORGANIZER.as_ref(), "organizer mismatch");
    // bump at offset: 1(disc) + 1(version) + 32(org) + 8(event_id) + 32(mint) + 32(vault) + 8(deposit_amount) + 8(event_end) + 8(refund_deadline) + 8(total_deposited) + 8(total_refunded) + 8(total_forfeited) + 1(is_active)
    let bump_offset = 1 + 1 + 32 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8 + 1;
    assert_eq!(data[bump_offset], escrow_bump, "bump mismatch");

    println!("  CREATE_EVENT CU: {}", result.compute_units_consumed);
}

#[test]
fn test_create_event_bad_deadline() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, _bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    let ix = with_writable(
        with_signers(
            CreateEventInstruction {
                organizer: ORGANIZER,
                event_escrow: escrow,
                deposit_mint: USDC_MINT,
                vault: VAULT,
                rent: RENT,
                token_program,
                system_program,
                event_id: EVENT_ID,
                deposit_amount: DEPOSIT_AMOUNT,
                event_end: EVENT_END,
                refund_deadline: EVENT_END, // same as event_end — should fail
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
            empty(escrow),
            mint_account(USDC_MINT),
            token_account(VAULT, USDC_MINT, escrow, 0),
        ],
    );

    assert!(
        result.is_err(),
        "create_event should fail when refund_deadline <= event_end"
    );
    println!("  CREATE_EVENT_BAD_DEADLINE: correctly rejected");
}
