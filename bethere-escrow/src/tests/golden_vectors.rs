//! Pinned golden vectors (`.plans/030` §3), escrow consumer.
//!
//! Reads the one fixture in `domain/tests/fixtures/golden_vectors.json`. The
//! PDA values come from `solana find-program-derived-address` and the
//! instruction bytes from Python `struct.pack`, not from our code. These tests
//! check that the program's seeds and the generated client's instruction
//! encoding agree with the worker and flow-harness, which assert the same file.

use {
    super::*,
    alloc::{string::String, vec::Vec},
    core::str::FromStr,
    serde_json::Value,
};

const ANY: Pubkey = Pubkey::new_from_array([9; 32]);

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../domain/tests/fixtures/golden_vectors.json"
    ))
    .expect("golden_vectors.json parses")
}

fn pubkey(v: &Value) -> Pubkey {
    Pubkey::from_str(v.as_str().expect("pubkey is a string")).expect("valid base58 pubkey")
}

fn bump(v: &Value) -> u8 {
    u8::try_from(v.as_u64().expect("bump is an integer")).expect("bump fits u8")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| alloc::format!("{b:02x}")).collect()
}

fn u64_arg(args: &[Value], i: usize) -> u64 {
    args[i].as_u64().expect("u64 arg")
}

fn i64_arg(args: &[Value], i: usize) -> i64 {
    args[i].as_i64().expect("i64 arg")
}

/// Build the client instruction for one fixture case. Account addresses are
/// placeholders: only the instruction data is compared.
fn encode(ix: &str, args: &[Value]) -> Vec<u8> {
    let instruction: Instruction = match ix {
        "create_event" => CreateEventInstruction {
            organizer: ANY,
            event_escrow: ANY,
            deposit_mint: ANY,
            vault: ANY,
            rent: ANY,
            token_program: ANY,
            system_program: ANY,
            event_id: u64_arg(args, 0),
            deposit_amount: u64_arg(args, 1),
            event_end: i64_arg(args, 2),
            refund_deadline: i64_arg(args, 3),
        }
        .into(),
        "deposit" => DepositInstruction {
            attendee: ANY,
            event_escrow: ANY,
            deposit_mint: ANY,
            attendee_deposit: ANY,
            attendee_ta: ANY,
            vault: ANY,
            rent: ANY,
            token_program: ANY,
            system_program: ANY,
            _event_id: u64_arg(args, 0),
        }
        .into(),
        "mark_checked_in" => MarkCheckedInInstruction {
            organizer: ANY,
            event_escrow: ANY,
            attendee_deposit: ANY,
            _event_id: u64_arg(args, 0),
        }
        .into(),
        "refund" => RefundInstruction {
            attendee: ANY,
            event_escrow: ANY,
            deposit_mint: ANY,
            attendee_deposit: ANY,
            attendee_ta: ANY,
            vault: ANY,
            instruction_sysvar: ANY,
            rent: ANY,
            token_program: ANY,
            system_program: ANY,
            _event_id: u64_arg(args, 0),
        }
        .into(),
        "claim_forfeited" => ClaimForfeitedInstruction {
            organizer: ANY,
            event_escrow: ANY,
            attendee_deposit: ANY,
            organizer_ta: ANY,
            deposit_mint: ANY,
            vault: ANY,
            rent: ANY,
            token_program: ANY,
            system_program: ANY,
            _event_id: u64_arg(args, 0),
        }
        .into(),
        "close_event" => CloseEventInstruction {
            organizer: ANY,
            event_escrow: ANY,
            vault: ANY,
            token_program: ANY,
            _event_id: u64_arg(args, 0),
        }
        .into(),
        "deactivate_event" => DeactivateEventInstruction {
            organizer: ANY,
            event_escrow: ANY,
            _event_id: u64_arg(args, 0),
        }
        .into(),
        "close_deposit" => CloseDepositInstruction {
            signer: ANY,
            event_escrow: ANY,
            attendee_deposit: ANY,
            system_program: ANY,
            _event_id: u64_arg(args, 0),
        }
        .into(),
        "rollover_deposit" => RolloverDepositInstruction {
            attendee: ANY,
            source_escrow: ANY,
            source_deposit: ANY,
            source_vault: ANY,
            target_escrow: ANY,
            target_deposit: ANY,
            target_vault: ANY,
            deposit_mint: ANY,
            rent: ANY,
            token_program: ANY,
            system_program: ANY,
            _source_event_id: u64_arg(args, 0),
            _target_event_id: u64_arg(args, 1),
        }
        .into(),
        other => panic!("fixture names an unknown instruction {other:?}"),
    };
    instruction.data
}

#[test]
fn program_id_matches_fixture() {
    let f = fixture();
    assert_eq!(pubkey(&f["escrow_pda"]["program_id"]), crate::ID);
}

#[test]
fn escrow_and_deposit_pdas_match_fixture() {
    let f = fixture();
    let pda = &f["escrow_pda"];
    let organizer = pubkey(&pda["organizer"]);
    let attendee = pubkey(&pda["attendee"]);
    let cases = pda["cases"].as_array().expect("cases array");
    assert!(!cases.is_empty());
    for case in cases {
        let event_id = case["event_id"].as_u64().expect("event_id is u64");

        // The program's own `#[seeds]` output, not a test-side transcription.
        let (escrow, escrow_bump) = Pubkey::find_program_address(
            &crate::state::EventEscrow::seeds(&organizer, event_id).as_slices(),
            &crate::ID,
        );
        assert_eq!(escrow, pubkey(&case["escrow"]), "escrow, event {event_id}");
        assert_eq!(
            escrow_bump,
            bump(&case["escrow_bump"]),
            "escrow bump, event {event_id}"
        );

        let (deposit, deposit_bump) = Pubkey::find_program_address(
            &crate::state::AttendeeDeposit::seeds(&escrow, &attendee).as_slices(),
            &crate::ID,
        );
        assert_eq!(
            deposit,
            pubkey(&case["deposit"]),
            "deposit, event {event_id}"
        );
        assert_eq!(
            deposit_bump,
            bump(&case["deposit_bump"]),
            "deposit bump, event {event_id}"
        );
    }
}

#[test]
fn client_instruction_data_matches_fixture() {
    let f = fixture();
    let cases = f["escrow_ix_data"]["cases"]
        .as_array()
        .expect("cases array");
    assert!(!cases.is_empty());
    for case in cases {
        let ix = case["ix"].as_str().expect("ix name");
        let args = case["args"].as_array().expect("args array");
        let expected = case["hex"].as_str().expect("hex string");
        assert_eq!(hex(&encode(ix, args)), expected, "{ix} {args:?}");
    }
}
