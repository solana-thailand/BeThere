//! JS interop: camera QR scanner, QR generation, clipboard, Solana wallet, haptics.

use wasm_bindgen::prelude::*;



// ===== Camera QR Scanner JS Interop =====
// Uses wasm_bindgen module imports from /js/scanner.js instead of js_sys::eval().
// This avoids requiring 'unsafe-eval' in the Content-Security-Policy.
//
// The JS module at frontend-leptos/js/scanner.js provides:
// - startCamera()      — request camera, attach to #scanner-video, start QR loop
// - stopCamera()       — stop camera stream and QR detection
// - checkQrResult()    — poll for detected QR code (string | null)
// - checkCameraError() — poll for camera error message (string | null)
// - isScannerActive()  — check if scanner loop is running (bool)
//
// Rust call sites use snake_case names mapped via #[wasm_bindgen(js_name = ...)].

#[wasm_bindgen(module = "/js/scanner.js")]
extern "C" {
    /// Start the camera and QR scanning loop.
    ///
    /// Requests camera access (rear-facing preferred), waits for the video element
    /// to be both present AND visible in the DOM, streams to `#scanner-video`,
    /// and starts a JS-side loop that polls for QR codes every 300ms.
    ///
    /// Results are stored in `window.__qrResult`; errors in `window.__cameraError`.
    #[wasm_bindgen(js_name = "startCamera")]
    pub(super) fn start_camera_js();

    /// Stop the camera stream and QR scanning loop.
    #[wasm_bindgen(js_name = "stopCamera")]
    pub(super) fn stop_camera_js();

    /// Poll for a detected QR code value. Returns the raw string and clears it.
    #[wasm_bindgen(js_name = "checkQrResult")]
    pub(super) fn check_qr_result_js() -> Option<String>;

    /// Poll for camera errors set by the JS scanning loop.
    #[wasm_bindgen(js_name = "checkCameraError")]
    pub(super) fn check_camera_error_js() -> Option<String>;

    /// Check if the scanner is still active (set by start/stop).
    #[wasm_bindgen(js_name = "isScannerActive")]
    pub(super) fn is_scanner_active_js() -> bool;
}

// ===== QR Code Generation (Rust) =====
// QR codes are generated using the `qrcode` Rust crate.
// No CDN dependency — generates PNG data URLs synchronously.

/// Generate a QR code image as a base64 PNG data URL.
///
/// Returns something like "data:image/png;base64,..." or None if
/// the input is too long for a QR code.
pub(super) fn generate_qr_data_url(text: &str, size: u32) -> Option<String> {
    crate::utils::qr_gen::generate_qr_data_url(text, size)
}

// ===== Clipboard JS Interop =====

#[wasm_bindgen(module = "/js/clipboard.js")]
extern "C" {
    /// Copy text to the system clipboard.
    ///
    /// Uses the Clipboard API with a textarea fallback for older browsers.
    /// Returns true if the copy operation was initiated successfully.
    #[wasm_bindgen(js_name = "copyToClipboard")]
    pub(super) fn copy_to_clipboard_js(text: &str) -> bool;
}

// ===== Solana Wallet JS Interop (for on-chain escrow check-in) =====

#[wasm_bindgen(module = "/js/solana_wallet.js")]
extern "C" {
    /// Get a list of detected Solana wallet adapter names.
    #[wasm_bindgen(js_name = "getDetectedWallets")]
    pub(super) fn get_detected_wallets_js() -> Vec<String>;

    /// Connect to a Solana wallet and return the public key (base58).
    #[wasm_bindgen(js_name = "connectWallet")]
    pub(super) fn connect_wallet_js_raw(wallet_name: &str) -> js_sys::Promise;

    /// Sign and send a base64-encoded serialized transaction.
    #[wasm_bindgen(js_name = "signAndSendTransaction")]
    pub(super) fn sign_and_send_tx_js_raw(wallet_name: &str, transaction_b64: &str) -> js_sys::Promise;
}

/// Async wrapper: connect to a Solana wallet and return the public key (base58).
pub(super) async fn connect_wallet_js(wallet_name: &str) -> crate::wallet_error::WalletResult {
    if wallet_name.is_empty() {
        log::warn!("[wasm] connect_wallet_js: empty wallet name");
        return crate::wallet_error::WalletResult::UnknownFailure;
    }
    let promise = connect_wallet_js_raw(wallet_name);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[wasm] connect_wallet_js error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}

/// Async wrapper: sign and send a base64-encoded serialized transaction.
pub(super) async fn sign_and_send_tx_js(wallet_name: &str, transaction_b64: &str) -> crate::wallet_error::WalletResult {
    if wallet_name.is_empty() {
        log::warn!("[wasm] sign_and_send_tx_js: empty wallet name");
        return crate::wallet_error::WalletResult::UnknownFailure;
    }
    let promise = sign_and_send_tx_js_raw(wallet_name, transaction_b64);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[wasm] sign_and_send_tx_js error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}

// ===== Haptic & Audio Feedback JS Interop =====
// Uses Web Audio API + Vibration API for instant scan feedback.
// The JS module at frontend-leptos/js/feedback.js provides:
// - feedbackSuccess()  — short vibration + high beep
// - feedbackWarning()  — double-pulse vibration + medium tone
// - feedbackError()    — long vibration + low tone
// - enableAudio()      — opt in to audio beeps
// - disableAudio()     — opt out of audio beeps
// - isAudioFeedbackEnabled() — check session preference

#[wasm_bindgen(module = "/js/feedback.js")]
extern "C" {
    #[wasm_bindgen(js_name = "feedbackSuccess")]
    pub(super) fn feedback_success_js();

    #[wasm_bindgen(js_name = "feedbackWarning")]
    pub(super) fn feedback_warning_js();

    #[wasm_bindgen(js_name = "feedbackError")]
    pub(super) fn feedback_error_js();

    #[wasm_bindgen(js_name = "enableAudio")]
    pub(super) fn enable_audio_js();

    #[wasm_bindgen(js_name = "disableAudio")]
    pub(super) fn disable_audio_js();

    #[wasm_bindgen(js_name = "isAudioFeedbackEnabled")]
    pub(super) fn is_audio_enabled_js() -> bool;
}
