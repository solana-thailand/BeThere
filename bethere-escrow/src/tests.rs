extern crate std;
use {
    alloc::vec,
    bethere_escrow_client::*,
    quasar_svm::{Account, Instruction, Pubkey, QuasarSvm},
    solana_program_pack::Pack,
    spl_token_interface::state::{Account as TokenAccount, AccountState, Mint},
    std::println,
};

// ---------------------------------------------------------------------------
// Deterministic addresses — avoids Pubkey::new_unique() whose global counter
// produces different values depending on test binary layout / discovery order.
// ---------------------------------------------------------------------------
const ORGANIZER: Pubkey = Pubkey::new_from_array([1; 32]);
const ATTENDEE: Pubkey = Pubkey::new_from_array([2; 32]);
const USDC_MINT: Pubkey = Pubkey::new_from_array([3; 32]);
const ORGANIZER_TA: Pubkey = Pubkey::new_from_array([4; 32]);
const ATTENDEE_TA: Pubkey = Pubkey::new_from_array([5; 32]);
const VAULT: Pubkey = Pubkey::new_from_array([6; 32]);
const WRONG_ORGANIZER: Pubkey = Pubkey::new_from_array([7; 32]);
const RENT: Pubkey = quasar_svm::solana_sdk_ids::sysvar::rent::ID;

const DEPOSIT_AMOUNT: u64 = 15_000_000; // $15 USDC (6 decimals)
const EVENT_ID: u64 = 42;
const EVENT_END: i64 = 1_700_000_000; // past timestamp for refund tests
const REFUND_DEADLINE: i64 = 1_700_604_800; // +7 days

// ---------------------------------------------------------------------------
// SVM setup
// ---------------------------------------------------------------------------

fn setup() -> QuasarSvm {
    let elf = std::fs::read("target/deploy/bethere_escrow.so").unwrap();
    QuasarSvm::new()
        .with_program(&crate::ID, &elf)
        .with_token_program()
}

// ---------------------------------------------------------------------------
// Account factories
// ---------------------------------------------------------------------------

fn signer(address: Pubkey) -> Account {
    quasar_svm::token::create_keyed_system_account(&address, 10_000_000_000)
}

fn empty(address: Pubkey) -> Account {
    Account {
        address,
        lamports: 0,
        data: vec![],
        owner: quasar_svm::system_program::ID,
        executable: false,
    }
}

fn mint_account(address: Pubkey) -> Account {
    quasar_svm::token::create_keyed_mint_account(
        &address,
        &Mint {
            mint_authority: Some(ORGANIZER).into(),
            supply: 1_000_000_000_000,
            decimals: 6,
            is_initialized: true,
            freeze_authority: None.into(),
        },
    )
}

fn token_account(address: Pubkey, mint: Pubkey, owner: Pubkey, amount: u64) -> Account {
    quasar_svm::token::create_keyed_token_account(
        &address,
        &TokenAccount {
            mint,
            owner,
            amount,
            state: AccountState::Initialized,
            ..TokenAccount::default()
        },
    )
}

fn event_escrow_account(
    address: Pubkey,
    organizer: Pubkey,
    event_id: u64,
    usdc_mint: Pubkey,
    vault: Pubkey,
    deposit_amount: u64,
    event_end: i64,
    refund_deadline: i64,
    total_deposited: u64,
    total_refunded: u64,
    total_forfeited: u64,
    is_active: bool,
    bump: u8,
) -> Account {
    let escrow_data = EventEscrowData {
        organizer,
        event_id,
        usdc_mint,
        vault,
        deposit_amount,
        event_end,
        refund_deadline,
        total_deposited,
        total_refunded,
        total_forfeited,
        is_active,
        bump,
    };
    let mut data = vec![1]; // discriminator for EventEscrow
    data.extend(wincode::serialize(&escrow_data).unwrap());
    Account {
        address,
        lamports: 2_000_000,
        data,
        owner: crate::ID,
        executable: false,
    }
}

fn attendee_deposit_account(
    address: Pubkey,
    attendee: Pubkey,
    event: Pubkey,
    amount: u64,
    deposited_at: i64,
    checked_in: bool,
    refunded: bool,
    bump: u8,
) -> Account {
    let deposit_data = AttendeeDepositData {
        attendee,
        event,
        amount,
        deposited_at,
        checked_in,
        refunded,
        bump,
    };
    let mut data = vec![2]; // discriminator for AttendeeDeposit
    data.extend(wincode::serialize(&deposit_data).unwrap());
    Account {
        address,
        lamports: 1_500_000,
        data,
        owner: crate::ID,
        executable: false,
    }
}

// PDA derivation helpers

fn find_event_escrow(organizer: &Pubkey, event_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"escrow", organizer.as_ref(), &event_id.to_le_bytes()],
        &crate::ID,
    )
}

fn find_attendee_deposit(event_escrow: &Pubkey, attendee: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"deposit", event_escrow.as_ref(), attendee.as_ref()],
        &crate::ID,
    )
}

/// Mark specific account indices as signers on an instruction.
fn with_signers(mut ix: Instruction, indices: &[usize]) -> Instruction {
    for &i in indices {
        ix.accounts[i].is_signer = true;
    }
    ix
}

/// Patch account metas to be writable (the generated client marks some as readonly
/// when the program actually needs them writable for init/modify).
fn with_writable(mut ix: Instruction, indices: &[usize]) -> Instruction {
    for &i in indices {
        ix.accounts[i].is_writable = true;
    }
    ix
}

// ---------------------------------------------------------------------------
// Serialization helpers for test account data
// ---------------------------------------------------------------------------

#[derive(wincode::SchemaWrite)]
struct EventEscrowData {
    organizer: Pubkey,
    event_id: u64,
    usdc_mint: Pubkey,
    vault: Pubkey,
    deposit_amount: u64,
    event_end: i64,
    refund_deadline: i64,
    total_deposited: u64,
    total_refunded: u64,
    total_forfeited: u64,
    is_active: bool,
    bump: u8,
}

#[derive(wincode::SchemaWrite)]
struct AttendeeDepositData {
    attendee: Pubkey,
    event: Pubkey,
    amount: u64,
    deposited_at: i64,
    checked_in: bool,
    refunded: bool,
    bump: u8,
}

// ===========================================================================
// TEST 1: Create Event — happy path
// ===========================================================================

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
                usdc_mint: USDC_MINT,
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
    // organizer at [1..33]
    assert_eq!(&data[1..33], ORGANIZER.as_ref(), "organizer mismatch");
    // bump at last byte
    assert_eq!(*data.last().unwrap(), escrow_bump, "bump mismatch");

    println!("  CREATE_EVENT CU: {}", result.compute_units_consumed);
}

// ===========================================================================
// TEST 2: Create Event — refund_deadline <= event_end should fail
// ===========================================================================

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
                usdc_mint: USDC_MINT,
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

// ===========================================================================
// TEST 3: Deposit — happy path
// ===========================================================================

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
            usdc_mint: USDC_MINT,
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
    // total_deposited at offset: 1(disc) + 32(org) + 8(event_id) + 32(mint) + 32(vault) + 8(deposit_amount) + 8(event_end) + 8(refund_deadline)
    let total_deposited_offset = 1 + 32 + 8 + 32 + 32 + 8 + 8 + 8;
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

// ===========================================================================
// TEST 4: Deposit — event not active should fail
// ===========================================================================

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
            usdc_mint: USDC_MINT,
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

// ===========================================================================
// TEST 5: Mark Checked In — happy path
// ===========================================================================

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

    // Verify checked_in flag set (offset: 1 disc + 32 attendee + 32 event + 8 amount + 8 deposited_at)
    let deposit_account = result.account(&deposit).unwrap();
    let data = &deposit_account.data;
    let checked_in_offset = 1 + 32 + 32 + 8 + 8;
    assert_eq!(data[checked_in_offset], 1, "checked_in should be true");

    println!("  MARK_CHECKED_IN CU: {}", result.compute_units_consumed);
}

// ===========================================================================
// TEST 6: Mark Checked In — wrong organizer should fail
// ===========================================================================

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

// ===========================================================================
// TEST 7: Refund — happy path (checked-in attendee after event ends)
// ===========================================================================

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
                usdc_mint: USDC_MINT,
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

// ===========================================================================
// TEST 8: Refund — not checked in should fail
// ===========================================================================

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
                usdc_mint: USDC_MINT,
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

// ===========================================================================
// TEST 9: Refund — already refunded should fail
// ===========================================================================

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
                usdc_mint: USDC_MINT,
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

// ===========================================================================
// TEST 10: Claim Forfeited — happy path (no-show attendee)
// ===========================================================================

#[test]
fn test_claim_forfeited() {
    let mut svm = setup();

    // Warp clock past refund_deadline
    svm.warp_to_timestamp(REFUND_DEADLINE + 1);

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    // organizer_ta (idx 2) needs writable for init(idempotent)
    let ix = with_writable(
        with_signers(
            ClaimForfeitedInstruction {
                organizer: ORGANIZER,
                event_escrow: escrow,
                organizer_ta: ORGANIZER_TA,
                usdc_mint: USDC_MINT,
                vault: VAULT,
                rent: RENT,
                token_program,
                system_program,
                _event_id: EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[2], // organizer_ta
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

// ===========================================================================
// TEST 11: Claim Forfeited — before refund deadline should fail
// ===========================================================================

#[test]
fn test_claim_forfeited_before_deadline() {
    let mut svm = setup();
    // Clock NOT warped — default is early, before refund_deadline

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    let ix = with_writable(
        with_signers(
            ClaimForfeitedInstruction {
                organizer: ORGANIZER,
                event_escrow: escrow,
                organizer_ta: ORGANIZER_TA,
                usdc_mint: USDC_MINT,
                vault: VAULT,
                rent: RENT,
                token_program,
                system_program,
                _event_id: EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[2],
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

// ===========================================================================
// TEST 12: Claim Forfeited — nothing to claim should fail
// ===========================================================================

#[test]
fn test_claim_forfeited_nothing_to_claim() {
    let mut svm = setup();
    svm.warp_to_timestamp(REFUND_DEADLINE + 1);

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    let ix = with_writable(
        with_signers(
            ClaimForfeitedInstruction {
                organizer: ORGANIZER,
                event_escrow: escrow,
                organizer_ta: ORGANIZER_TA,
                usdc_mint: USDC_MINT,
                vault: VAULT,
                rent: RENT,
                token_program,
                system_program,
                _event_id: EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[2],
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

// ===========================================================================
// TEST 13: Close Event — happy path
// ===========================================================================

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

// ===========================================================================
// TEST 14: Close Event — still active should fail
// ===========================================================================

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

// ===========================================================================
// TEST 15: Full happy path — create → deposit → check_in → refund → close
// ===========================================================================

#[test]
fn test_full_happy_path() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, _escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, _deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    // --- Step 1: Create Event ---
    let create_ix = with_writable(
        with_signers(
            CreateEventInstruction {
                organizer: ORGANIZER,
                event_escrow: escrow,
                usdc_mint: USDC_MINT,
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
        &[3], // vault needs writable
    );

    let result = svm.process_instruction(
        &create_ix,
        &[
            signer(ORGANIZER),
            empty(escrow),
            mint_account(USDC_MINT),
            token_account(VAULT, USDC_MINT, escrow, 0),
        ],
    );
    assert!(
        result.is_ok(),
        "step 1 create_event failed: {:?}",
        result.raw_result
    );
    println!(
        "  FULL_PATH step 1 CREATE_EVENT OK (CU: {})",
        result.compute_units_consumed
    );

    // --- Step 2: Deposit ---
    let deposit_ix = with_signers(
        DepositInstruction {
            attendee: ATTENDEE,
            event_escrow: escrow,
            usdc_mint: USDC_MINT,
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
        &deposit_ix,
        &[
            signer(ATTENDEE),
            // Use escrow from step 1 result
            result.account(&escrow).unwrap().clone(),
            mint_account(USDC_MINT),
            empty(deposit),
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, DEPOSIT_AMOUNT),
            // Use vault from step 1 result
            result.account(&VAULT).unwrap().clone(),
        ],
    );
    assert!(
        result.is_ok(),
        "step 2 deposit failed: {:?}",
        result.raw_result
    );
    println!(
        "  FULL_PATH step 2 DEPOSIT OK (CU: {})",
        result.compute_units_consumed
    );

    let escrow_after_deposit = result.account(&escrow).unwrap().clone();
    let vault_after_deposit = result.account(&VAULT).unwrap().clone();
    let deposit_after_step2 = result.account(&deposit).unwrap().clone();

    // --- Step 3: Mark Checked In ---
    let checkin_ix = with_signers(
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
        &checkin_ix,
        &[signer(ORGANIZER), escrow_after_deposit, deposit_after_step2],
    );
    assert!(
        result.is_ok(),
        "step 3 mark_checked_in failed: {:?}",
        result.raw_result
    );
    println!(
        "  FULL_PATH step 3 MARK_CHECKED_IN OK (CU: {})",
        result.compute_units_consumed
    );

    let escrow_after_checkin = result.account(&escrow).unwrap().clone();
    let deposit_after_checkin = result.account(&deposit).unwrap().clone();

    // --- Step 4: Warp clock past event_end, then Refund ---
    svm.warp_to_timestamp(EVENT_END + 1);

    let refund_ix = with_writable(
        with_signers(
            RefundInstruction {
                attendee: ATTENDEE,
                event_escrow: escrow,
                usdc_mint: USDC_MINT,
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
        &refund_ix,
        &[
            signer(ATTENDEE),
            escrow_after_checkin,
            mint_account(USDC_MINT),
            deposit_after_checkin,
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, 0),
            vault_after_deposit,
        ],
    );
    assert!(
        result.is_ok(),
        "step 4 refund failed: {:?}",
        result.raw_result
    );
    println!(
        "  FULL_PATH step 4 REFUND OK (CU: {})",
        result.compute_units_consumed
    );

    let escrow_after_refund = result.account(&escrow).unwrap().clone();
    let vault_after_refund = result.account(&VAULT).unwrap().clone();

    // Verify attendee got their USDC back
    let attendee_acct = result.account(&ATTENDEE_TA).unwrap();
    let attendee_token = spl_token_interface::state::Account::unpack(&attendee_acct.data).unwrap();
    assert_eq!(
        attendee_token.amount, DEPOSIT_AMOUNT,
        "attendee should have USDC after refund"
    );

    // --- Step 5: Close Event (set is_active=false first via manual account override) ---
    // Since we can't call a separate "deactivate" instruction, we simulate it by
    // constructing an escrow with is_active=false (would normally be set by organizer)
    let close_ix = with_signers(
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

    // Build escrow with is_active=false for close test
    let escrow_data_for_close = {
        let raw = &escrow_after_refund.data;
        let mut modified = raw.clone();
        // is_active offset: 1 + 32 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8 = 148
        let is_active_offset = 1 + 32 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8;
        modified[is_active_offset] = 0; // set is_active = false
        Account {
            address: escrow,
            lamports: escrow_after_refund.lamports,
            data: modified,
            owner: escrow_after_refund.owner,
            executable: escrow_after_refund.executable,
        }
    };

    let result = svm.process_instruction(
        &close_ix,
        &[signer(ORGANIZER), escrow_data_for_close, vault_after_refund],
    );
    assert!(
        result.is_ok(),
        "step 5 close_event failed: {:?}",
        result.raw_result
    );
    println!(
        "  FULL_PATH step 5 CLOSE_EVENT OK (CU: {})",
        result.compute_units_consumed
    );

    println!("  FULL HAPPY PATH: All 5 steps completed successfully!");
}

// ===========================================================================
// TEST 16: No-show path — deposit → claim_forfeited (organizer gets funds)
// ===========================================================================

#[test]
fn test_no_show_path() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    // Warp past refund_deadline
    svm.warp_to_timestamp(REFUND_DEADLINE + 1);

    // Organizer claims forfeited — attendee deposited but never checked in or refunded
    let ix = with_writable(
        with_signers(
            ClaimForfeitedInstruction {
                organizer: ORGANIZER,
                event_escrow: escrow,
                organizer_ta: ORGANIZER_TA,
                usdc_mint: USDC_MINT,
                vault: VAULT,
                rent: RENT,
                token_program,
                system_program,
                _event_id: EVENT_ID,
            }
            .into(),
            &[0],
        ),
        &[2], // organizer_ta
    );

    // 2 attendees deposited: one checked in and refunded, one no-show
    let attendee1_amount = DEPOSIT_AMOUNT;
    let attendee2_amount = DEPOSIT_AMOUNT;
    let total_deposited = attendee1_amount + attendee2_amount;
    let total_refunded = attendee1_amount; // only attendee1 refunded

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
                total_deposited,
                total_refunded,
                0,
                true,
                escrow_bump,
            ),
            token_account(ORGANIZER_TA, USDC_MINT, ORGANIZER, 0),
            mint_account(USDC_MINT),
            // Vault has attendee2's deposit (attendee1's was already refunded)
            token_account(VAULT, USDC_MINT, escrow, attendee2_amount),
        ],
    );

    assert!(
        result.is_ok(),
        "no-show claim_forfeited failed: {:?}",
        result.raw_result
    );
    result.print_logs();

    // Verify organizer received attendee2's forfeited deposit
    let org_ta_account = result.account(&ORGANIZER_TA).unwrap();
    let org_token = spl_token_interface::state::Account::unpack(&org_ta_account.data).unwrap();
    assert_eq!(
        org_token.amount, attendee2_amount,
        "organizer should receive no-show's deposit"
    );

    // Verify vault is drained
    let vault_account = result.account(&VAULT).unwrap();
    let vault_token = spl_token_interface::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_token.amount, 0, "vault should be empty after claim");

    println!("  NO_SHOW_PATH CU: {}", result.compute_units_consumed);
}

// ===========================================================================
// TEST 17: Deactivate Event — happy path
// ===========================================================================

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

    // Verify is_active set to false (offset: 1 + 32 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8 = 148)
    let escrow_account = result.account(&escrow).unwrap();
    let is_active_offset = 1 + 32 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8;
    assert_eq!(
        escrow_account.data[is_active_offset], 0,
        "is_active should be false"
    );

    println!("  DEACTIVATE_EVENT: OK");
}

// ===========================================================================
// TEST 18: Deactivate Event — wrong organizer should fail
// ===========================================================================

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

// ===========================================================================
// TEST 19: Deactivate Event — already inactive should fail
// ===========================================================================

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

// ===========================================================================
// TEST 20: Close Event — vault not empty should fail
// ===========================================================================

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

// ===========================================================================
// TEST 21: Full lifecycle — create → deposit → check_in → refund → deactivate → close
// ===========================================================================

#[test]
fn test_full_lifecycle_with_deactivate() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, _escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);
    let (deposit, _deposit_bump) = find_attendee_deposit(&escrow, &ATTENDEE);

    // --- Step 1: Create Event ---
    let create_ix = with_writable(
        with_signers(
            CreateEventInstruction {
                organizer: ORGANIZER,
                event_escrow: escrow,
                usdc_mint: USDC_MINT,
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
        &[3],
    );

    let result = svm.process_instruction(
        &create_ix,
        &[
            signer(ORGANIZER),
            empty(escrow),
            mint_account(USDC_MINT),
            token_account(VAULT, USDC_MINT, escrow, 0),
        ],
    );
    assert!(
        result.is_ok(),
        "step 1 create_event: {:?}",
        result.raw_result
    );
    println!("  LIFECYCLE step 1 CREATE OK");

    // --- Step 2: Deposit ---
    let deposit_ix = with_signers(
        DepositInstruction {
            attendee: ATTENDEE,
            event_escrow: escrow,
            usdc_mint: USDC_MINT,
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
        &deposit_ix,
        &[
            signer(ATTENDEE),
            result.account(&escrow).unwrap().clone(),
            mint_account(USDC_MINT),
            empty(deposit),
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, DEPOSIT_AMOUNT),
            result.account(&VAULT).unwrap().clone(),
        ],
    );
    assert!(result.is_ok(), "step 2 deposit: {:?}", result.raw_result);
    println!("  LIFECYCLE step 2 DEPOSIT OK");

    let escrow_after_deposit = result.account(&escrow).unwrap().clone();
    let vault_after_deposit = result.account(&VAULT).unwrap().clone();
    let deposit_after_step2 = result.account(&deposit).unwrap().clone();

    // --- Step 3: Mark Checked In ---
    let checkin_ix = with_signers(
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
        &checkin_ix,
        &[signer(ORGANIZER), escrow_after_deposit, deposit_after_step2],
    );
    assert!(result.is_ok(), "step 3 check_in: {:?}", result.raw_result);
    println!("  LIFECYCLE step 3 CHECK_IN OK");

    let escrow_after_checkin = result.account(&escrow).unwrap().clone();
    let deposit_after_checkin = result.account(&deposit).unwrap().clone();

    // --- Step 4: Warp past event_end, then Refund ---
    svm.warp_to_timestamp(EVENT_END + 1);

    let refund_ix = with_writable(
        with_signers(
            RefundInstruction {
                attendee: ATTENDEE,
                event_escrow: escrow,
                usdc_mint: USDC_MINT,
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
        &refund_ix,
        &[
            signer(ATTENDEE),
            escrow_after_checkin,
            mint_account(USDC_MINT),
            deposit_after_checkin,
            token_account(ATTENDEE_TA, USDC_MINT, ATTENDEE, 0),
            vault_after_deposit,
        ],
    );
    assert!(result.is_ok(), "step 4 refund: {:?}", result.raw_result);
    println!("  LIFECYCLE step 4 REFUND OK");

    let escrow_after_refund = result.account(&escrow).unwrap().clone();
    let vault_after_refund = result.account(&VAULT).unwrap().clone();

    // --- Step 5: Deactivate ---
    let deactivate_ix = with_signers(
        DeactivateEventInstruction {
            organizer: ORGANIZER,
            event_escrow: escrow,
            _event_id: EVENT_ID,
        }
        .into(),
        &[0],
    );

    let result = svm.process_instruction(&deactivate_ix, &[signer(ORGANIZER), escrow_after_refund]);
    assert!(result.is_ok(), "step 5 deactivate: {:?}", result.raw_result);
    println!("  LIFECYCLE step 5 DEACTIVATE OK");

    let escrow_after_deactivate = result.account(&escrow).unwrap().clone();

    // Verify is_active = false
    let is_active_offset = 1 + 32 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8;
    assert_eq!(
        escrow_after_deactivate.data[is_active_offset], 0,
        "is_active should be false after deactivate"
    );

    // --- Step 6: Close Event ---
    let close_ix = with_signers(
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
        &close_ix,
        &[
            signer(ORGANIZER),
            escrow_after_deactivate,
            vault_after_refund,
        ],
    );
    assert!(result.is_ok(), "step 6 close: {:?}", result.raw_result);
    println!("  LIFECYCLE step 6 CLOSE OK");

    println!("  FULL LIFECYCLE WITH DEACTIVATE: All 6 steps completed!");
}

// ===========================================================================
// TEST: Close Deposit — attendee closes after refunded=true
// ===========================================================================

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

// ===========================================================================
// TEST: Close Deposit — should fail if not refunded
// ===========================================================================

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

// ===========================================================================
// TEST: Close Deposit — should fail if wrong signer when escrow still exists
// ===========================================================================

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

// ===========================================================================
// TEST: Close Deposit — anyone can close when event_escrow is closed (GC)
// ===========================================================================

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
