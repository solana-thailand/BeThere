//! Deposit page — attendees pay their event deposit (USDC or THB).
//!
//! Public page (no auth required) accessed via `/deposit/:attendee_id?event_id=xxx`.
//! Flow:
//! 1. Extract attendee_id from URL path, event_id from query params
//! 2. GET /api/deposit/status/{attendee_id}?event_id={event_id}
//! 3. If deposit not enabled → show message
//! 4. If already deposited → show status
//! 5. If not deposited → show dual-track payment options (USDC / THB)
//! 6. USDC (wallet adapter): connect wallet → fetch TX → sign & send → poll confirmation
//! 7. USDC (QR fallback): generate Solana Pay QR → wallet scans → poll confirmation
//! 8. THB: text input for slip URL (MVP) → calls upload_thb_slip()

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;

use crate::api::{
    self, CloseDepositRequest, ConfirmDepositResponse, DepositMethod, DepositStatusResponse, RefundTxRequest,
    ThbSlipUploadRequest, UsdcDepositRequest,
};

// Thai banks (2c2p payout codes)
const THAI_BANKS: &[(&str, &str)] = &[
    ("002", "Bangkok Bank (BBL)"),
    ("004", "Kasikornbank (KBANK)"),
    ("006", "Krung Thai Bank (KTB)"),
    ("011", "TMB Bank (TTB)"),
    ("014", "Siam Commercial Bank (SCB)"),
    ("022", "CIMB Thai Bank (CIMB)"),
    ("024", "United Overseas Bank Thai (UOBT)"),
    ("025", "Bank of Ayudhya (BAY)"),
    ("065", "Thanachart Bank"),
    ("066", "Islamic Bank of Thailand"),
    ("067", "Tisco Bank"),
    ("069", "Kiatnakin Bank (KK)"),
    ("070", "ICBC Thai"),
    ("071", "Thai Credit Retail Bank (TCRB)"),
    ("073", "Land and Houses Bank (LHBANK)"),
    ("030", "Government Saving Bank (GSB)"),
    ("033", "Government Housing Bank (GHB)"),
    ("034", "Bank for Agriculture (BAAC)"),
];
use crate::components::{self, Toast, ToastType};
use crate::icons::{Icon, IconName, wallet_icon_name};
use crate::utils::{format_timestamp, solscan_tx_url, get_cluster};
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// JS interop — QR generation + clipboard
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/qr_generate.js")]
extern "C" {
    /// Preload jsQR and QRious libraries from CDN.
    /// Call on mount to ensure QR generation is ready when needed.
    #[wasm_bindgen(js_name = "preloadQrLibraries")]
    fn preload_qr_libraries_js_raw() -> js_sys::Promise;

    /// Copy text to the system clipboard.
    #[wasm_bindgen(js_name = "copyToClipboard")]
    fn copy_to_clipboard_js(text: &str) -> bool;

    /// Generate a QR code as a base64 PNG data URL.
    /// Requires QRious to be preloaded — call preload_qr_libraries_js() on mount.
    #[wasm_bindgen(js_name = "generateQrDataUrl")]
    fn generate_qr_data_url_js(text: &str, size: u32) -> Option<String>;
}

// ===== Navigation JS Interop =====
// Uses wasm_bindgen module imports from /js/navigation.js instead of js_sys::eval().
// This avoids requiring 'unsafe-eval' in the Content-Security-Policy.
#[wasm_bindgen(module = "/js/navigation.js")]
extern "C" {
    fn navigateTo(path: &str);
}

// ---------------------------------------------------------------------------
// JS interop — PromptPay QR generation
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/promptpay_qr.js")]
extern "C" {
    /// Generate an EMVCo QR string for Thai PromptPay payments.
    /// `reference` is an optional note shown in the banking app (EMVCo Tag 62 sub-tag 01).
    #[wasm_bindgen(js_name = "generatePromptPayQr")]
    fn generate_promptpay_qr_js(promptpay_id: &str, amount: f64, reference: &str) -> Option<String>;
}

#[wasm_bindgen(module = "/js/download.js")]
extern "C" {
    /// Download a data URL as a file.
    #[wasm_bindgen(js_name = "downloadDataUrl")]
    fn download_data_url(data_url: &str, filename: &str);
}

// ---------------------------------------------------------------------------
// JS interop — File upload
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/file_upload.js")]
extern "C" {
    /// Read a file from an input element as a base64 data URL.
    #[wasm_bindgen(js_name = "readFileAsDataUrl")]
    fn read_file_as_data_url_js_raw(input: &wasm_bindgen::JsValue) -> js_sys::Promise;
}

// ---------------------------------------------------------------------------
// JS interop — Solana wallet adapter
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/solana_wallet.js")]
extern "C" {
    /// Get a list of detected Solana wallet adapter names.
    #[wasm_bindgen(js_name = "getDetectedWallets")]
    fn get_detected_wallets_js() -> Vec<String>;

    /// Connect to a Solana wallet and return the public key (base58).
    #[wasm_bindgen(js_name = "connectWallet")]
    fn connect_wallet_js_raw(wallet_name: &str) -> js_sys::Promise;

    /// Get the currently connected wallet's public key (base58) without prompting.
    #[wasm_bindgen(js_name = "getConnectedPublicKey")]
    fn get_connected_public_key_js_raw(wallet_name: &str) -> js_sys::Promise;

    /// Sign and send a base64-encoded serialized transaction.
    #[wasm_bindgen(js_name = "signAndSendTransaction")]
    fn sign_and_send_tx_js_raw(wallet_name: &str, transaction_b64: &str) -> js_sys::Promise;

    /// Fetch the serialized deposit transaction from the Solana Pay callback URL.
    #[wasm_bindgen(js_name = "fetchTransactionFromCallback")]
    fn fetch_tx_from_callback_js_raw(callback_url: &str) -> js_sys::Promise;

    /// Check if a wallet provider is available.
    #[wasm_bindgen(js_name = "isWalletAvailable")]
    fn is_wallet_available_js(wallet_name: &str) -> bool;
}

// ---------------------------------------------------------------------------
// Async wrappers — bridge js_sys::Promise → Rust Future
// ---------------------------------------------------------------------------

/// Preload jsQR and QRious libraries from CDN.
async fn preload_qr_libraries_js() {
    let promise = preload_qr_libraries_js_raw();
    if let Err(e) = wasm_bindgen_futures::JsFuture::from(promise).await {
        log::error!("[wasm] preload_qr_libraries_js error: {:?}", e);
    }
}

/// Read a file from an input element as a base64 data URL.
async fn read_file_as_data_url_js(input: &wasm_bindgen::JsValue) -> Option<String> {
    let promise = read_file_as_data_url_js_raw(input);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => {
            if val.is_null() || val.is_undefined() {
                None
            } else {
                val.as_string()
            }
        }
        Err(e) => {
            log::error!("[wasm] read_file_as_data_url_js error: {:?}", e);
            None
        }
    }
}

/// Connect to a Solana wallet and return the public key (base58).
async fn connect_wallet_js(wallet_name: &str) -> crate::wallet_error::WalletResult {
    let promise = connect_wallet_js_raw(wallet_name);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[wasm] connect_wallet_js error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}

/// Get the currently connected wallet's public key (base58) without prompting.
async fn _get_connected_public_key_js(wallet_name: &str) -> Option<String> {
    let promise = get_connected_public_key_js_raw(wallet_name);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => {
            if val.is_null() || val.is_undefined() {
                None
            } else {
                val.as_string()
            }
        }
        Err(e) => {
            log::error!("[wasm] get_connected_public_key_js error: {:?}", e);
            None
        }
    }
}

/// Sign and send a base64-encoded serialized transaction.
async fn sign_and_send_tx_js(wallet_name: &str, transaction_b64: &str) -> crate::wallet_error::WalletResult {
    let promise = sign_and_send_tx_js_raw(wallet_name, transaction_b64);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[wasm] sign_and_send_tx_js error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}

/// Fetch the serialized deposit transaction from the Solana Pay callback URL.
async fn fetch_tx_from_callback_js(callback_url: &str) -> Option<String> {
    let promise = fetch_tx_from_callback_js_raw(callback_url);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => {
            if val.is_null() || val.is_undefined() {
                None
            } else {
                val.as_string()
            }
        }
        Err(e) => {
            log::error!("[wasm] fetch_tx_from_callback_js error: {:?}", e);
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Refund deadline helpers (behavioral economics: loss aversion framing)
// ---------------------------------------------------------------------------

/// Format epoch ms to a short readable date for the refund deadline.
fn format_refund_deadline(ms: i64) -> String {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    let month = date.get_month() + 1;
    let day = date.get_date();
    let hours = date.get_hours();
    let minutes = date.get_minutes();
    format!("{:02}/{:02} {:02}:{:02}", month, day, hours, minutes)
}

/// Format hours into a human-friendly duration label (e.g. "7 days", "3d 12h").
/// Extract event context (name, tagline) from the current page state.
/// Returns None for Loading/Error/NotEnabled/ThbUploaded states.
fn extract_event_context(state: &DepositPageState) -> Option<(String, String)> {
    let data: &DepositStatusResponse = match state {
        DepositPageState::AlreadyDeposited(d)
        | DepositPageState::ChoosePayment(d)
        | DepositPageState::WalletConnected(d, _, _)
        | DepositPageState::AwaitingConfirmation(d, _, _)
        | DepositPageState::DepositConfirmed(d, _)
        | DepositPageState::UsdcQrReady(d, _)
        | DepositPageState::ThbUploading(d)
        | DepositPageState::RefundChooseWallet(d)
        | DepositPageState::RefundWalletConnected(d, _, _)
        | DepositPageState::RefundSigning(d, _, _)
        | DepositPageState::RefundConfirmed(d, _)
        | DepositPageState::CloseDepositChooseWallet(d)
        | DepositPageState::CloseDepositWalletConnected(d, _, _)
        | DepositPageState::CloseDepositSigning(d, _, _)
        | DepositPageState::CloseDepositConfirmed(d, _) => d,
        _ => return None,
    };
    if data.event_name.is_empty() {
        None
    } else {
        Some((data.event_name.clone(), data.event_tagline.clone()))
    }
}

fn format_duration_label(hours: u32) -> String {
    if hours >= 24 {
        let days = hours / 24;
        let remaining = hours % 24;
        if remaining == 0 {
            if days == 1 { "1 day".to_string() } else { format!("{days} days") }
        } else {
            format!("{days}d {remaining}h")
        }
    } else {
        format!("{hours}h")
    }
}

// ---------------------------------------------------------------------------
// Route params
// ---------------------------------------------------------------------------

/// Route parameters for `/deposit/:attendee_id`.
#[derive(Params, PartialEq, Clone)]
struct DepositParams {
    attendee_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Page state
// ---------------------------------------------------------------------------

/// Top-level state machine for the deposit page flow.
#[derive(Clone, Debug)]
enum DepositPageState {
    /// Loading deposit status from backend.
    Loading,
    /// API or param error.
    Error(String),
    /// Deposits not enabled for this event.
    NotEnabled,
    /// Deposit already completed.
    AlreadyDeposited(DepositStatusResponse),
    /// Ready to choose payment method.
    ChoosePayment(DepositStatusResponse),
    /// Wallet connected — ready to send TX.
    WalletConnected(DepositStatusResponse, String, String),
    // (deposit_data, wallet_name, public_key)
    /// TX sent — polling for on-chain confirmation.
    AwaitingConfirmation(DepositStatusResponse, String, String),
    // (deposit_data, wallet_name, tx_signature)
    /// Deposit confirmed on-chain.
    DepositConfirmed(DepositStatusResponse, String),
    // (deposit_data, tx_signature)
    /// USDC QR URL generated and ready to display (QR fallback for mobile).
    UsdcQrReady(DepositStatusResponse, String),
    /// THB slip is being uploaded.
    #[allow(dead_code)]
    ThbUploading(DepositStatusResponse),
    /// THB slip uploaded successfully.
    ThbUploaded(String, String, String), // (attendee_id, event_id, event_slug)
    /// Refund flow — choosing wallet to connect.
    RefundChooseWallet(DepositStatusResponse),
    /// Refund flow — wallet connected, ready to claim.
    RefundWalletConnected(DepositStatusResponse, String, String),
    // (deposit_data, wallet_name, public_key)
    /// Refund flow — signing and sending refund TX.
    RefundSigning(DepositStatusResponse, String, String),
    // (deposit_data, wallet_name, public_key)
    /// Refund flow — TX confirmed on-chain.
    RefundConfirmed(DepositStatusResponse, String),
    // (deposit_data, tx_signature)
    /// Close deposit — choosing wallet to connect.
    CloseDepositChooseWallet(DepositStatusResponse),
    /// Close deposit — wallet connected, ready to close.
    CloseDepositWalletConnected(DepositStatusResponse, String, String),
    // (deposit_data, wallet_name, public_key)
    /// Close deposit — signing TX.
    CloseDepositSigning(DepositStatusResponse, String, String),
    // (deposit_data, wallet_name, public_key)
    /// Close deposit — confirmed.
    CloseDepositConfirmed(DepositStatusResponse, String),
    // (deposit_data, tx_signature)
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// Deposit page component.
///
/// Public page where attendees pay their event deposit via USDC (Solana Pay)
/// or THB (PromptPay slip upload).
#[component]
pub fn Deposit() -> impl IntoView {
    let params = use_params::<DepositParams>();

    // Auth state — check if user is signed in (for logout button visibility)
    let (signed_in_email, set_signed_in_email) = signal(None::<String>);
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            let window = web_sys::window().expect("no window");
            let origin = window
                .location()
                .origin()
                .unwrap_or_else(|_| "http://localhost:8787".to_string());
            let url = format!("{origin}/api/auth/me");
            if let Ok(resp) = gloo::net::http::Request::get(&url).send().await {
                if resp.status() == 200 {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        if let Some(email) = data["data"]["email"].as_str() {
                            if !email.is_empty() {
                                set_signed_in_email.set(Some(email.to_string()));
                            }
                        }
                    }
                }
            }
        });
    });

    // Reactive state
    let (state, set_state) = signal(DepositPageState::Loading);
    let (toast, set_toast) = signal(None::<components::ToastMessage>);
    let (slip_url_input, set_slip_url_input) = signal(String::new());
    let (wallet_input, set_wallet_input) = signal(String::new());
    let (pay_url_copied, set_pay_url_copied) = signal(false);
    // Bank info signals for THB refund
    let (bank_account_input, set_bank_account_input) = signal(String::new());
    let (bank_name_input, set_bank_name_input) = signal(String::new());
    let (account_name_input, set_account_name_input) = signal(String::new());
    let (show_bank_dropdown, set_show_bank_dropdown) = signal(false);

    // File input ref for slip image upload
    let file_input_ref = NodeRef::<leptos::html::Input>::new();

    // Track QR library loading state so QR rendering can retry.
    let (qr_ready, set_qr_ready) = signal(false);

    // Preload jsQR + QRious libraries on mount.
    // The deposit page renders PromptPay/USDC payment QR codes,
    // so libraries should be loaded before the payment view appears.
    leptos::task::spawn_local(async move {
        preload_qr_libraries_js().await;
        set_qr_ready.set(true);
        log::info!("[deposit] QR libraries loaded, qr_ready = true");
    });

    // Extract attendee_id from URL path and event_id from query params, then fetch status
    Effect::new(move |_| {
        let attendee_id = match params.get() {
            Ok(p) => p.attendee_id.unwrap_or_default(),
            Err(_) => {
                set_state.set(DepositPageState::Error(
                    "Invalid deposit link — missing attendee ID.".to_string(),
                ));
                return;
            }
        };

        if attendee_id.is_empty() {
            set_state.set(DepositPageState::Error(
                "Invalid deposit link — missing attendee ID.".to_string(),
            ));
            return;
        }

        // Parse event_id from query params: ?event_id=xxx
        let event_id = web_sys::Url::new(
            &web_sys::window()
                .unwrap()
                .location()
                .href()
                .unwrap(),
        )
        .ok()
        .and_then(|url| url.search_params().get("event_id"));

        let event_id_clone = event_id.clone();
        leptos::task::spawn_local(async move {
            match api::get_deposit_status(&attendee_id, event_id.as_deref()).await {
                Ok(data) => {
                    if !data.deposit_enabled {
                        set_state.set(DepositPageState::NotEnabled);
                    } else if data.status.is_some() {
                        set_state.set(DepositPageState::AlreadyDeposited(data));
                    } else {
                        set_state.set(DepositPageState::ChoosePayment(data));
                    }
                }
                Err(e) => {
                    log::error!("[deposit] failed to get status: {e}");
                    set_state.set(DepositPageState::Error(format!(
                        "Failed to load deposit info: {e}"
                    )));
                }
            }
        });

        // Store event_id for later use
        if let Some(eid) = event_id_clone {
            // Provide via context or just hold in the closure — we'll use it
            // in the handlers below via cloning the attendee_id + event_id
            let _ = eid; // available for child spawns
        }
    });

    // --- Detect installed wallets on mount (poll sync with delays for late injection) ---
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());
    let has_wallets = move || !detected_wallets.get().is_empty();
    {
        let set_dw = set_detected_wallets;
        leptos::task::spawn_local(async move {
            let mut wallets = get_detected_wallets_js();
            if wallets.is_empty() {
                for _ in 0..10 {
                    gloo::timers::future::TimeoutFuture::new(300).await;
                    wallets = get_detected_wallets_js();
                    if !wallets.is_empty() {
                        break;
                    }
                }
            }
            log::info!("[deposit] detected wallets: {:?}", wallets);
            set_dw.set(wallets);
        });
    }

    // --- Connect Wallet handler ---
    let handle_connect_wallet = move |wallet_name: String| {
        let deposit_data = match &state.get() {
            DepositPageState::ChoosePayment(d) => d.clone(),
            _ => return,
        };

        let wallet_name_clone = wallet_name.clone();
        let deposit_data_for_state = deposit_data.clone();
        leptos::task::spawn_local(async move {
            match connect_wallet_js(&wallet_name_clone).await {
                crate::wallet_error::WalletResult::Success(pubkey) => {
                    log::info!("[deposit] wallet connected: {} ({})", wallet_name_clone, pubkey);
                    set_state.set(DepositPageState::WalletConnected(
                        deposit_data_for_state,
                        wallet_name_clone,
                        pubkey,
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!("[deposit] wallet connect error: code={:?} msg={}", e.code, e.raw_message);
                    components::show_toast(
                        &set_toast,
                        &crate::wallet_error::user_friendly_message(&e),
                        ToastType::Error,
                    );
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    components::show_toast(
                        &set_toast,
                        "Failed to connect wallet. Please try again.",
                        ToastType::Error,
                    );
                }
            }
        });
    };

    // --- Send Deposit TX (via connected wallet) ---
    let handle_send_deposit = move |wallet_name: String, public_key: String| {
        let current_state = state.get();
        let deposit_data = match &current_state {
            DepositPageState::WalletConnected(d, _, _) => d.clone(),
            _ => return,
        };

        // Re-extract attendee_id and event_id from URL
        let attendee_id = match params.get() {
            Ok(p) => p.attendee_id.unwrap_or_default(),
            Err(_) => return,
        };
        let event_id = web_sys::Url::new(
            &web_sys::window()
                .unwrap()
                .location()
                .href()
                .unwrap(),
        )
        .ok()
        .and_then(|url| url.search_params().get("event_id"));

        let pk_for_api = public_key.clone();
        let wallet_name_for_tx = wallet_name.clone();
        let deposit_data_for_state = deposit_data.clone();
        let event_id_str = event_id.unwrap_or_default();

        leptos::task::spawn_local(async move {
            // Step 1: Initiate deposit with backend (records pending status + gets callback URL)
            let body = UsdcDepositRequest {
                event_id: event_id_str.clone(),
                attendee_id: attendee_id.clone(),
                wallet_address: pk_for_api,
            };
            let deposit_resp = match api::deposit_usdc(&body).await {
                Ok(r) => r,
                Err(e) => {
                    log::error!("[deposit] USDC deposit initiate failed: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to initiate deposit: {e}"),
                        ToastType::Error,
                    );
                    return;
                }
            };

            // Step 2: Extract the callback URL from the Solana Pay URL
            // solana_pay_url format: "solana:https://...callback_url"
            let callback_url = if deposit_resp.solana_pay_url.starts_with("solana:") {
                &deposit_resp.solana_pay_url[7..]
            } else {
                &deposit_resp.solana_pay_url
            };

            // Step 3: Fetch the serialized TX from the callback
            let tx_b64 = match fetch_tx_from_callback_js(callback_url).await {
                Some(tx) => tx,
                None => {
                    log::error!("[deposit] failed to fetch TX from callback");
                    components::show_toast(
                        &set_toast,
                        "Failed to build deposit transaction. Please try again.",
                        ToastType::Error,
                    );
                    return;
                }
            };

            // SEC-014: Verify wallet cluster matches expected network.
            let expected_cluster = crate::utils::get_cluster();
            if let Err(cluster_err) = crate::pages::escrow_init::check_wallet_cluster(&wallet_name_for_tx, &expected_cluster).await {
                log::error!("[deposit] cluster mismatch: {cluster_err}");
                components::show_toast(
                    &set_toast,
                    &cluster_err,
                    ToastType::Error,
                );
                return;
            }

            // Pre-sign simulation (Solana Foundation Security Checklist).
            match crate::pages::escrow_init::simulate_transaction_js(&wallet_name_for_tx, &tx_b64).await {
                Ok(sim) if sim.ok => {}
                Ok(sim) => {
                    let err_msg = sim.error.unwrap_or_else(|| "Simulation failed".to_string());
                    log::error!("[deposit] simulation failed: {err_msg}");
                    components::show_toast(&set_toast, &format!("Transaction would fail: {err_msg}"), ToastType::Error);
                    return;
                }
                Err(e) => { log::warn!("[deposit] simulate error (not blocking): {e}"); }
            }

            // Step 4: Sign and send the TX via the wallet
            match sign_and_send_tx_js(&wallet_name_for_tx, &tx_b64).await {
                crate::wallet_error::WalletResult::Success(signature) => {
                    log::info!("[deposit] TX sent, signature: {}", signature);

                    // Step 5: Record the TX signature with the backend
                    let _ = api::record_deposit_tx(
                        &event_id_str,
                        &attendee_id,
                        &signature,
                    )
                    .await;

                    // Step 6: Start polling for confirmation
                    set_state.set(DepositPageState::AwaitingConfirmation(
                        deposit_data_for_state.clone(),
                        wallet_name_for_tx.clone(),
                        signature.clone(),
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!("[deposit] wallet sign+send error: code={:?} msg={}", e.code, e.raw_message);
                    components::show_toast(
                        &set_toast,
                        &crate::wallet_error::user_friendly_message(&e),
                        ToastType::Error,
                    );
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    log::error!("[deposit] wallet sign+send failed");
                    components::show_toast(
                        &set_toast,
                        "Transaction failed. Please try again.",
                        ToastType::Error,
                    );
                }
            }
        });
    };

    // --- Deposit Confirmation Polling ---
    let handle_poll_confirmation =
        move |event_id: String, attendee_id: String, _tx_sig: String| {
            let set_state = set_state;
            let set_toast = set_toast;
            leptos::task::spawn_local(async move {
                // Poll the confirmation endpoint
                let mut attempts = 0u32;
                let max_attempts = 30; // 30 * 2s = 60s max
                let deposit_data_for_state = match &state.get() {
                    DepositPageState::AwaitingConfirmation(d, _, _) => d.clone(),
                    _ => return,
                };

                while attempts < max_attempts {
                    match api::confirm_deposit(&event_id, &attendee_id).await {
                        Ok(ConfirmDepositResponse {
                            confirmed: true,
                            tx_signature: Some(sig),
                            ..
                        }) => {
                            log::info!("[deposit] confirmed on-chain: {}", sig);
                            set_state.set(DepositPageState::DepositConfirmed(
                                deposit_data_for_state.clone(),
                                sig,
                            ));
                            return;
                        }
                        Ok(_) => {
                            // Not yet confirmed, keep polling
                            attempts += 1;
                            if attempts < max_attempts {
                                gloo::timers::future::TimeoutFuture::new(2000).await;
                            }
                        }
                        Err(e) => {
                            log::warn!("[deposit] confirmation poll error: {e}");
                            attempts += 1;
                            if attempts < max_attempts {
                                gloo::timers::future::TimeoutFuture::new(3000).await;
                            }
                        }
                    }
                }

                // Timeout — show the signature for manual checking
                components::show_toast(
                    &set_toast,
                    "Confirmation is taking longer than expected. Your deposit may still be processing.",
                    ToastType::Warning,
                );
            });
        };

    // --- USDC QR Fallback handler (for mobile / no wallet browser) ---
    let handle_pay_usdc_qr = move || {
        let current_state = state.get();
        let (deposit_data, attendee_id, event_id) = match &current_state {
            DepositPageState::ChoosePayment(d) => {
                let aid = match params.get() {
                    Ok(p) => p.attendee_id.unwrap_or_default(),
                    Err(_) => return,
                };
                let eid = web_sys::Url::new(
                    &web_sys::window()
                        .unwrap()
                        .location()
                        .href()
                        .unwrap(),
                )
                .ok()
                .and_then(|url| url.search_params().get("event_id"));
                (d.clone(), aid, eid)
            }
            _ => return,
        };

        let wallet = wallet_input.get();
        if wallet.trim().is_empty() {
            components::show_toast(
                &set_toast,
                "Please enter your Solana wallet address.",
                ToastType::Warning,
            );
            return;
        }

        let deposit_data_for_set = deposit_data.clone();
        leptos::task::spawn_local(async move {
            let body = UsdcDepositRequest {
                event_id: event_id.unwrap_or_default(),
                attendee_id,
                wallet_address: wallet,
            };
            match api::deposit_usdc(&body).await {
                Ok(resp) => {
                    log::info!("[deposit] USDC QR deposit initiated");
                    set_state.set(DepositPageState::UsdcQrReady(
                        deposit_data_for_set,
                        resp.solana_pay_url,
                    ));
                }
                Err(e) => {
                    log::error!("[deposit] USDC deposit failed: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to initiate USDC payment: {e}"),
                        ToastType::Error,
                    );
                }
            }
        });
    };

    // --- THB Slip Upload handler ---
    let handle_upload_slip = move || {
        let current_state = state.get();
        let (deposit_data, attendee_id, event_id) = match &current_state {
            DepositPageState::ChoosePayment(d) => {
                let aid = match params.get() {
                    Ok(p) => p.attendee_id.unwrap_or_default(),
                    Err(_) => return,
                };
                let eid = web_sys::Url::new(
                    &web_sys::window()
                        .unwrap()
                        .location()
                        .href()
                        .unwrap(),
                )
                .ok()
                .and_then(|url| url.search_params().get("event_id"));
                (d.clone(), aid, eid)
            }
            _ => return,
        };

        // Transition to uploading state
        let deposit_data_for_err = deposit_data.clone();
        let deposit_data_slug = deposit_data.event_slug.clone();
        set_state.set(DepositPageState::ThbUploading(deposit_data));

        let file_ref = file_input_ref.clone();
        let text_slip_url = slip_url_input.get();
        let bank_account_input_for_upload = bank_account_input.get();
        let bank_name_input_for_upload = bank_name_input.get();
        let account_name_input_for_upload = account_name_input.get();
        let set_state = set_state;
        let set_toast = set_toast;
        let params = params.clone();

        leptos::task::spawn_local(async move {
            // Try file upload first, then fall back to text input
            let slip_url = match file_ref.get() {
                Some(el) => {
                    let js_val: wasm_bindgen::JsValue = el.into();
                    match read_file_as_data_url_js(&js_val).await {
                        Some(data_url) => data_url,
                        None => {
                            // No file selected — try text input
                            if text_slip_url.trim().is_empty() {
                                set_state.set(DepositPageState::ChoosePayment(deposit_data_for_err));
                                components::show_toast(
                                    &set_toast,
                                    "Please select a slip image or paste a URL.",
                                    ToastType::Warning,
                                );
                                return;
                            }
                            text_slip_url
                        }
                    }
                }
                None => {
                    if text_slip_url.trim().is_empty() {
                        set_state.set(DepositPageState::ChoosePayment(deposit_data_for_err));
                        components::show_toast(
                            &set_toast,
                            "Please select a slip image or paste a URL.",
                            ToastType::Warning,
                        );
                        return;
                    }
                    text_slip_url
                }
            };

            let body = ThbSlipUploadRequest {
                event_id: event_id.unwrap_or_default(),
                attendee_id,
                slip_url,
                bank_account: {
                    let v = bank_account_input_for_upload.trim().to_string();
                    if v.is_empty() { None } else { Some(v) }
                },
                bank_name: {
                    let v = bank_name_input_for_upload.trim().to_string();
                    if v.is_empty() { None } else { Some(v) }
                },
                account_name: {
                    let v = account_name_input_for_upload.trim().to_string();
                    if v.is_empty() { None } else { Some(v) }
                },
            };
            match api::upload_thb_slip(&body).await {
                Ok(_resp) => {
                    log::info!("[deposit] THB slip uploaded successfully");
                    // Extract attendee_id and event_id for redirect to ticket page
                    let aid = match params.get() {
                        Ok(p) => p.attendee_id.unwrap_or_default(),
                        Err(_) => String::new(),
                    };
                    let eid = web_sys::Url::new(
                        &web_sys::window()
                            .unwrap()
                            .location()
                            .href()
                            .unwrap(),
                    )
                    .ok()
                    .and_then(|url| url.search_params().get("event_id"))
                    .unwrap_or_default();
                    set_state.set(DepositPageState::ThbUploaded(aid, eid, deposit_data_slug));
                }
                Err(e) => {
                    log::error!("[deposit] THB slip upload failed: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to upload slip: {e}"),
                        ToastType::Error,
                    );
                    let aid = match params.get() {
                        Ok(p) => p.attendee_id.unwrap_or_default(),
                        Err(_) => return,
                    };
                    let eid = web_sys::Url::new(
                        &web_sys::window()
                            .unwrap()
                            .location()
                            .href()
                            .unwrap(),
                    )
                    .ok()
                    .and_then(|url| url.search_params().get("event_id"));
                    match api::get_deposit_status(&aid, eid.as_deref()).await {
                        Ok(data) => {
                            set_state.set(DepositPageState::ChoosePayment(data));
                        }
                        Err(_) => {
                            set_state.set(DepositPageState::Error(
                                "Failed to reload deposit status.".to_string(),
                            ));
                        }
                    }
                }
            }
        });
    };

    // --- Copy Solana Pay URL handler ---
    let handle_copy_url = move |url: String| {
        if copy_to_clipboard_js(&url) {
            set_pay_url_copied.set(true);
            components::show_toast(&set_toast, "Payment link copied!", ToastType::Success);
            set_timeout(
                move || set_pay_url_copied.set(false),
                std::time::Duration::from_secs(3),
            );
        } else {
            components::show_toast(
                &set_toast,
                "Failed to copy. Please copy the link manually.",
                ToastType::Error,
            );
        }
    };

    // --- Refund: Connect wallet for refund ---
    let handle_refund_connect_wallet = move |wallet_name: String| {
        let deposit_data = match &state.get() {
            DepositPageState::RefundChooseWallet(d) => d.clone(),
            _ => return,
        };

        let wallet_name_clone = wallet_name.clone();
        let deposit_data_for_state = deposit_data.clone();
        leptos::task::spawn_local(async move {
            match connect_wallet_js(&wallet_name_clone).await {
                crate::wallet_error::WalletResult::Success(pubkey) => {
                    log::info!("[deposit] refund wallet connected: {} ({})", wallet_name_clone, pubkey);
                    set_state.set(DepositPageState::RefundWalletConnected(
                        deposit_data_for_state,
                        wallet_name_clone,
                        pubkey,
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!("[deposit] refund wallet connect error: code={:?} msg={}", e.code, e.raw_message);
                    components::show_toast(
                        &set_toast,
                        &crate::wallet_error::user_friendly_message(&e),
                        ToastType::Error,
                    );
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    components::show_toast(
                        &set_toast,
                        "Failed to connect wallet. Please try again.",
                        ToastType::Error,
                    );
                }
            }
        });
    };

    // --- Refund: Claim refund (sign & send refund TX) ---
    let handle_claim_refund = move |wallet_name: String, public_key: String| {
        let current_state = state.get();
        let deposit_data = match &current_state {
            DepositPageState::RefundWalletConnected(d, _, _) => d.clone(),
            _ => return,
        };

        // Extract attendee_id and event_id from URL
        let attendee_id = match params.get() {
            Ok(p) => p.attendee_id.unwrap_or_default(),
            Err(_) => return,
        };
        let event_id = web_sys::Url::new(
            &web_sys::window()
                .unwrap()
                .location()
                .href()
                .unwrap(),
        )
        .ok()
        .and_then(|url| url.search_params().get("event_id"))
        .unwrap_or_default();

        let wallet_name_for_tx = wallet_name.clone();
        let pk_for_tx = public_key.clone();
        let deposit_data_for_state = deposit_data.clone();

        // Transition to signing state
        set_state.set(DepositPageState::RefundSigning(
            deposit_data.clone(),
            wallet_name.clone(),
            public_key.clone(),
        ));

        leptos::task::spawn_local(async move {
            // Step 1: Request refund TX from backend
            let body = RefundTxRequest {
                event_id: event_id.clone(),
                attendee_id: attendee_id.clone(),
                wallet_address: pk_for_tx.clone(),
            };
            let refund_resp = match api::build_refund_tx(&body).await {
                Ok(r) => r,
                Err(e) => {
                    log::error!("[deposit] refund TX build failed: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to build refund transaction: {e}"),
                        ToastType::Error,
                    );
                    set_state.set(DepositPageState::RefundWalletConnected(
                        deposit_data_for_state,
                        wallet_name_for_tx,
                        pk_for_tx,
                    ));
                    return;
                }
            };

            let tx_b64 = refund_resp.transaction;

            if tx_b64.is_empty() {
                log::error!("[deposit] refund TX is empty");
                components::show_toast(
                    &set_toast,
                    "Refund transaction was empty. Please try again later.",
                    ToastType::Error,
                );
                set_state.set(DepositPageState::RefundWalletConnected(
                    deposit_data,
                    wallet_name_for_tx,
                    pk_for_tx,
                ));
                return;
            }

            // SEC-014: Verify wallet cluster matches expected network.
            let expected_cluster = crate::utils::get_cluster();
            if let Err(cluster_err) = crate::pages::escrow_init::check_wallet_cluster(&wallet_name_for_tx, &expected_cluster).await {
                log::error!("[deposit] cluster mismatch (refund): {cluster_err}");
                components::show_toast(
                    &set_toast,
                    &cluster_err,
                    ToastType::Error,
                );
                return;
            }

            // Pre-sign simulation.
            match crate::pages::escrow_init::simulate_transaction_js(&wallet_name_for_tx, &tx_b64).await {
                Ok(sim) if sim.ok => {}
                Ok(sim) => {
                    let err_msg = sim.error.unwrap_or_else(|| "Simulation failed".to_string());
                    log::error!("[deposit] refund simulation failed: {err_msg}");
                    components::show_toast(&set_toast, &format!("Transaction would fail: {err_msg}"), ToastType::Error);
                    return;
                }
                Err(e) => { log::warn!("[deposit] simulate error (not blocking): {e}"); }
            }

            // Step 2: Sign and send via wallet
            match sign_and_send_tx_js(&wallet_name_for_tx, &tx_b64).await {
                crate::wallet_error::WalletResult::Success(signature) => {
                    log::info!("[deposit] refund TX sent, signature: {}", signature);
                    set_state.set(DepositPageState::RefundConfirmed(
                        deposit_data,
                        signature,
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!("[deposit] refund wallet sign+send error: code={:?} msg={}", e.code, e.raw_message);
                    components::show_toast(
                        &set_toast,
                        &crate::wallet_error::user_friendly_message(&e),
                        ToastType::Error,
                    );
                    set_state.set(DepositPageState::RefundWalletConnected(
                        deposit_data_for_state,
                        wallet_name_for_tx,
                        pk_for_tx,
                    ));
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    log::error!("[deposit] refund wallet sign+send failed");
                    components::show_toast(
                        &set_toast,
                        "Refund transaction failed. Please try again.",
                        ToastType::Error,
                    );
                    set_state.set(DepositPageState::RefundWalletConnected(
                        deposit_data_for_state,
                        wallet_name_for_tx,
                        pk_for_tx,
                    ));
                }
            }
        });
    };

    // --- Close Deposit: Connect wallet for close deposit ---
    let handle_close_deposit_connect_wallet = move |wallet_name: String| {
        let deposit_data = match &state.get() {
            DepositPageState::CloseDepositChooseWallet(d) => d.clone(),
            _ => return,
        };

        let wallet_name_clone = wallet_name.clone();
        let deposit_data_for_state = deposit_data.clone();
        leptos::task::spawn_local(async move {
            match connect_wallet_js(&wallet_name_clone).await {
                crate::wallet_error::WalletResult::Success(pubkey) => {
                    log::info!("[deposit] close-deposit wallet connected: {} ({})", wallet_name_clone, pubkey);
                    set_state.set(DepositPageState::CloseDepositWalletConnected(
                        deposit_data_for_state,
                        wallet_name_clone,
                        pubkey,
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!("[deposit] close-deposit wallet connect error: code={:?} msg={}", e.code, e.raw_message);
                    components::show_toast(
                        &set_toast,
                        &crate::wallet_error::user_friendly_message(&e),
                        ToastType::Error,
                    );
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    components::show_toast(
                        &set_toast,
                        "Failed to connect wallet. Please try again.",
                        ToastType::Error,
                    );
                }
            }
        });
    };

    // --- Close Deposit: Reclaim rent (sign & send close-deposit TX) ---
    let handle_close_deposit = move |wallet_name: String, public_key: String| {
        let current_state = state.get();
        let deposit_data = match &current_state {
            DepositPageState::CloseDepositWalletConnected(d, _, _) => d.clone(),
            _ => return,
        };

        // Extract attendee_id and event_id from URL
        let attendee_id = match params.get() {
            Ok(p) => p.attendee_id.unwrap_or_default(),
            Err(_) => return,
        };
        let event_id = web_sys::Url::new(
            &web_sys::window()
                .unwrap()
                .location()
                .href()
                .unwrap(),
        )
        .ok()
        .and_then(|url| url.search_params().get("event_id"))
        .unwrap_or_default();

        let wallet_name_for_tx = wallet_name.clone();
        let pk_for_tx = public_key.clone();
        let deposit_data_for_state = deposit_data.clone();

        // Transition to signing state
        set_state.set(DepositPageState::CloseDepositSigning(
            deposit_data.clone(),
            wallet_name.clone(),
            public_key.clone(),
        ));

        leptos::task::spawn_local(async move {
            // Step 1: Request close-deposit TX from backend
            let req = CloseDepositRequest {
                event_id: event_id.clone(),
                attendee_id: attendee_id.clone(),
                wallet_address: pk_for_tx.clone(),
            };
            let close_resp = match api::close_deposit(&req).await {
                Ok(r) => r,
                Err(e) => {
                    log::error!("[deposit] close-deposit TX build failed: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Failed to build close-deposit transaction: {e}"),
                        ToastType::Error,
                    );
                    set_state.set(DepositPageState::CloseDepositWalletConnected(
                        deposit_data_for_state,
                        wallet_name_for_tx,
                        pk_for_tx,
                    ));
                    return;
                }
            };

            let tx_b64 = close_resp.transaction;

            if tx_b64.is_empty() {
                log::error!("[deposit] close-deposit TX is empty");
                components::show_toast(
                    &set_toast,
                    "Close-deposit transaction was empty. Please try again later.",
                    ToastType::Error,
                );
                set_state.set(DepositPageState::CloseDepositWalletConnected(
                    deposit_data,
                    wallet_name_for_tx,
                    pk_for_tx,
                ));
                return;
            }

            // SEC-014: Verify wallet cluster matches expected network.
            let expected_cluster = crate::utils::get_cluster();
            if let Err(cluster_err) = crate::pages::escrow_init::check_wallet_cluster(&wallet_name_for_tx, &expected_cluster).await {
                log::error!("[deposit] cluster mismatch (close): {cluster_err}");
                components::show_toast(
                    &set_toast,
                    &cluster_err,
                    ToastType::Error,
                );
                return;
            }

            // Pre-sign simulation.
            match crate::pages::escrow_init::simulate_transaction_js(&wallet_name_for_tx, &tx_b64).await {
                Ok(sim) if sim.ok => {}
                Ok(sim) => {
                    let err_msg = sim.error.unwrap_or_else(|| "Simulation failed".to_string());
                    log::error!("[deposit] close simulation failed: {err_msg}");
                    components::show_toast(&set_toast, &format!("Transaction would fail: {err_msg}"), ToastType::Error);
                    return;
                }
                Err(e) => { log::warn!("[deposit] simulate error (not blocking): {e}"); }
            }

            // Step 2: Sign and send via wallet
            match sign_and_send_tx_js(&wallet_name_for_tx, &tx_b64).await {
                crate::wallet_error::WalletResult::Success(signature) => {
                    log::info!("[deposit] close-deposit TX sent, signature: {}", signature);
                    set_state.set(DepositPageState::CloseDepositConfirmed(
                        deposit_data,
                        signature,
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!("[deposit] close-deposit wallet sign+send error: code={:?} msg={}", e.code, e.raw_message);
                    components::show_toast(
                        &set_toast,
                        &crate::wallet_error::user_friendly_message(&e),
                        ToastType::Error,
                    );
                    set_state.set(DepositPageState::CloseDepositWalletConnected(
                        deposit_data_for_state,
                        wallet_name_for_tx,
                        pk_for_tx,
                    ));
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    log::error!("[deposit] close-deposit wallet sign+send failed");
                    components::show_toast(
                        &set_toast,
                        "Close-deposit transaction failed. Please try again.",
                        ToastType::Error,
                    );
                    set_state.set(DepositPageState::CloseDepositWalletConnected(
                        deposit_data_for_state,
                        wallet_name_for_tx,
                        pk_for_tx,
                    ));
                }
            }
        });
    };

    view! {
        <Title text="BeThere — Event Deposit" />
        <div class="center-page">
            <div class="container layout-col-center">
                // Logo
                <div class="brand-logo">"BeThere"</div>
                <div class="brand-logo-sub">"Proof of Attendance"</div>

                <h1 class="claim-title">"Event Deposit"</h1>

                // Logout button — only visible when signed in
                {move || match signed_in_email.get() {
                    Some(email) => view! {
                        <div class="logout-btn-wrapper">
                            <span style="color:var(--text-secondary);font-size:0.85rem;">
                                {format!("Welcome, {email}")}
                            </span>
                            <button
                                class="btn btn-outline btn-xs"
                                on:click=move |_| {
                                    leptos::task::spawn_local(async move {
                                        let _ = gloo::net::http::Request::post("/api/auth/logout")
                                                                                    .send()
                                                                                    .await;
                                                                                navigateTo("/");
                                    });
                                }
                            >
                                "Sign out"
                            </button>
                        </div>
                    }.into_any(),
                    None => ().into_any(),
                }}

                // Event context header — shows which event this deposit is for
                {move || {
                    let s = state.get();
                    match extract_event_context(&s) {
                        Some((name, tagline)) => view! {
                            <div class="event-context-header">
                                <div class="event-context-name">{name}</div>
                                {if !tagline.is_empty() {
                                    view! { <div class="event-context-tagline">{tagline}</div> }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}
                            </div>
                        }.into_any(),
                        None => view! { <div></div> }.into_any(),
                    }
                }}

                {move || {
                    let s = state.get();
                    match s {
                        // ===== Loading =====
                        DepositPageState::Loading => {
                            view! {
                                <div class="loading visible loading-top">
                                    <span class="spinner spinner-lg"></span>
                                    " Loading deposit info..."
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Error =====
                        DepositPageState::Error(msg) => {
                            view! {
                                <div class="card dep-card-error">
                                    <div class="card-header">
                                        <h2 class="card-title"><Icon icon=IconName::Warning class="icon-sm icon-danger" />" Error"</h2>
                                    </div>
                                    <p class="hint-desc">
                                        {msg}
                                    </p>
                                    <a href="/" class="btn btn-primary">"Go Home"</a>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Not Enabled =====
                        DepositPageState::NotEnabled => {
                            view! {
                                <div class="card dep-card-error">
                                    <div class="card-header">
                                        <h2 class="card-title"><Icon icon=IconName::CreditCard class="icon-sm" />" Deposits Not Available"</h2>
                                    </div>
                                    <p class="hint-desc">
                                        "Deposits are not enabled for this event."
                                    </p>
                                    <a href="/" class="btn btn-primary">"Go Home"</a>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Already Deposited =====
                        DepositPageState::AlreadyDeposited(data) => {
                            let info = data.status.as_ref().unwrap();
                            let (method_icon, method_label) = match info.method {
                                DepositMethod::Usdc => (IconName::Coin, "USDC (Solana)"),
                                DepositMethod::Thb => (IconName::Baht, "THB (PromptPay)"),
                                DepositMethod::CreditThb => (IconName::Baht, "THB Credit (held deposit)"),
                                DepositMethod::CreditUsdc => (IconName::Coin, "USDC Credit (held deposit)"),
                            };
                            let (verified_icon, verified_text) = if info.verified {
                                (IconName::Check, "Verified")
                            } else {
                                (IconName::Hourglass, "Pending Verification")
                            };
                            let verified_class = if info.verified {
                                "badge badge-success"
                            } else {
                                "badge badge-warning"
                            };
                            // Compute refund deadline for loss-aversion framing
                            let usdc_fmt = format!("{:.2}", data.deposit_amount_usdc as f64 / 1_000_000.0);
                            let refund_info = if data.event_end_ms > 0 && data.refund_deadline_hours > 0 {
                                let deadline_ms = data.event_end_ms + (i64::from(data.refund_deadline_hours) * 3_600_000);
                                let deadline_date = format_refund_deadline(deadline_ms);
                                let duration_label = format_duration_label(data.refund_deadline_hours);
                                Some((deadline_date, duration_label))
                            } else {
                                None
                            };
                            view! {
                                <div class="card dep-card-error">
                                    <div class="card-header">
                                        <h2 class="card-title"><Icon icon=IconName::Ticket class="icon-sm" />" Spot Reserved"</h2>
                                    </div>
                                    <div class="dep-details-block">
                                        <div class="dep-detail-row">
                                            <span class="dep-label">"Method"</span>
                                            <span><Icon icon=method_icon class="icon-sm" />" "{method_label}</span>
                                        </div>
                                        <div class="dep-detail-row">
                                            <span class="dep-label">"Amount"</span>
                                            <span>
                                                {format!("{} {}", info.amount, info.currency)}
                                            </span>
                                        </div>
                                        <div class="dep-detail-row-center">
                                            <span class="dep-label">"Status"</span>
                                            <span class=verified_class><Icon icon=verified_icon class="icon-sm" />" "{verified_text}</span>
                                        </div>
                                        <div class="dep-detail-row-last">
                                            <span class="dep-label">"Date"</span>
                                            <span>{format_timestamp(&info.deposited_at)}</span>
                                        </div>
                                    </div>
                                    {if info.verified && info.method == DepositMethod::Usdc {
                                        let data_clone_for_refund = data.clone();
                                        let refund_info_clone = refund_info.clone();
                                        view! {
                                            <div class="dep-info-note">
                                                <p class="hint-note">
                                                    <Icon icon=IconName::Coin class="icon-sm" />" "{format!("Your {usdc_fmt} USDC is secured on-chain. Show up → get it all back.")}
                                                </p>
                                            </div>
                                            // Refund deadline urgency (loss aversion)
                                            {match refund_info_clone {
                                                Some((deadline_date, duration_label)) => view! {
                                                    <div class="dep-info-note">
                                                        <p class="hint-note">
                                                            {format!("Refund window: {duration_label} after event ends ({deadline_date}).")}
                                                        </p>
                                                    </div>
                                                }.into_any(),
                                                None => view! { <div></div> }.into_any(),
                                            }}
                                            <button
                                                class="btn btn-success btn-block btn-action-lg"
                                                on:click=move |_| {
                                                    set_state.set(DepositPageState::RefundChooseWallet(data_clone_for_refund.clone()));
                                                }
                                            >
                                                <Icon icon=IconName::Coin class="icon-sm" />" "{format!("Don't lose your {usdc_fmt} USDC — claim it now")}
                                            </button>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div class="dep-info-note">
                                                <p class="hint-note">
                                                    <Icon icon=IconName::Coin class="icon-sm" />" "{format!("Your {usdc_fmt} USDC deposit is secured. Refund will be available after the event.")}
                                                </p>
                                            </div>
                                        }.into_any()
                                    }}
                                    <a href=if data.event_slug.is_empty() { "/".to_string() } else { format!("/e/{}", data.event_slug) } class="btn btn-primary action-row-top">"← Back to event"</a>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Choose Payment =====
                        DepositPageState::ChoosePayment(data) => {
                            let data_clone = data.clone();
                            let wallets = detected_wallets.get();
                            let is_dev_mode = data.dev_mode;
                            let deposit_deadline = data_clone.deposit_deadline_hours;
                            let deadline_expired = data_clone.deadline_expired;
                            let can_reclaim = data_clone.in_person_available.unwrap_or(false);
                            view! {
                                // Deadline expired banner
                                {if deadline_expired && !can_reclaim {
                                    // Fully expired — no reclaim possible
                                    view! {
                                        <div class="dep-info-note" style="margin-bottom:1rem">
                                            <div class="badge badge-warning" style="margin-bottom:0.5rem">
                                                "Deadline Expired"
                                            </div>
                                            <p class="hint-note">
                                                <Icon icon=IconName::Clock class="icon-sm" />
                                                " Your deposit deadline has passed and in-person spots are now full. You have been moved to the online track."
                                            </p>
                                            <p class="hint-note" style="margin-top:0.5rem">
                                                "You will be able to claim your NFT after the event ends."
                                            </p>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}

                                {if deadline_expired && can_reclaim {
                                    // Reclaim available — show payment options with reclaim banner
                                    view! {
                                        <div class="dep-info-note" style="margin-bottom:1rem">
                                            <div class="badge badge-success" style="margin-bottom:0.5rem">
                                                "Spot Still Available!"
                                            </div>
                                            <p class="hint-note">
                                                <Icon icon=IconName::Clock class="icon-sm" />
                                                " Your deposit deadline has passed, but in-person spots are still available! Complete your deposit now to reclaim your spot."
                                            </p>
                                        </div>
                                        <p class="subtitle subtitle-lg">
                                            "Choose your preferred payment method to secure your spot."
                                        </p>
                                    }.into_any()
                                } else if !deadline_expired {
                                    // Normal flow — not expired
                                    view! {
                                        <p class="subtitle subtitle-lg">
                                            "Choose your preferred payment method to secure your spot."
                                        </p>

                                        // Deposit deadline countdown warning
                                        {if let Some(hours) = deposit_deadline {
                                            let label = format_duration_label(hours);
                                            view! {
                                                <div class="dep-info-note" style="margin-bottom:1rem">
                                                    <p class="hint-note">
                                                        <Icon icon=IconName::Clock class="icon-sm" />
                                                        " You have "{label}" to complete your deposit. After that, your in-person spot may be released."
                                                    </p>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }}
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}

                                {if !deadline_expired || can_reclaim {
                                    view! {
                                        <div class="dep-methods">

                                    // USDC Card — only shown in dev mode
                                    {if is_dev_mode {
                                        view! {
                                            <div class="card">
                                                <div class="card-header">
                                                    <h2 class="card-title">"Pay with USDC"</h2>
                                                    <span class="badge badge-info">
                                                        {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                                    </span>
                                                </div>
                                                <p class="hint-desc">
                                                    "Pay via Solana. Connect your wallet to send the deposit directly, or use a QR code."
                                                </p>
                                                <span class="badge badge-muted" style="margin-bottom:0.5rem;">"🧪 Dev Mode"</span>

                                                // Wallet adapter buttons (shown if wallets detected)
                                                {if has_wallets() {
                                                    let wallets_for_click = wallets.clone();
                                                    view! {
                                                        <div class="wallet-list">
                                                            <p class="wallet-prompt">
                                                                <Icon icon=IconName::Link class="icon-sm" />" Connect your Solana wallet:"
                                                            </p>
                                                            {wallets_for_click.into_iter().map(|w| {
                                                                let w_clone = w.clone();
                                                                let wallet_icon = wallet_icon_name(&w);
                                                                view! {
                                                                    <button
                                                                        class="btn btn-primary btn-block wallet-btn-inner"
                                                                        on:click={
                                                                            let w = w.clone();
                                                                            move |_| handle_connect_wallet(w.clone())
                                                                        }
                                                                    >
                                                                        <Icon icon=wallet_icon class="icon-md wallet-icon-white" />
                                                                        <span>{format!("Connect {}", &w_clone)}</span>
                                                                    </button>
                                                                }
                                                            }).collect::<Vec<_>>()}
                                                        </div>
                                                    }.into_any()
                                                } else {
                                                    view! { <div></div> }.into_any()
                                                }}

                                                // QR fallback section
                                                <div class="dep-divider-section">
                                                    <p class="hint-sm">
                                                        <Icon icon=IconName::Phone class="icon-sm" />" No wallet? Use QR code instead:"
                                                    </p>
                                                    <div class="u-mb-sm">
                                                        <input
                                                            type="text"
                                                            class="form-input dep-input"
                                                            placeholder="Enter your Solana wallet address"
                                                            prop:value=move || wallet_input.get()
                                                            on:input=move |ev| {
                                                                let val = event_target_value(&ev);
                                                                set_wallet_input.set(val);
                                                            }
                                                        />
                                                    </div>
                                                    <button
                                                        class="btn btn-outline btn-block"
                                                        on:click=move |_| handle_pay_usdc_qr()
                                                    >
                                                        "Generate QR Code"
                                                    </button>
                                                </div>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }}

                                    // THB Card (always shown)
                                    <div class="card">
                                        <div class="card-header">
                                            <h2 class="card-title">"Pay with THB"</h2>
                                            <span class="badge badge-warning">
                                                {format!("{} THB", data_clone.deposit_amount_thb)}
                                            </span>
                                        </div>
                                        <p class="hint-desc">
                                            "Transfer via PromptPay and upload your payment slip."
                                        </p>

                                        // PromptPay QR — only shown when promptpay_id is configured
                                        // Reactive: re-evaluates when qr_ready changes (library loads async)
                                        {if !data_clone.promptpay_id.is_empty() && data_clone.deposit_amount_thb > 0 {
                                            let pp_id = data_clone.promptpay_id.clone();
                                            let pp_amount = data_clone.deposit_amount_thb as f64;
                                            let pp_amount_display = data_clone.deposit_amount_thb;
                                            let pp_reference = data_clone.event_name.clone();
                                            view! {
                                                <div class="layout-col-center u-mb-1rem">
                                                    <p class="text-amount">
                                                        {format!("Scan to pay {} THB", pp_amount_display)}
                                                    </p>
                                                    {move || {
                                                        if qr_ready.get() {
                                                            let pp_qr_string = generate_promptpay_qr_js(&pp_id, pp_amount, &pp_reference);
                                                            let pp_qr_image = pp_qr_string.as_ref().and_then(|s| generate_qr_data_url_js(s, 256));
                                                            match pp_qr_image {
                                                                Some(url) => {
                                                                    let url_for_save = url.clone();
                                                                    let pp_amount_for_filename = pp_amount_display;
                                                                    view! {
                                                                        <div class="qr-wrapper">
                                                                            <img src=url alt="PromptPay QR" class="qr-img-md" />
                                                                        </div>
                                                                        <button
                                                                            class="btn btn-outline btn-sm u-mt-sm"
                                                                            on:click=move |_| {
                                                                                download_data_url(&url_for_save, &format!("promptpay-{pp_amount_for_filename}THB-qr.png"));
                                                                            }
                                                                        >
                                                                            <Icon icon=IconName::Save class="icon-sm" />
                                                                            " Save QR Code"
                                                                        </button>
                                                                    }.into_any()
                                                                },
                                                                None => view! {
                                                                    <p class="hint-2xs">"QR generation failed — please pay manually."
                                                                    </p>
                                                                }.into_any(),
                                                            }
                                                        } else {
                                                            view! {
                                                                <div class="qr-wrapper qr-loading">
                                                                    <div class="qr-loading-spinner"></div>
                                                                    <p class="hint-2xs">"Loading QR generator..."</p>
                                                                </div>
                                                            }.into_any()
                                                        }
                                                    }}
                                                    <p class="qr-hint-text">
                                                        "Open your banking app → Scan QR → Pay"
                                                    </p>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }}

                                        // Upload section divider
                                        <div class="dep-divider-section">
                                            <label class="upload-label">
                                                <Icon icon=IconName::Clip class="icon-sm" />" Upload payment slip"
                                            </label>
                                            <input
                                                type="file"
                                                accept="image/*"
                                                node_ref=file_input_ref
                                                class="file-input-styled"
                                            />

                                            // Text input fallback for slip URL
                                            <details class="u-mt-xs">
                                                <summary class="details-summary-text">
                                                    "Or paste slip URL manually"
                                                </summary>
                                                <input
                                                    type="text"
                                                    class="form-input dep-input u-mt-xs"
                                                    placeholder="Paste slip image URL"
                                                    prop:value=move || slip_url_input.get()
                                                    on:input=move |ev| {
                                                        let val = event_target_value(&ev);
                                                        set_slip_url_input.set(val);
                                                    }
                                                />
                                            </details>

                                            // Bank info for refund
                                            <div class="u-mt-1rem" style="border-top:1px solid rgba(255,255,255,0.1);padding-top:0.75rem;">
                                                <p class="hint-desc" style="margin-bottom:0.5rem;">
                                                    "Bank account for refund"
                                                </p>
                                                <input
                                                    type="text"
                                                    class="form-input dep-input"
                                                    placeholder="Bank account number"
                                                    prop:value=move || bank_account_input.get()
                                                    on:input=move |ev| {
                                                        let val = event_target_value(&ev);
                                                        set_bank_account_input.set(val);
                                                    }
                                                />
                                                <div class="bank-dropdown u-mt-xs">
                                                    <input
                                                        type="text"
                                                        class="form-input dep-input"
                                                        placeholder="Bank name (e.g. KBank, SCB)"
                                                        prop:value=move || bank_name_input.get()
                                                        on:focus=move |_| set_show_bank_dropdown.set(true)
                                                        on:input=move |ev| {
                                                            let val = event_target_value(&ev);
                                                            set_bank_name_input.set(val);
                                                            set_show_bank_dropdown.set(true);
                                                        }
                                                        on:blur=move |_| {
                                                            set_timeout(move || set_show_bank_dropdown.set(false), std::time::Duration::from_millis(200));
                                                        }
                                                    />
                                                    {move || {
                                                        if !show_bank_dropdown.get() {
                                                            view! { <div></div> }.into_any()
                                                        } else {
                                                            let query = bank_name_input.get().to_lowercase();
                                                            let matches: Vec<&(&str, &str)> = THAI_BANKS
                                                                .iter()
                                                                .filter(|(code, name)| {
                                                                    if query.is_empty() { return true; }
                                                                    code.to_lowercase().contains(&query) || name.to_lowercase().contains(&query)
                                                                })
                                                                .collect();
                                                            if matches.is_empty() {
                                                                view! { <div></div> }.into_any()
                                                            } else {
                                                                let items: Vec<_> = matches.into_iter().map(|bank| {
                                                                    let bank_val = bank.1.to_string();
                                                                    let bank_display = bank_val.clone();
                                                                    view! {
                                                                        <div
                                                                            class="bank-dropdown-item"
                                                                            on:mousedown=move |ev| {
                                                                                ev.prevent_default();
                                                                                set_bank_name_input.set(bank_val.clone());
                                                                                set_show_bank_dropdown.set(false);
                                                                            }
                                                                        >
                                                                            <span class="bank-dropdown-name">{bank_display}</span>
                                                                        </div>
                                                                    }
                                                                }).collect();
                                                                view! {
                                                                    <div class="bank-dropdown-list">
                                                                        {items}
                                                                    </div>
                                                                }.into_any()
                                                            }
                                                        }
                                                    }}
                                                </div>
                                                <input
                                                    type="text"
                                                    class="form-input dep-input u-mt-xs"
                                                    placeholder="Account holder name"
                                                    prop:value=move || account_name_input.get()
                                                    on:input=move |ev| {
                                                        let val = event_target_value(&ev);
                                                        set_account_name_input.set(val);
                                                    }
                                                />
                                            </div>

                                            <button
                                                class="btn btn-success btn-block btn-action-lg u-mt-1rem"
                                                on:click=move |_| handle_upload_slip()
                                            >
                                                "Upload Slip"
                                            </button>
                                        </div>
                                    </div>
                                </div>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}

                                // Back to event
                                {
                                    let slug = data_clone.event_slug.clone();
                                    if !slug.is_empty() {
                                        view! {
                                            <a href=format!("/e/{slug}") class="link-back-home">
                                                "← Back to event"
                                            </a>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <a href="/" class="link-back-home">
                                                "← Back to home"
                                            </a>
                                        }.into_any()
                                    }
                                }
                            }
                                .into_any()
                        }

                        // ===== Wallet Connected — Ready to send TX =====
                        DepositPageState::WalletConnected(data, wallet_name, public_key) => {
                            let wallet_name_send = wallet_name.clone();
                            let pk_send = public_key.clone();
                            let wallet_icon = wallet_icon_name(&wallet_name);
                            let pk_short = if public_key.len() > 12 {
                                format!("{}...{}", &public_key[..4], &public_key[public_key.len()-4..])
                            } else {
                                public_key.clone()
                            };
                            view! {
                                <div class="card dep-card">
                                    <div class="card-header">
                                        <h2 class="card-title">"USDC Deposit"</h2>
                                        <span class="badge badge-info">
                                            {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                        </span>
                                    </div>
                                    <div class="wallet-connected-bar">
                                        <span class="wallet-icon-lg"><Icon icon=wallet_icon class="icon-lg wallet-icon-white" /></span>
                                        <div class="wallet-info-left">
                                            <div class="wallet-label">"Connected via " {wallet_name.clone()}</div>
                                            <div class="wallet-address-bold">{pk_short}</div>
                                        </div>
                                        <span class="badge badge-success u-ml-auto"><Icon icon=IconName::Check class="icon-sm icon-success" />" Connected"</span>
                                    </div>
                                    <p class="hint-desc">
                                        "Click below to send your deposit transaction. You'll be asked to approve the transaction in your wallet."
                                    </p>
                                    <button
                                        class="btn btn-primary btn-block btn-action-lg"
                                        on:click=move |_| handle_send_deposit(wallet_name_send.clone(), pk_send.clone())
                                    >
                                        "Send " {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)} " Deposit"
                                    </button>
                                    <button
                                        class="btn btn-outline btn-sm btn-action-secondary"
                                        on:click=move |_| {
                                            set_state.set(DepositPageState::ChoosePayment(data.clone()));
                                        }
                                    >
                                        "← Go Back"
                                    </button>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Awaiting Confirmation — polling for TX =====
                        DepositPageState::AwaitingConfirmation(data, _wallet_name, tx_sig) => {
                            let event_id = web_sys::Url::new(
                                &web_sys::window()
                                    .unwrap()
                                    .location()
                                    .href()
                                    .unwrap(),
                            )
                            .ok()
                            .and_then(|url| url.search_params().get("event_id"));
                            let attendee_id = match params.get() {
                                Ok(p) => p.attendee_id.unwrap_or_default(),
                                Err(_) => String::new(),
                            };
                            let sig_display = if tx_sig.len() > 20 {
                                format!("{}...{}", &tx_sig[..8], &tx_sig[tx_sig.len()-8..])
                            } else {
                                tx_sig.clone()
                            };

                            // Trigger confirmation polling
                            let eid = event_id.unwrap_or_default();
                            let aid = attendee_id.clone();
                            let sig = tx_sig.clone();
                            Effect::new(move |_| {
                                let eid_c = eid.clone();
                                let aid_c = aid.clone();
                                let sig_c = sig.clone();
                                leptos::task::spawn_local(async move {
                                    handle_poll_confirmation(eid_c, aid_c, sig_c);
                                });
                            });

                            view! {
                                <div class="card dep-card">
                                    <div class="card-header">
                                        <h2 class="card-title"><Icon icon=IconName::Hourglass class="icon-sm icon-warning" />" Confirming Deposit..."</h2>
                                        <span class="badge badge-info">
                                            {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                        </span>
                                    </div>
                                    <div class="spinner-wrap">
                                        <span class="spinner spinner-lg spinner-xl"></span>
                                    </div>
                                    <p class="hint-sm">
                                        "Your transaction has been submitted! Waiting for on-chain confirmation..."
                                    </p>
                                    <div class="tx-hash-box-top">
                                        {format!("TX: {}", &sig_display)}
                                    </div>
                                    <p class="hint-xs">
                                        "This usually takes 5-15 seconds. Don't close this page."
                                    </p>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Deposit Confirmed =====
                        DepositPageState::DepositConfirmed(data, tx_sig) => {
                            let sig_display = if tx_sig.len() > 20 {
                                format!("{}...{}", &tx_sig[..8], &tx_sig[tx_sig.len()-8..])
                            } else {
                                tx_sig.clone()
                            };
                            let solscan_url = solscan_tx_url(&tx_sig, &get_cluster());
                            let usdc_fmt = format!("{:.2}", data.deposit_amount_usdc as f64 / 1_000_000.0);
                            // Build ticket page link
                            let ticket_attendee_id = match params.get() {
                                Ok(p) => p.attendee_id.unwrap_or_default(),
                                Err(_) => String::new(),
                            };
                            let ticket_event_id = web_sys::Url::new(
                                &web_sys::window()
                                    .unwrap()
                                    .location()
                                    .href()
                                    .unwrap(),
                            )
                            .ok()
                            .and_then(|url| url.search_params().get("event_id"))
                            .unwrap_or_default();
                            let ticket_href = format!("/ticket/{ticket_attendee_id}?event_id={ticket_event_id}");
                            // Compute refund deadline info
                            let refund_info = if data.event_end_ms > 0 && data.refund_deadline_hours > 0 {
                                let deadline_ms = data.event_end_ms + (i64::from(data.refund_deadline_hours) * 3_600_000);
                                let deadline_date = format_refund_deadline(deadline_ms);
                                let duration_label = format_duration_label(data.refund_deadline_hours);
                                Some((deadline_date, duration_label))
                            } else {
                                None
                            };
                            view! {
                                <div class="card dep-card">
                                    <div class="card-header">
                                        <h2 class="card-title"><Icon icon=IconName::Ticket class="icon-sm" />" Spot Reserved!"</h2>
                                        <span class="badge badge-success">"On-chain verified"</span>
                                    </div>
                                    <div class="celebration-emoji"><Icon icon=IconName::Party class="icon-xl" /></div>
                                    <p class="success-title">
                                        {format!("{usdc_fmt} USDC deposited")}
                                    </p>
                                    <p class="hint-desc">
                                        "You're confirmed! Your spot is secured on Solana."
                                    </p>
                                    // Tier badge
                                    {
                                        let status = &data.status;
                                        match status {
                                            Some(s) if !s.refundable => view! {
                                                <div class="badge badge-warning" style="margin-top:0.5rem">
                                                    "Non-refundable (#" {s.deposit_order} ") — no refund on check-in"
                                                </div>
                                            }.into_any(),
                                            Some(s) => view! {
                                                <div class="badge badge-success" style="margin-top:0.5rem">
                                                    "Refundable (#" {s.deposit_order} ") — check in to get your deposit back"
                                                </div>
                                            }.into_any(),
                                            _ => view! { <div></div> }.into_any(),
                                        }
                                    }
                                    <div class="tx-hash-box">
                                        {format!("TX: {}", &sig_display)}
                                    </div>
                                    <a href=&solscan_url target="_blank" class="tx-explorer-link">
                                        "View on Solscan ↗"
                                    </a>
                                    // Ownership + deal framing
                                    <div class="dep-info-note-lg">
                                        <p class="hint-note">
                                            {format!("Show up → get your {usdc_fmt} USDC back. That's the deal.")}
                                        </p>
                                    </div>
                                    // Refund deadline info (loss aversion: inaction = loss)
                                    {match refund_info {
                                        Some((deadline_date, duration_label)) => view! {
                                            <div class="dep-info-note">
                                                <p class="hint-note">
                                                    <Icon icon=IconName::Coin class="icon-sm" />" "{format!("Refund window: {duration_label} after the event ends ({deadline_date}). Don't lose your deposit — claim it back.")}
                                                </p>
                                            </div>
                                        }.into_any(),
                                        None => view! {
                                            <div class="dep-info-note">
                                                <p class="hint-note">
                                                    <Icon icon=IconName::Coin class="icon-sm" />" Refund will be available after the event."
                                                </p>
                                            </div>
                                        }.into_any(),
                                    }}
                                    <div class="action-row-top-lg">
                                        <a href=ticket_href class="btn btn-primary">
                                            <Icon icon=IconName::Ticket class="icon-sm" />" View Your Ticket →"
                                        </a>
                                        <a href=if data.event_slug.is_empty() { "/".to_string() } else { format!("/e/{}", data.event_slug) } class="btn btn-outline">"← Back to event"</a>
                                    </div>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== USDC QR Ready =====
                        DepositPageState::UsdcQrReady(data, pay_url) => {
                            let pay_url_display = pay_url.clone();
                            let pay_url_copy = pay_url.clone();
                            let pay_url_qr = pay_url.clone();
                            let copied = pay_url_copied.get();
                            let copy_btn_text = if copied { "Copied!" } else { "Copy Link" };
                            let copy_btn_class = if copied { "btn btn-success btn-sm" } else { "btn btn-outline btn-sm" };
                            let copy_btn_icon = if copied { IconName::Check } else { IconName::Copy };

                            // Poll for payment confirmation (3s interval, 100 attempts = 5 min)
                            let eid_poll = web_sys::Url::new(
                                &web_sys::window()
                                    .unwrap()
                                    .location()
                                    .href()
                                    .unwrap(),
                            )
                            .ok()
                            .and_then(|url| url.search_params().get("event_id"))
                            .unwrap_or_default();
                            let aid_poll = match params.get() {
                                Ok(p) => p.attendee_id.unwrap_or_default(),
                                Err(_) => String::new(),
                            };
                            let deposit_data_poll = data.clone();
                            Effect::new(move |_| {
                                let eid_c = eid_poll.clone();
                                let aid_c = aid_poll.clone();
                                let dd = deposit_data_poll.clone();
                                leptos::task::spawn_local(async move {
                                    let mut attempts = 0u32;
                                    let max_attempts = 100u32; // 100 × 3s = 300s = 5 min
                                    while attempts < max_attempts {
                                        // Check if still in UsdcQrReady state
                                        let still_qr = matches!(&state.get(), DepositPageState::UsdcQrReady(_, _));
                                        if !still_qr {
                                            break;
                                        }
                                        match api::confirm_deposit(&eid_c, &aid_c).await {
                                            Ok(ConfirmDepositResponse {
                                                confirmed: true,
                                                tx_signature: Some(sig),
                                                ..
                                            }) => {
                                                log::info!("[deposit] QR payment confirmed: {}", sig);
                                                set_state.set(DepositPageState::DepositConfirmed(dd, sig));
                                                return;
                                            }
                                            Ok(_) => {
                                                attempts += 1;
                                                if attempts < max_attempts {
                                                    gloo::timers::future::TimeoutFuture::new(3000).await;
                                                }
                                            }
                                            Err(e) => {
                                                log::warn!("[deposit] QR poll error: {e}");
                                                attempts += 1;
                                                if attempts < max_attempts {
                                                    gloo::timers::future::TimeoutFuture::new(3000).await;
                                                }
                                            }
                                        }
                                    }
                                    if attempts >= max_attempts {
                                        log::warn!("[deposit] QR poll timed out after 5 min");
                                    }
                                });
                            });

                            view! {
                                <div class="card dep-card">
                                    <div class="card-header">
                                        <h2 class="card-title"><Icon icon=IconName::Coin class="icon-sm" />" USDC Payment Ready"</h2>
                                        <span class="badge badge-info">
                                            {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                        </span>
                                    </div>
                                    <p class="hint-desc">
                                        "Scan this QR code with a Solana wallet, or copy the link below:"
                                    </p>
                                    {move || {
                                        if qr_ready.get() {
                                            match generate_qr_data_url_js(&pay_url_qr, 256) {
                                                Some(url) => view! {
                                                    <div class="qr-wrapper">
                                                        <img src=url alt="Solana Pay QR" class="qr-img-lg" />
                                                    </div>
                                                }.into_any(),
                                                None => view! { <div></div> }.into_any(),
                                            }
                                        } else {
                                            view! {
                                                <div class="qr-wrapper qr-loading">
                                                    <div class="qr-loading-spinner"></div>
                                                </div>
                                            }.into_any()
                                        }
                                    }}
                                    <div class="tx-pay-url-box">
                                        {pay_url_display}
                                    </div>
                                    <button
                                        class=copy_btn_class
                                        on:click=move |_| handle_copy_url(pay_url_copy.clone())
                                    >
                                        <Icon icon=copy_btn_icon class="icon-sm" />" "{copy_btn_text}
                                    </button>
                                    <div class="dep-qr-polling">
                                        <span class="spinner spinner-sm"></span>
                                        " Checking for payment..."
                                    </div>
                                    <p class="hint-2xs u-mt-1rem">
                                        "After payment, your deposit will be verified automatically."
                                    </p>
                                </div>

                                {
                                    let slug = data.event_slug.clone();
                                    if !slug.is_empty() {
                                        view! {
                                            <a href=format!("/e/{slug}") class="link-back-home">
                                                "← Back to event"
                                            </a>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <a href="/" class="link-back-home">
                                                "← Back to home"
                                            </a>
                                        }.into_any()
                                    }
                                }
                            }
                                .into_any()
                        }

                        // ===== THB Uploading =====
                        DepositPageState::ThbUploading(_) => {
                            view! {
                                <div class="loading visible loading-top">
                                    <span class="spinner spinner-lg"></span>
                                    " Uploading slip..."
                                </div>
                            }
                                .into_any()
                        }

                        // ===== THB Uploaded =====
                        DepositPageState::ThbUploaded(attendee_id, event_id, _event_slug) => {
                            // Auto-redirect to ticket page after brief confirmation
                            let aid = attendee_id.clone();
                            let eid = event_id.clone();
                            leptos::task::spawn_local(async move {
                                gloo::timers::future::TimeoutFuture::new(1500).await;
                                navigateTo(&format!("/ticket/{aid}?event_id={eid}"));
                            });
                            view! {
                                <div class="card dep-card-error">
                                    <div class="card-header">
                                        <h2 class="card-title"><Icon icon=IconName::Check class="icon-sm icon-success" />" Slip Uploaded"</h2>
                                    </div>
                                    <p class="hint-desc">
                                        "Your payment slip has been submitted for verification. You'll be notified once it's confirmed."
                                    </p>
                                    <span class="badge badge-warning"><Icon icon=IconName::Hourglass class="icon-sm icon-warning" />" Pending Verification"</span>
                                    <p style="color:var(--text-secondary);font-size:0.8rem;margin-top:0.75rem;">
                                        "Redirecting to your ticket..."
                                    </p>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Refund: Choose Wallet =====
                        DepositPageState::RefundChooseWallet(data) => {
                            let wallets = detected_wallets.get();
                            let data_for_back = data.clone();
                            view! {
                                <div class="card dep-card">
                                    <div class="card-header">
                                        <h2 class="card-title">"Claim Refund"</h2>
                                        <span class="badge badge-info">
                                            {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                        </span>
                                    </div>
                                    <p class="hint-desc">
                                        "Connect the wallet you used to deposit. Your refund will be sent to the same wallet."
                                    </p>
                                    // Non-refundable check
                                    {if data.status.as_ref().map(|s| s.refundable).unwrap_or(true) {
                                        view! { <div></div> }.into_any()
                                    } else {
                                        view! {
                                            <div class="badge badge-warning" style="margin-bottom:0.75rem">
                                                "Non-refundable deposit — no refund available"
                                            </div>
                                        }.into_any()
                                    }}
                                    {if wallets.is_empty() {
                                        view! {
                                            <div class="wallet-fallback-box">
                                                <p class="wallet-fallback-text">
                                                    "No Solana wallet detected. Please install a wallet extension (Phantom, Backpack, Solflare) and refresh."
                                                </p>
                                            </div>
                                        }.into_any()
                                    } else {
                                        let wallets_for_click = wallets.clone();
                                        view! {
                                            <div class="wallet-list">
                                                {wallets_for_click.into_iter().map(|w| {
                                                    let w_clone = w.clone();
                                                    let wallet_icon = wallet_icon_name(&w);
                                                    view! {
                                                        <button
                                                            class="btn btn-primary btn-block wallet-btn-inner"
                                                            on:click={
                                                                let w = w.clone();
                                                                move |_| handle_refund_connect_wallet(w.clone())
                                                            }
                                                        >
                                                            <Icon icon=wallet_icon class="icon-md wallet-icon-white" />
                                                            <span>{format!("Connect {}", &w_clone)}</span>
                                                        </button>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        }.into_any()
                                    }}
                                    <button
                                        class="btn btn-outline btn-sm"
                                        on:click=move |_| {
                                            set_state.set(DepositPageState::AlreadyDeposited(data_for_back.clone()));
                                        }
                                    >
                                        "← Go Back"
                                    </button>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Refund: Wallet Connected — Ready to claim =====
                        DepositPageState::RefundWalletConnected(data, wallet_name, public_key) => {
                            let wallet_name_send = wallet_name.clone();
                            let pk_send = public_key.clone();
                            let wallet_icon = wallet_icon_name(&wallet_name);
                            let pk_short = if public_key.len() > 12 {
                                format!("{}...{}", &public_key[..4], &public_key[public_key.len()-4..])
                            } else {
                                public_key.clone()
                            };
                            let data_for_back = data.clone();
                            view! {
                                <div class="card dep-card">
                                    <div class="card-header">
                                        <h2 class="card-title">"Claim Refund"</h2>
                                        <span class="badge badge-info">
                                            {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                        </span>
                                    </div>
                                    <div class="wallet-connected-bar">
                                        <span class="wallet-icon-lg"><Icon icon=wallet_icon class="icon-lg wallet-icon-white" /></span>
                                        <div class="wallet-info-left">
                                            <div class="wallet-label">"Connected via " {wallet_name.clone()}</div>
                                            <div class="wallet-address-bold">{pk_short}</div>
                                        </div>
                                        <span class="badge badge-success u-ml-auto"><Icon icon=IconName::Check class="icon-sm icon-success" />" Connected"</span>
                                    </div>
                                    <p class="hint-desc">
                                        "Your deposit is waiting to be returned. Click below to claim it."
                                    </p>
                                    <button
                                        class="btn btn-success btn-block btn-action-lg"
                                        on:click=move |_| handle_claim_refund(wallet_name_send.clone(), pk_send.clone())
                                    >
                                        <Icon icon=IconName::Recycle class="icon-sm" />" Claim "{format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}" — Don't lose it"
                                    </button>
                                    <button
                                        class="btn btn-outline btn-sm btn-action-secondary"
                                        on:click=move |_| {
                                            set_state.set(DepositPageState::RefundChooseWallet(data_for_back.clone()));
                                        }
                                    >
                                        "← Go Back"
                                    </button>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Refund: Signing TX =====
                        DepositPageState::RefundSigning(data, _wallet_name, _public_key) => {
                            view! {
                                <div class="card dep-card">
                                    <div class="card-header">
                                        <h2 class="card-title"><Icon icon=IconName::Hourglass class="icon-sm icon-warning" />" Processing Refund..."</h2>
                                        <span class="badge badge-info">
                                            {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                        </span>
                                    </div>
                                    <div class="spinner-wrap">
                                        <span class="spinner spinner-lg spinner-xl"></span>
                                    </div>
                                    <p class="hint-sm">
                                        "Please approve the transaction in your wallet..."
                                    </p>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Refund: Confirmed =====
                        DepositPageState::RefundConfirmed(data, tx_sig) => {
                            let sig_display = if tx_sig.len() > 20 {
                                format!("{}...{}", &tx_sig[..8], &tx_sig[tx_sig.len()-8..])
                            } else {
                                tx_sig.clone()
                            };
                            let solscan_url = solscan_tx_url(&tx_sig, &get_cluster());
                            let usdc_fmt = format!("{:.2}", data.deposit_amount_usdc as f64 / 1_000_000.0);
                            view! {
                                <div class="card dep-card">
                                    <div class="card-header">
                                        <h2 class="card-title"><Icon icon=IconName::Party class="icon-sm" />" Refund Recovered & Rent Reclaimed!"</h2>
                                        <span class="badge badge-success">"On-chain verified"</span>
                                    </div>
                                    <div class="celebration-emoji"><Icon icon=IconName::Recycle class="icon-xl" /></div>
                                    <p class="success-title">
                                        {format!("{usdc_fmt} USDC + ~0.002 SOL returned to your wallet")}
                                    </p>
                                    <p class="hint-desc">
                                        "Your refund has been confirmed on Solana and your deposit account has been closed. Both the USDC refund and rent lamports should appear in your wallet shortly."
                                    </p>
                                    <div class="tx-hash-box">
                                        {format!("TX: {}", &sig_display)}
                                    </div>
                                    <a href=&solscan_url target="_blank" class="tx-explorer-link">
                                        "View on Solscan ↗"
                                    </a>
                                    <div class="action-row-top-lg">
                                        <a href=if data.event_slug.is_empty() { "/".to_string() } else { format!("/e/{}", data.event_slug) } class="btn btn-primary">"← Back to event"</a>
                                    </div>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Close Deposit: Choose Wallet =====
                        DepositPageState::CloseDepositChooseWallet(data) => {
                            let wallets = detected_wallets.get();
                            let data_for_back = data.clone();
                            view! {
                                <div class="card dep-card">
                                    <div class="card-header">
                                        <h2 class="card-title">"Reclaim Deposit Rent"</h2>
                                        <span class="badge badge-info">"~0.002 SOL"</span>
                                    </div>
                                    <p class="hint-desc">
                                        "Connect the wallet you used to deposit. This closes your deposit account and returns the rent-exempt SOL."
                                    </p>
                                    {if wallets.is_empty() {
                                        view! {
                                            <div class="wallet-fallback-box">
                                                <p class="wallet-fallback-text">
                                                    "No Solana wallet detected. Please install a wallet extension (Phantom, Backpack, Solflare) and refresh."
                                                </p>
                                            </div>
                                        }.into_any()
                                    } else {
                                        let wallets_for_click = wallets.clone();
                                        view! {
                                            <div class="wallet-list">
                                                {wallets_for_click.into_iter().map(|w| {
                                                    let w_clone = w.clone();
                                                    let wallet_icon = wallet_icon_name(&w);
                                                    view! {
                                                        <button
                                                            class="btn btn-primary btn-block wallet-btn-inner"
                                                            on:click={
                                                                let w = w.clone();
                                                                move |_| handle_close_deposit_connect_wallet(w.clone())
                                                            }
                                                        >
                                                            <Icon icon=wallet_icon class="icon-md wallet-icon-white" />
                                                            <span>{format!("Connect {}", &w_clone)}</span>
                                                        </button>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        }.into_any()
                                    }}
                                    <button
                                        class="btn btn-outline btn-sm"
                                        on:click=move |_| {
                                            set_state.set(DepositPageState::RefundChooseWallet(data_for_back.clone()));
                                        }
                                    >
                                        "← Go Back"
                                    </button>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Close Deposit: Wallet Connected — Ready to close =====
                        DepositPageState::CloseDepositWalletConnected(data, wallet_name, public_key) => {
                            let wallet_name_send = wallet_name.clone();
                            let pk_send = public_key.clone();
                            let wallet_icon = wallet_icon_name(&wallet_name);
                            let pk_short = if public_key.len() > 12 {
                                format!("{}...{}", &public_key[..4], &public_key[public_key.len()-4..])
                            } else {
                                public_key.clone()
                            };
                            let data_for_back = data.clone();
                            view! {
                                <div class="card dep-card">
                                    <div class="card-header">
                                        <h2 class="card-title">"Reclaim Deposit Rent"</h2>
                                        <span class="badge badge-info">"~0.002 SOL"</span>
                                    </div>
                                    <div class="wallet-connected-bar">
                                        <span class="wallet-icon-lg"><Icon icon=wallet_icon class="icon-lg wallet-icon-white" /></span>
                                        <div class="wallet-info-left">
                                            <div class="wallet-label">"Connected via " {wallet_name.clone()}</div>
                                            <div class="wallet-address-bold">{pk_short}</div>
                                        </div>
                                        <span class="badge badge-success u-ml-auto"><Icon icon=IconName::Check class="icon-sm icon-success" />" Connected"</span>
                                    </div>
                                    <p class="hint-desc">
                                        "Click below to close your deposit account and reclaim the rent-exempt SOL."
                                    </p>
                                    <button
                                        class="btn btn-success btn-block btn-action-lg"
                                        on:click=move |_| handle_close_deposit(wallet_name_send.clone(), pk_send.clone())
                                    >
                                        <Icon icon=IconName::Recycle class="icon-sm" />" Reclaim ~0.002 SOL Rent"
                                    </button>
                                    <button
                                        class="btn btn-outline btn-sm btn-action-secondary"
                                        on:click=move |_| {
                                            set_state.set(DepositPageState::CloseDepositChooseWallet(data_for_back.clone()));
                                        }
                                    >
                                        "← Go Back"
                                    </button>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Close Deposit: Signing TX =====
                        DepositPageState::CloseDepositSigning(_data, _wallet_name, _public_key) => {
                            view! {
                                <div class="card dep-card">
                                    <div class="card-header">
                                        <h2 class="card-title"><Icon icon=IconName::Hourglass class="icon-sm icon-warning" />" Closing Deposit..."</h2>
                                        <span class="badge badge-info">"~0.002 SOL"</span>
                                    </div>
                                    <div class="spinner-wrap">
                                        <span class="spinner spinner-lg spinner-xl"></span>
                                    </div>
                                    <p class="hint-sm">
                                        "Please approve the transaction in your wallet..."
                                    </p>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Close Deposit: Confirmed =====
                        DepositPageState::CloseDepositConfirmed(_data, tx_sig) => {
                            let sig_display = if tx_sig.len() > 20 {
                                format!("{}...{}", &tx_sig[..8], &tx_sig[tx_sig.len()-8..])
                            } else {
                                tx_sig.clone()
                            };
                            let solscan_url = solscan_tx_url(&tx_sig, &get_cluster());
                            view! {
                                <div class="card dep-card">
                                    <div class="card-header">
                                        <h2 class="card-title"><Icon icon=IconName::Party class="icon-sm" />" Rent Reclaimed!"</h2>
                                        <span class="badge badge-success">"On-chain verified"</span>
                                    </div>
                                    <div class="celebration-emoji"><Icon icon=IconName::Recycle class="icon-xl" /></div>
                                    <p class="success-title">
                                        "Deposit account closed"
                                    </p>
                                    <p class="hint-desc">
                                        "Your deposit account has been closed and ~0.002 SOL returned to your wallet."
                                    </p>
                                    <div class="tx-hash-box">
                                        {format!("TX: {}", &sig_display)}
                                    </div>
                                    <a href=&solscan_url target="_blank" class="tx-explorer-link">
                                        "View on Solscan ↗"
                                    </a>
                                    <div class="action-row-top-lg">
                                        <a href=if _data.event_slug.is_empty() { "/".to_string() } else { format!("/e/{}", _data.event_slug) } class="btn btn-primary">"← Back to event"</a>
                                    </div>
                                </div>
                            }
                                .into_any()
                        }
                    }
                }}

                // Footer
                <div class="claim-footer loading-top">
                    <div class="brand-line">
                        <span class="accent">"BeThere"</span>
                        " — Proof of Attendance"
                    </div>
                </div>
            </div>
        </div>

        // Toast notifications
        <Toast toast_signal=toast />
    }
}
