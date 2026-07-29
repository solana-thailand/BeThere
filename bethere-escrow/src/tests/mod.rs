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
const INSTRUCTIONS_SYSVAR: Pubkey = Pubkey::new_from_array([
    6, 167, 213, 23, 24, 123, 209, 102, 53, 218, 212, 4, 85, 253, 194, 192, 193, 36, 198, 143, 33,
    86, 117, 165, 219, 186, 203, 95, 8, 0, 0, 0,
]);

const DEPOSIT_AMOUNT: u64 = 15_000_000; // $15 USDC (6 decimals)
const EVENT_ID: u64 = 42;
const EVENT_END: i64 = 1_700_000_000; // past timestamp for refund tests
const REFUND_DEADLINE: i64 = 1_700_604_800; // +7 days
const TARGET_EVENT_ID: u64 = 43;
const TARGET_VAULT: Pubkey = Pubkey::new_from_array([8; 32]);

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

// Test fixture builder: args map 1:1 to EventEscrow fields (50+ call sites across the
// test suite). Refactoring to a parameter struct would be churn with no functional gain.
#[allow(clippy::too_many_arguments)]
fn event_escrow_account(
    address: Pubkey,
    organizer: Pubkey,
    event_id: u64,
    deposit_mint: Pubkey,
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
        version: crate::state::ESCROW_VERSION,
        organizer,
        event_id,
        deposit_mint,
        vault,
        deposit_amount,
        event_end,
        refund_deadline,
        total_deposited,
        total_refunded,
        total_forfeited,
        is_active,
        bump,
        _padding: [0; 36],
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

// Test fixture builder: args map 1:1 to AttendeeDeposit fields (50+ call sites across
// the test suite). Refactoring to a parameter struct would be churn with no functional gain.
#[allow(clippy::too_many_arguments)]
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
        version: crate::state::DEPOSIT_VERSION,
        attendee,
        event,
        amount,
        deposited_at,
        checked_in,
        refunded,
        bump,
        _padding: [0; 11],
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
    version: u8,
    organizer: Pubkey,
    event_id: u64,
    deposit_mint: Pubkey,
    vault: Pubkey,
    deposit_amount: u64,
    event_end: i64,
    refund_deadline: i64,
    total_deposited: u64,
    total_refunded: u64,
    total_forfeited: u64,
    is_active: bool,
    bump: u8,
    _padding: [u8; 36],
}

#[derive(wincode::SchemaWrite)]
struct AttendeeDepositData {
    version: u8,
    attendee: Pubkey,
    event: Pubkey,
    amount: u64,
    deposited_at: i64,
    checked_in: bool,
    refunded: bool,
    bump: u8,
    _padding: [u8; 11],
}

// ---------------------------------------------------------------------------
// Submodules — one per instruction group
// ---------------------------------------------------------------------------

mod checkin;
mod close;
mod create_event;
mod deposit;
mod differential;
mod introspection;
mod refund;
mod rollover;
mod rollover_flow;
mod reference_oracle;

// ===========================================================================
// Full happy path lifecycle test
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
        &[4], // attendee_ta
    );

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
        // is_active offset: 1(disc) + 1(version) + 32 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8
        let is_active_offset = 1 + 1 + 32 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8;
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
// No-show path — deposit → claim_forfeited (organizer gets funds)
// ===========================================================================

#[test]
fn test_no_show_path() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) = find_event_escrow(&ORGANIZER, EVENT_ID);

    // Two attendees: ATTENDEE (checked in + refunded), ATTENDEE2 (no-show)
    const ATTENDEE2: Pubkey = Pubkey::new_from_array([10; 32]);
    let (deposit2, deposit2_bump) = find_attendee_deposit(&escrow, &ATTENDEE2);

    // Warp past refund_deadline
    svm.warp_to_timestamp(REFUND_DEADLINE + 1);

    // Organizer claims forfeited — ATTENDEE2 deposited but never checked in or refunded
    let ix = with_writable(
        with_signers(
            ClaimForfeitedInstruction {
                organizer: ORGANIZER,
                event_escrow: escrow,
                attendee_deposit: deposit2,
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
        &[1, 2, 3, 5], // event_escrow, attendee_deposit, organizer_ta, vault
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
            attendee_deposit_account(
                deposit2,
                ATTENDEE2,
                escrow,
                attendee2_amount,
                1_699_000_000,
                false, // not checked in (no-show)
                false, // not refunded
                deposit2_bump,
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
// Full lifecycle — create → deposit → check_in → refund → deactivate → close
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

    // Verify is_active = false (offset: 1(disc) + 1(version) + 32 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8)
    let is_active_offset = 1 + 1 + 32 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8;
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
