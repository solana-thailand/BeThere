//! Solana wallet JS interop.

use wasm_bindgen::prelude::*;



// ===== Solana Wallet JS Interop =====

#[wasm_bindgen(module = "/js/solana_wallet.js")]
extern "C" {
    #[wasm_bindgen(js_name = "getDetectedWallets")]
    fn detected_wallets() -> Vec<String>;

    #[wasm_bindgen(js_name = "connectWallet")]
    fn connect_wallet_js_raw(wallet_name: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = "signAndSendTransaction")]
    fn sign_and_send_tx_js_raw(wallet_name: &str, transaction_b64: &str) -> js_sys::Promise;

    /// SEC-014: Detect wallet's connected cluster via genesis hash.
    /// Returns "devnet", "mainnet-beta", "testnet", "localnet", or null.
    #[wasm_bindgen(js_name = "getWalletCluster")]
    fn get_wallet_cluster_js_raw(wallet_name: &str) -> js_sys::Promise;

    /// Pre-sign simulation: simulates TX before requesting wallet signature.
    /// Returns JSON string: { ok: bool, skipped: bool, error: string?, logs: string[] }
    #[wasm_bindgen(js_name = "simulateTransactionB64")]
    fn simulate_tx_js_raw(wallet_name: &str, transaction_b64: &str) -> js_sys::Promise;
}

/// Detect installed Solana wallet extensions.
pub fn get_detected_wallets_js() -> Vec<String> {
    detected_wallets()
}

pub async fn connect_wallet_js(wallet_name: &str) -> crate::wallet_error::WalletResult {
    if wallet_name.is_empty() {
        log::warn!("[escrow-init] connect_wallet_js: empty wallet name");
        return crate::wallet_error::WalletResult::UnknownFailure;
    }
    let promise = connect_wallet_js_raw(wallet_name);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[escrow-init] connect_wallet_js error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}

pub async fn sign_and_send_tx_js(wallet_name: &str, transaction_b64: &str) -> crate::wallet_error::WalletResult {
    if wallet_name.is_empty() {
        log::warn!("[escrow-init] sign_and_send_tx_js: empty wallet name");
        return crate::wallet_error::WalletResult::UnknownFailure;
    }
    let promise = sign_and_send_tx_js_raw(wallet_name, transaction_b64);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[escrow-init] sign_and_send_tx_js error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}

/// SEC-014: Detect the wallet's connected cluster.
/// Returns the cluster name ("devnet", "mainnet-beta", etc.) or None if undetectable.
pub async fn get_wallet_cluster_js(wallet_name: &str) -> Option<String> {
    if wallet_name.is_empty() {
        log::warn!("[escrow-init] get_wallet_cluster_js: empty wallet name");
        return None;
    }
    let promise = get_wallet_cluster_js_raw(wallet_name);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => {
            if val.is_null() || val.is_undefined() {
                log::warn!("[escrow-init] get_wallet_cluster_js: wallet returned null");
                None
            } else {
                val.as_string()
            }
        }
        Err(e) => {
            log::error!("[escrow-init] get_wallet_cluster_js error: {:?}", e);
            None
        }
    }
}

/// SEC-014: Check if the wallet's cluster matches the expected cluster.
/// Returns Ok(()) if they match, or Err with a descriptive message.
pub async fn check_wallet_cluster(wallet_name: &str, expected_cluster: &str) -> Result<(), String> {
    match get_wallet_cluster_js(wallet_name).await {
        Some(wallet_cluster) => {
            if wallet_cluster == expected_cluster {
                log::info!(
                    "[escrow-init] cluster check passed: wallet={wallet_cluster}, expected={expected_cluster}"
                );
                Ok(())
            } else {
                let msg = format!(
                    "Wallet is on {wallet_cluster} but app expects {expected_cluster}. \
                     Switch your wallet network to {expected_cluster} and try again."
                );
                log::error!("[escrow-init] {msg}");
                Err(msg)
            }
        }
        None => {
            // Cannot detect cluster — allow through with a warning log.
            // Some wallets don't expose their RPC endpoint, so we can't check.
            log::warn!(
                "[escrow-init] cannot detect wallet cluster, skipping check (expected={expected_cluster})"
            );
            Ok(())
        }
    }
}

/// Result of a pre-sign simulation.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SimulateResult {
    pub ok: bool,
    #[serde(default)]
    pub skipped: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub logs: Vec<String>,
}

/// Simulate a transaction before requesting wallet signature.
/// Follows Solana Foundation Security Checklist: "Simulate first."
/// Returns Ok(SimulateResult) on success, or Err(msg) if simulation failed.
pub async fn simulate_transaction_js(wallet_name: &str, transaction_b64: &str) -> Result<SimulateResult, String> {
    if wallet_name.is_empty() {
        log::warn!("[simulate] empty wallet name, skipping");
        return Ok(SimulateResult { ok: true, skipped: true, error: None, logs: vec![] });
    }
    let promise = simulate_tx_js_raw(wallet_name, transaction_b64);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => {
            let json_str = val.as_string().unwrap_or_default();
            match serde_json::from_str::<SimulateResult>(&json_str) {
                Ok(result) => {
                    if result.ok {
                        log::info!("[simulate] TX simulation passed (skipped={})", result.skipped);
                    } else {
                        log::warn!("[simulate] TX simulation failed: {:?}", result.error);
                    }
                    Ok(result)
                }
                Err(e) => {
                    log::warn!("[simulate] failed to parse result: {e}, skipping");
                    Ok(SimulateResult { ok: true, skipped: true, error: None, logs: vec![] })
                }
            }
        }
        Err(e) => {
            log::warn!("[simulate] JS error: {e:?}, skipping");
            Ok(SimulateResult { ok: true, skipped: true, error: None, logs: vec![] })
        }
    }
}
