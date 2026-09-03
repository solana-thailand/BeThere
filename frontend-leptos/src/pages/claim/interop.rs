//! Type aliases and JS interop (navigation + Solana wallet adapter).

use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

/// Quiz answers: question_id → selected option text.
pub(super) type QuizAnswers = std::collections::HashMap<String, String>;

// ---------------------------------------------------------------------------
// JS interop
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/clipboard.js")]
extern "C" {
    /// Copy text to the system clipboard.
    ///
    /// Uses the Clipboard API with a textarea fallback for older browsers.
    /// Returns true if the copy operation was initiated successfully.
    #[wasm_bindgen(js_name = "copyToClipboard")]
    pub(super) fn copy_to_clipboard_js(text: &str) -> bool;
}

#[wasm_bindgen(module = "/js/confetti.js")]
extern "C" {
    /// Launch a burst of festive confetti particles across the viewport.
    #[wasm_bindgen(js_name = "launchConfetti")]
    pub(super) fn launch_confetti();
}

// ===== Navigation JS Interop =====
// Uses wasm_bindgen module imports from /js/navigation.js instead of js_sys::eval().
#[wasm_bindgen(module = "/js/navigation.js")]
extern "C" {
    #[wasm_bindgen(js_name = "readClipboardText")]
    pub(super) fn read_clipboard_text_js() -> js_sys::Promise;
}

// ---------------------------------------------------------------------------
// JS interop — Solana wallet adapter (shared with deposit.rs)
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/solana_wallet.js")]
extern "C" {
    #[wasm_bindgen(js_name = "getDetectedWallets")]
    pub(super) fn get_detected_wallets_js() -> Vec<String>;

    #[wasm_bindgen(js_name = "connectWallet")]
    pub(super) fn connect_wallet_js_raw(wallet_name: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = "signAndSendTransaction")]
    pub(super) fn sign_and_send_tx_js_raw(
        wallet_name: &str,
        transaction_b64: &str,
    ) -> js_sys::Promise;

    #[wasm_bindgen(js_name = "isWalletAvailable")]
    pub(super) fn is_wallet_available_js(wallet_name: &str) -> bool;
}

pub(super) async fn connect_wallet_js(wallet_name: &str) -> crate::wallet_error::WalletResult {
    let promise = connect_wallet_js_raw(wallet_name);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[wasm] connect_wallet_js error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}

/// Sign and send a transaction via the wallet adapter.
/// Currently unused after 1C simplification (refund moved to deposit page).
/// Retained for potential future wallet operations on the claim page.
#[allow(dead_code)]
pub(super) async fn sign_and_send_tx_js(
    wallet_name: &str,
    transaction_b64: &str,
) -> crate::wallet_error::WalletResult {
    let promise = sign_and_send_tx_js_raw(wallet_name, transaction_b64);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[wasm] sign_and_send_tx_js error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}
