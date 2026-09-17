//! `/api/health` network parsing: escrow and badge clusters are cached apart.
//!
//! Prod runs badges on mainnet with escrow on devnet (commit 8265d13); links
//! and the SEC-014 wallet guard must each read the right one.
//!
//! Run: `cargo test --test cluster_networks`

use event_checkin_frontend::utils::{SolanaNetworks, parse_health_networks, solscan_address_url};

fn networks(escrow: &str, nft: &str) -> SolanaNetworks {
    SolanaNetworks {
        escrow: escrow.to_string(),
        nft: nft.to_string(),
    }
}

#[test]
fn prod_split_keeps_escrow_and_badge_clusters_apart() {
    let body = r#"{"cluster":"devnet","solana":{"escrow_cluster":"devnet","nft_cluster":"mainnet-beta","rpc_cluster":"mainnet-beta"}}"#;
    assert_eq!(
        parse_health_networks(body),
        networks("devnet", "mainnet-beta")
    );
}

#[test]
fn mainnet_escrow_is_not_masked_by_the_devnet_fallback() {
    let body = r#"{"cluster":"mainnet-beta","solana":{"nft_cluster":"mainnet-beta"}}"#;
    assert_eq!(
        parse_health_networks(body),
        networks("mainnet-beta", "mainnet-beta")
    );
}

#[test]
fn missing_or_unknown_nft_cluster_follows_escrow() {
    assert_eq!(
        parse_health_networks(r#"{"cluster":"mainnet-beta"}"#),
        networks("mainnet-beta", "mainnet-beta")
    );
    assert_eq!(
        parse_health_networks(r#"{"cluster":"devnet","solana":{"nft_cluster":"unknown"}}"#),
        networks("devnet", "devnet")
    );
}

#[test]
fn failed_fetch_falls_back_to_devnet() {
    assert_eq!(parse_health_networks(""), networks("devnet", "devnet"));
    assert_eq!(
        parse_health_networks("<html>"),
        networks("devnet", "devnet")
    );
}

#[test]
fn badge_wallet_link_on_mainnet_has_no_cluster_param() {
    assert_eq!(
        solscan_address_url("Abc", "mainnet-beta"),
        "https://solscan.io/address/Abc"
    );
}
