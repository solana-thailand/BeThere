//! The sandbox RPC (`SolanaConfig::devnet_rpc_url`): always devnet, with the
//! same key, even where `rpc_url` is the mainnet host prod mints with. A
//! mainnet URL here sent devnet transactions to mainnet (prod, 2026-10-11).

use event_checkin_domain::config::{DEVNET_RPC, SolanaConfig};

fn config(rpc_url: &str, api_key: &str) -> SolanaConfig {
    SolanaConfig {
        rpc_url: rpc_url.into(),
        api_key: api_key.into(),
        crossmint_host: String::new(),
        crossmint_api_key: String::new(),
        crossmint_collection_id: String::new(),
    }
}

#[test]
fn mainnet_host_gets_the_devnet_host_and_keeps_the_key() {
    let c = config("https://mainnet.helius-rpc.com", "k1");
    assert_eq!(
        c.full_rpc_url(),
        "https://mainnet.helius-rpc.com?api-key=k1"
    );
    assert_eq!(c.devnet_rpc_url(), format!("{DEVNET_RPC}?api-key=k1"));
}

#[test]
fn devnet_host_is_kept_as_is() {
    let c = config("https://devnet.helius-rpc.com", "k2");
    assert_eq!(c.devnet_rpc_url(), c.full_rpc_url());
    assert_eq!(
        config("https://devnet.example", "").devnet_rpc_url(),
        "https://devnet.example"
    );
}

#[test]
fn no_key_means_no_query() {
    assert_eq!(
        config("https://mainnet.helius-rpc.com", "").devnet_rpc_url(),
        DEVNET_RPC
    );
}

#[test]
fn sandbox_handler_never_uses_the_cluster_rpc() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../worker/src/handlers/sandbox.rs"
    ))
    .unwrap();
    assert!(
        !src.contains("full_rpc_url"),
        "sandbox must use devnet_rpc_url"
    );
    assert!(src.contains("devnet_rpc_url()"));
}
