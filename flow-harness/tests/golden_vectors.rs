//! Pinned golden vectors (`.plans/030` §3), flow-harness consumer.
//!
//! Reads the one fixture in `domain/tests/fixtures/golden_vectors.json`. Its
//! PDA/ATA values were derived with `solana find-program-derived-address`, not
//! with our code, so these tests catch the harness drifting from the program
//! and the worker (`[[onchain-struct-offset-drift]]`).

use std::str::FromStr;

use flow_harness::context::{derive_on_chain_event_id, ESCROW_PROGRAM_ID};
use flow_harness::StagingContext;
use serde_json::Value;
use solana_sdk::pubkey::Pubkey;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../domain/tests/fixtures/golden_vectors.json"
    ))
    .expect("golden_vectors.json parses")
}

fn pubkey(v: &Value) -> Pubkey {
    Pubkey::from_str(v.as_str().expect("pubkey is a string")).expect("valid base58 pubkey")
}

fn bump(v: &Value) -> u8 {
    u8::try_from(v.as_u64().expect("bump is an integer")).expect("bump fits u8")
}

#[test]
fn program_id_matches_fixture() {
    let f = fixture();
    assert_eq!(
        f["escrow_pda"]["program_id"].as_str(),
        Some(ESCROW_PROGRAM_ID)
    );
}

#[test]
fn on_chain_event_id_matches_fixture() {
    let f = fixture();
    let cases = f["on_chain_event_id"].as_array().expect("cases array");
    assert!(!cases.is_empty());
    for case in cases {
        let event_id = case["event_id"].as_str().expect("event_id string");
        let expected = case["id"].as_u64().expect("id is u64");
        assert_eq!(
            derive_on_chain_event_id(event_id),
            expected,
            "on-chain id for {event_id:?}"
        );
    }
}

#[test]
fn escrow_deposit_and_vault_addresses_match_fixture() {
    let f = fixture();
    let pda = &f["escrow_pda"];
    let organizer = pubkey(&pda["organizer"]);
    let attendee = pubkey(&pda["attendee"]);
    let mint = pubkey(&pda["usdc_mint"]);
    let cases = pda["cases"].as_array().expect("cases array");
    assert!(!cases.is_empty());
    for case in cases {
        let event_id = case["event_id"].as_u64().expect("event_id is u64");
        let mut ctx = StagingContext::for_testing(
            "https://staging.example.workers.dev",
            event_id,
            organizer,
            attendee,
        )
        .expect("for_testing");
        ctx.deposit_mint = mint;

        let (escrow, escrow_bump) = ctx.event_escrow_pda();
        assert_eq!(escrow, pubkey(&case["escrow"]), "escrow, event {event_id}");
        assert_eq!(
            escrow_bump,
            bump(&case["escrow_bump"]),
            "escrow bump, event {event_id}"
        );

        let (deposit, deposit_bump) = ctx.attendee_deposit_pda();
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

        assert_eq!(
            ctx.vault_ata_address(),
            pubkey(&case["vault"]),
            "vault, event {event_id}"
        );
    }
}
