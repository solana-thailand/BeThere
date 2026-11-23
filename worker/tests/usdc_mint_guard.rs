//! The mainnet USDC mint is pinned to the real address (`.issues/196`).
//!
//! The constant was once written with a wrong last character. Nothing caught
//! it: both spellings are valid base58, the unit test compared the constant
//! with itself, and mainnet is not selected anywhere yet. On mainnet the
//! wrong spelling has no account, so every deposit would have failed.

use std::fs;
use std::path::Path;

/// Circle's USDC mint on mainnet-beta (Token program, 6 decimals), read from
/// chain on 2026-11-23.
const REAL_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

fn read(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn the_worker_constant_is_the_real_mainnet_mint() {
    let code = read("src/solana_escrow/mod.rs");
    let line = code
        .lines()
        .find(|l| l.contains("const USDC_MINT_MAINNET"))
        .expect("USDC_MINT_MAINNET is declared in solana_escrow/mod.rs");
    assert!(
        line.contains(&format!("\"{REAL_MINT}\"")),
        "USDC_MINT_MAINNET must be {REAL_MINT}, found: {line}"
    );
}

#[test]
fn no_doc_or_source_spells_the_mint_another_way() {
    let stem = &REAL_MINT[..REAL_MINT.len() - 1];
    let files = [
        "src/solana_escrow/mod.rs",
        "../README.md",
        "../docs/mainnet_readiness_runbook.md",
        "../docs/mainnet_deployment_checklist.md",
        "../docs/escrow_protocol.md",
        "../docs/onchain_event_indexing.md",
        "../docs/devnet_e2e_walkthrough.md",
        "../docs/protocol_poc_requirements.md",
    ];
    for rel in files {
        let text = read(rel);
        for (at, _) in text.match_indices(stem) {
            let found = &text[at..(at + REAL_MINT.len()).min(text.len())];
            assert_eq!(
                found, REAL_MINT,
                "{rel} spells the mainnet USDC mint as {found}"
            );
        }
    }
}
