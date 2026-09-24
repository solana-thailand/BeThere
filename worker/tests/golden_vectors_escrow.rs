//! Escrow PDA golden vectors (`.plans/030` §3).
//!
//! The expected addresses in `domain/tests/fixtures/golden_vectors.json` were
//! derived with `solana find-program-derived-address`, not by this code, so a
//! drift in seeds, seed order, the bump search or the ATA derivation fails
//! here instead of as an `IllegalOwner` simulation error on-chain
//! (`[[onchain-struct-offset-drift]]`). The two non-trivial cases are real
//! event ids and need bump 254, so they also exercise the bump loop.

use event_checkin_domain::onchain::on_chain_event_id;
use event_checkin_worker::solana_escrow::{
    derive_attendee_deposit_pdas, derive_escrow_address, get_associated_token_address,
    pubkey_from_base58, pubkey_to_base58,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    escrow_pda: EscrowPda,
    on_chain_event_id: Vec<OnChainId>,
}

#[derive(Deserialize)]
struct OnChainId {
    event_id: String,
    id: u64,
}

#[derive(Deserialize)]
struct EscrowPda {
    usdc_mint: String,
    organizer: String,
    attendee: String,
    cases: Vec<EscrowCase>,
}

#[derive(Deserialize)]
struct EscrowCase {
    event_id: u64,
    escrow: String,
    deposit: String,
    vault: String,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!(
        "../../domain/tests/fixtures/golden_vectors.json"
    ))
    .expect("golden_vectors.json must parse")
}

#[tokio::test]
async fn escrow_deposit_and_vault_addresses_are_pinned() {
    let pda = fixture().escrow_pda;
    assert!(pda.cases.len() >= 3, "fixture lost its escrow cases");
    let mint = pubkey_from_base58(&pda.usdc_mint).expect("mint");
    for case in &pda.cases {
        let escrow = derive_escrow_address(&pda.organizer, case.event_id)
            .await
            .expect("escrow PDA");
        assert_eq!(
            escrow, case.escrow,
            "escrow PDA for event {}",
            case.event_id
        );

        let pairs = derive_attendee_deposit_pdas(
            &pda.organizer,
            case.event_id,
            std::slice::from_ref(&pda.attendee),
        )
        .await
        .expect("deposit PDA");
        assert_eq!(pairs.len(), 1);
        assert_eq!(
            pairs[0].1, case.deposit,
            "deposit PDA for event {}",
            case.event_id
        );

        let escrow_bytes = pubkey_from_base58(&escrow).expect("escrow bytes");
        let vault = get_associated_token_address(&escrow_bytes, &mint)
            .await
            .expect("vault ATA");
        assert_eq!(
            pubkey_to_base58(&vault),
            case.vault,
            "vault for event {}",
            case.event_id
        );
    }
}

/// Slug → on-chain id → PDA, end to end, for the real event ids.
#[tokio::test]
async fn real_event_slugs_reach_their_pinned_escrow() {
    let fixture = fixture();
    let mut reached = 0;
    for id_case in &fixture.on_chain_event_id {
        let id = on_chain_event_id(&id_case.event_id);
        assert_eq!(id, id_case.id, "{:?}", id_case.event_id);
        let Some(case) = fixture.escrow_pda.cases.iter().find(|c| c.event_id == id) else {
            continue;
        };
        let escrow = derive_escrow_address(&fixture.escrow_pda.organizer, id)
            .await
            .expect("escrow PDA");
        assert_eq!(escrow, case.escrow, "{:?}", id_case.event_id);
        reached += 1;
    }
    assert!(
        reached >= 2,
        "only {reached} slugs have a pinned escrow case"
    );
}
