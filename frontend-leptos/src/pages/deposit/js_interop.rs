//! JS interop for the deposit page — wallet, QR, clipboard, file reading.

use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// QR generation + clipboard
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/qr_generate.js")]
extern "C" {
    /// Preload jsQR and QRious libraries from CDN.
    #[wasm_bindgen(js_name = "preloadQrLibraries")]
    fn preload_qr_libraries_js_raw() -> js_sys::Promise;

    /// Copy text to the system clipboard.
    #[wasm_bindgen(js_name = "copyToClipboard")]
    fn copy_to_clipboard_js(text: &str) -> bool;

    /// Generate a QR data URL from a string payload.
    #[wasm_bindgen(js_name = "generateQrDataUrl")]
    fn generate_qr_data_url_js(data: &str, size: u32) -> Option<String>;
}

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------
// Uses wasm_bindgen module imports from /js/navigation.js instead of js_sys::eval().
// This avoids requiring 'unsafe-eval' in the Content-Security-Policy.

#[wasm_bindgen(module = "/js/navigation.js")]
extern "C" {
    #[wasm_bindgen(js_name = "navigateTo")]
    fn navigate_to_js(url: &str);
}

// ---------------------------------------------------------------------------
// PromptPay QR generation
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/promptpay_qr.js")]
extern "C" {
    /// Generate an EMVCo QR string for Thai PromptPay payments.
    #[wasm_bindgen(js_name = "generatePromptPayQr")]
    fn generate_promptpay_qr_js(
        id: &str,
        amount: f64,
        reference: &str,
    ) -> JsValue;
}

// ---------------------------------------------------------------------------
// Download
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/download.js")]
extern "C" {
    /// Download a data URL as a file.
    #[wasm_bindgen(js_name = "downloadDataUrl")]
    fn download_data_url_js(data_url: &str, filename: &str);
}

// ---------------------------------------------------------------------------
// File upload
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/file_upload.js")]
extern "C" {
    /// Read a file from an input element as a base64 data URL.
    #[wasm_bindgen(js_name = "readFileAsDataUrl")]
    fn read_file_as_data_url_js_raw(input_element: &JsValue) -> js_sys::Promise;
}

pub async fn preload_qr_libraries_js() {
    let _ = wasm_bindgen_futures::JsFuture::from(preload_qr_libraries_js_raw()).await;
}

pub fn copy_to_clipboard(text: &str) -> bool {
    copy_to_clipboard_js(text)
}

pub fn generate_qr_data_url(data: &str, size: u32) -> Option<String> {
    generate_qr_data_url_js(data, size)
}

pub fn navigate_to(url: &str) {
    navigate_to_js(url);
}

pub fn generate_promptpay_qr(id: &str, amount: f64, reference: &str) -> JsValue {
    generate_promptpay_qr_js(id, amount, reference)
}

pub fn download_data_url(data_url: &str, filename: &str) {
    download_data_url_js(data_url, filename);
}

pub async fn read_file_as_data_url(input_element: &JsValue) -> Option<String> {
    match wasm_bindgen_futures::JsFuture::from(read_file_as_data_url_js_raw(input_element)).await {
        Ok(val) => val.as_string(),
        Err(_) => None,
    }
}

// ---------------------------------------------------------------------------
// Solana wallet adapter
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/solana_wallet.js")]
extern "C" {
    /// Get list of detected Solana wallet provider names.
    #[wasm_bindgen(js_name = "getDetectedWallets")]
    fn get_detected_wallets_js() -> Vec<String>;

    /// Connect to a Solana wallet by name. Returns Promise<string> (public key).
    #[wasm_bindgen(js_name = "connectWallet")]
    fn connect_wallet_js_raw(wallet_name: &str) -> js_sys::Promise;

    /// Get the currently connected public key for a wallet. Returns Promise<string>.
    #[wasm_bindgen(js_name = "getConnectedPublicKey")]
    fn get_connected_public_key_js_raw(wallet_name: &str) -> js_sys::Promise;

    /// Sign and send a transaction via the wallet. Returns Promise<string> (signature).
    #[wasm_bindgen(js_name = "signAndSendTransaction")]
    fn sign_and_send_tx_js_raw(wallet_name: &str, transaction_b64: &str) -> js_sys::Promise;

    /// Fetch a serialized transaction from a Solana Pay callback URL.
    #[wasm_bindgen(js_name = "fetchTransactionFromCallback")]
    fn fetch_tx_from_callback_js_raw(callback_url: &str) -> js_sys::Promise;

    /// Check if a specific wallet is available.
    #[wasm_bindgen(js_name = "isWalletAvailable")]
    fn is_wallet_available_js(wallet_name: &str) -> bool;
}

pub fn get_detected_wallets() -> Vec<String> {
    get_detected_wallets_js()
}

pub async fn connect_wallet(wallet_name: &str) -> crate::wallet_error::WalletResult {
    let promise = connect_wallet_js_raw(wallet_name);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[wasm] connect_wallet error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}

#[allow(dead_code)]
pub async fn get_connected_public_key(wallet_name: &str) -> Option<String> {
    match wasm_bindgen_futures::JsFuture::from(get_connected_public_key_js_raw(wallet_name)).await {
        Ok(val) => val.as_string(),
        Err(e) => {
            log::error!("[wasm] get_connected_public_key error: {:?}", e);
            None
        }
    }
}

pub async fn sign_and_send_tx(
    wallet_name: &str,
    transaction_b64: &str,
) -> crate::wallet_error::WalletResult {
    let promise = sign_and_send_tx_js_raw(wallet_name, transaction_b64);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[wasm] sign_and_send_tx error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}

pub async fn fetch_tx_from_callback(callback_url: &str) -> Option<String> {
    match wasm_bindgen_futures::JsFuture::from(fetch_tx_from_callback_js_raw(callback_url)).await {
        Ok(val) => val.as_string(),
        Err(e) => {
            log::error!("[wasm] fetch_tx_from_callback error: {:?}", e);
            None
        }
    }
}

#[allow(dead_code)]
pub fn is_wallet_available(wallet_name: &str) -> bool {
    is_wallet_available_js(wallet_name)
}
