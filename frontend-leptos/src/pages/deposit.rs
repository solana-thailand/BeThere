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
    self, ConfirmDepositResponse, DepositStatusResponse, RefundTxRequest, ThbSlipUploadRequest,
    UsdcDepositRequest,
};
use crate::components::{self, Toast, ToastType};
use crate::utils::format_timestamp;
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

// ---------------------------------------------------------------------------
// JS interop — PromptPay QR generation
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/promptpay_qr.js")]
extern "C" {
    /// Generate an EMVCo QR string for Thai PromptPay payments.
    #[wasm_bindgen(js_name = "generatePromptPayQr")]
    fn generate_promptpay_qr_js(promptpay_id: &str, amount: f64) -> Option<String>;
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
async fn connect_wallet_js(wallet_name: &str) -> Option<String> {
    let promise = connect_wallet_js_raw(wallet_name);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => {
            if val.is_null() || val.is_undefined() {
                None
            } else {
                val.as_string()
            }
        }
        Err(e) => {
            log::error!("[wasm] connect_wallet_js error: {:?}", e);
            None
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
async fn sign_and_send_tx_js(wallet_name: &str, transaction_b64: &str) -> Option<String> {
    let promise = sign_and_send_tx_js_raw(wallet_name, transaction_b64);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => {
            if val.is_null() || val.is_undefined() {
                None
            } else {
                val.as_string()
            }
        }
        Err(e) => {
            log::error!("[wasm] sign_and_send_tx_js error: {:?}", e);
            None
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
    ThbUploaded,
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

    // Reactive state
    let (state, set_state) = signal(DepositPageState::Loading);
    let (toast, set_toast) = signal(None::<components::ToastMessage>);
    let (slip_url_input, set_slip_url_input) = signal(String::new());
    let (wallet_input, set_wallet_input) = signal(String::new());
    let (pay_url_copied, set_pay_url_copied) = signal(false);

    // File input ref for slip image upload
    let file_input_ref = NodeRef::<leptos::html::Input>::new();

    // Preload jsQR + QRious libraries on mount.
    // The deposit page renders PromptPay/USDC payment QR codes,
    // so libraries should be loaded before the payment view appears.
    leptos::task::spawn_local(async {
        preload_qr_libraries_js().await;
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
                Some(pubkey) => {
                    log::info!("[deposit] wallet connected: {} ({})", wallet_name_clone, pubkey);
                    set_state.set(DepositPageState::WalletConnected(
                        deposit_data_for_state,
                        wallet_name_clone,
                        pubkey,
                    ));
                }
                None => {
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

            // Step 4: Sign and send the TX via the wallet
            match sign_and_send_tx_js(&wallet_name_for_tx, &tx_b64).await {
                Some(signature) => {
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
                None => {
                    log::error!("[deposit] wallet sign+send failed");
                    components::show_toast(
                        &set_toast,
                        "Transaction rejected or failed. Please try again.",
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
        set_state.set(DepositPageState::ThbUploading(deposit_data));

        let file_ref = file_input_ref.clone();
        let text_slip_url = slip_url_input.get();
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
            };
            match api::upload_thb_slip(&body).await {
                Ok(_resp) => {
                    log::info!("[deposit] THB slip uploaded successfully");
                    set_state.set(DepositPageState::ThbUploaded);
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
                Some(pubkey) => {
                    log::info!("[deposit] refund wallet connected: {} ({})", wallet_name_clone, pubkey);
                    set_state.set(DepositPageState::RefundWalletConnected(
                        deposit_data_for_state,
                        wallet_name_clone,
                        pubkey,
                    ));
                }
                None => {
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

            // Step 2: Sign and send via wallet
            match sign_and_send_tx_js(&wallet_name_for_tx, &tx_b64).await {
                Some(signature) => {
                    log::info!("[deposit] refund TX sent, signature: {}", signature);
                    set_state.set(DepositPageState::RefundConfirmed(
                        deposit_data,
                        signature,
                    ));
                }
                None => {
                    log::error!("[deposit] refund wallet sign+send failed");
                    components::show_toast(
                        &set_toast,
                        "Refund transaction rejected or failed. Please try again.",
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

    view! {
        <Title text="BeThere — Event Deposit" />
        <div class="center-page">
            <div class="container" style="display:flex;flex-direction:column;align-items:center;">
                // Logo
                <div class="brand-logo">"BeThere"</div>
                <div class="brand-logo-sub">"Proof of Attendance"</div>

                <h1 class="claim-title">"Event Deposit"</h1>

                {move || {
                    let s = state.get();
                    match s {
                        // ===== Loading =====
                        DepositPageState::Loading => {
                            view! {
                                <div class="loading visible" style="margin-top:2rem;">
                                    <span class="spinner spinner-lg"></span>
                                    " Loading deposit info..."
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Error =====
                        DepositPageState::Error(msg) => {
                            view! {
                                <div class="card" style="margin-top:2rem;text-align:center;">
                                    <div class="card-header">
                                        <h2 class="card-title">"⚠️ Error"</h2>
                                    </div>
                                    <p style="color:var(--text-secondary);margin-bottom:1rem;">
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
                                <div class="card" style="margin-top:2rem;text-align:center;">
                                    <div class="card-header">
                                        <h2 class="card-title">"💳 Deposits Not Available"</h2>
                                    </div>
                                    <p style="color:var(--text-secondary);margin-bottom:1rem;">
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
                            let method_label = match info.method.as_str() {
                                "usdc" => "🪙 USDC (Solana)",
                                "thb" => "฿ THB (PromptPay)",
                                _ => &info.method,
                            };
                            let verified_badge = if info.verified {
                                "✅ Verified"
                            } else {
                                "⏳ Pending Verification"
                            };
                            let verified_class = if info.verified {
                                "badge badge-success"
                            } else {
                                "badge badge-warning"
                            };
                            view! {
                                <div class="card" style="margin-top:2rem;text-align:center;">
                                    <div class="card-header">
                                        <h2 class="card-title">"✅ Deposit Received"</h2>
                                    </div>
                                    <div style="text-align:left;margin:1rem 0;">
                                        <div style="display:flex;justify-content:space-between;padding:0.5rem 0;border-bottom:1px solid var(--border-color,rgba(255,255,255,0.1));">
                                            <span style="color:var(--text-secondary);">"Method"</span>
                                            <span>{method_label}</span>
                                        </div>
                                        <div style="display:flex;justify-content:space-between;padding:0.5rem 0;border-bottom:1px solid var(--border-color,rgba(255,255,255,0.1));">
                                            <span style="color:var(--text-secondary);">"Amount"</span>
                                            <span>
                                                {format!("{} {}", info.amount, info.currency)}
                                            </span>
                                        </div>
                                        <div style="display:flex;justify-content:space-between;align-items:center;padding:0.5rem 0;border-bottom:1px solid var(--border-color,rgba(255,255,255,0.1));">
                                            <span style="color:var(--text-secondary);">"Status"</span>
                                            <span class=verified_class>{verified_badge}</span>
                                        </div>
                                        <div style="display:flex;justify-content:space-between;padding:0.5rem 0;">
                                            <span style="color:var(--text-secondary);">"Date"</span>
                                            <span>{format_timestamp(&info.deposited_at)}</span>
                                        </div>
                                    </div>
                                    {if info.verified && info.method == "usdc" {
                                        let data_clone_for_refund = data.clone();
                                        view! {
                                            <div style="margin-top:0.75rem;padding:0.75rem;background:var(--bg-secondary,#1a1a2e);border-radius:8px;border:1px dashed var(--border-color,rgba(255,255,255,0.2));">
                                                <p style="font-size:0.85rem;color:var(--text-secondary);margin:0;">
                                                    "💰 Your deposit is secured on-chain. You can claim a refund after the event ends."
                                                </p>
                                            </div>
                                            <button
                                                class="btn btn-success btn-block"
                                                style="margin-top:1rem;font-size:1rem;padding:0.7rem;"
                                                on:click=move |_| {
                                                    set_state.set(DepositPageState::RefundChooseWallet(data_clone_for_refund.clone()));
                                                }
                                            >
                                                "💸 Claim Refund"
                                            </button>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div style="margin-top:0.75rem;padding:0.75rem;background:var(--bg-secondary,#1a1a2e);border-radius:8px;border:1px dashed var(--border-color,rgba(255,255,255,0.2));">
                                                <p style="font-size:0.85rem;color:var(--text-secondary);margin:0;">
                                                    "💰 Refund will be available after the event."
                                                </p>
                                            </div>
                                        }.into_any()
                                    }}
                                    <a href="/" class="btn btn-primary" style="margin-top:1rem;">"Go Home"</a>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Choose Payment =====
                        DepositPageState::ChoosePayment(data) => {
                            let data_clone = data.clone();
                            let wallets = detected_wallets.get();
                            view! {
                                <p class="subtitle" style="margin-bottom:1.5rem;">
                                    "Choose your preferred payment method to secure your spot."
                                </p>

                                <div style="width:100%;max-width:480px;display:flex;flex-direction:column;gap:1.5rem;">

                                    // USDC Card
                                    <div class="card">
                                        <div class="card-header">
                                            <h2 class="card-title">"🪙 Pay with USDC"</h2>
                                            <span class="badge badge-info">
                                                {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                            </span>
                                        </div>
                                        <p style="color:var(--text-secondary);font-size:0.9rem;margin-bottom:1rem;">
                                            "Pay via Solana. Connect your wallet to send the deposit directly, or use a QR code."
                                        </p>

                                        // Wallet adapter buttons (shown if wallets detected)
                                        {if has_wallets() {
                                            let wallets_for_click = wallets.clone();
                                            view! {
                                                <div style="display:flex;flex-direction:column;gap:0.5rem;margin-bottom:1rem;">
                                                    <p style="font-size:0.85rem;color:var(--text-secondary);margin-bottom:0.25rem;">
                                                        "🔗 Connect your Solana wallet:"
                                                    </p>
                                                    {wallets_for_click.into_iter().map(|w| {
                                                        let w_clone = w.clone();
                                                        let wallet_icon = match w.as_str() {
                                                            "Phantom" => "👻",
                                                            "Backpack" => "🎒",
                                                            "Solflare" => "☀️",
                                                            "Coinbase" => "🪙",
                                                            _ => "💼",
                                                        };
                                                        view! {
                                                            <button
                                                                class="btn btn-primary btn-block"
                                                                style="display:flex;align-items:center;justify-content:center;gap:0.5rem;"
                                                                on:click={
                                                                    let w = w.clone();
                                                                    move |_| handle_connect_wallet(w.clone())
                                                                }
                                                            >
                                                                <span>{wallet_icon}</span>
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
                                        <div style="border-top:1px solid var(--border-color,rgba(255,255,255,0.1));padding-top:1rem;margin-top:0.5rem;">
                                            <p style="font-size:0.85rem;color:var(--text-secondary);margin-bottom:0.5rem;">
                                                "📱 No wallet? Use QR code instead:"
                                            </p>
                                            <div style="margin-bottom:0.75rem;">
                                                <input
                                                    type="text"
                                                    class="form-input"
                                                    placeholder="Enter your Solana wallet address"
                                                    prop:value=move || wallet_input.get()
                                                    on:input=move |ev| {
                                                        let val = event_target_value(&ev);
                                                        set_wallet_input.set(val);
                                                    }
                                                    style="width:100%;padding:0.6rem 0.8rem;border-radius:6px;border:1px solid var(--border-color,rgba(255,255,255,0.2));background:var(--bg-secondary,#1a1a2e);color:var(--text-primary,#fff);font-size:0.9rem;"
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

                                    // THB Card
                                    <div class="card">
                                        <div class="card-header">
                                            <h2 class="card-title">"฿ Pay with THB"</h2>
                                            <span class="badge badge-warning">
                                                {format!("{} THB", data_clone.deposit_amount_thb)}
                                            </span>
                                        </div>
                                        <p style="color:var(--text-secondary);font-size:0.9rem;margin-bottom:1rem;">
                                            "Transfer via PromptPay and upload your payment slip."
                                        </p>

                                        // PromptPay QR — only shown when promptpay_id is configured
                                        {if !data_clone.promptpay_id.is_empty() && data_clone.deposit_amount_thb > 0 {
                                            let pp_qr_string = generate_promptpay_qr_js(
                                                &data_clone.promptpay_id,
                                                data_clone.deposit_amount_thb as f64,
                                            );
                                            let pp_qr_image = pp_qr_string.as_ref().and_then(|s| generate_qr_data_url_js(s, 256));
                                            view! {
                                                <div style="text-align:center;margin-bottom:1rem;">
                                                    <p style="font-size:0.95rem;font-weight:600;margin-bottom:0.75rem;">
                                                        {format!("Scan to pay {} THB", data_clone.deposit_amount_thb)}
                                                    </p>
                                                    {match pp_qr_image {
                                                        Some(url) => view! {
                                                            <div style="background:white;border-radius:12px;padding:1rem;display:inline-block;margin-bottom:0.5rem;">
                                                                <img src=url alt="PromptPay QR" style="width:220px;height:220px;" />
                                                            </div>
                                                        }.into_any(),
                                                        None => view! {
                                                            <p style="color:var(--text-secondary);font-size:0.8rem;">"QR generation failed — please pay manually."
                                                            </p>
                                                        }.into_any(),
                                                    }}
                                                    <p style="font-size:0.75rem;color:var(--text-secondary);margin-top:0.3rem;">
                                                        "Open your banking app → Scan QR → Pay"
                                                    </p>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }}

                                        // File upload for slip image
                                        <div style="margin-bottom:0.75rem;">
                                            <label style="display:block;font-size:0.85rem;color:var(--text-secondary);margin-bottom:0.3rem;">
                                                "📎 Upload payment slip image:"
                                            </label>
                                            <input
                                                type="file"
                                                accept="image/*"
                                                node_ref=file_input_ref
                                                style="width:100%;font-size:0.85rem;color:var(--text-secondary);margin-bottom:0.5rem;"
                                            />
                                        </div>

                                        // Text input fallback for slip URL
                                        <details style="margin-bottom:0.75rem;">
                                            <summary style="font-size:0.8rem;color:var(--text-secondary);cursor:pointer;margin-bottom:0.3rem;">
                                                "Or paste slip URL manually"
                                            </summary>
                                            <input
                                                type="text"
                                                class="form-input"
                                                placeholder="Paste slip image URL"
                                                prop:value=move || slip_url_input.get()
                                                on:input=move |ev| {
                                                    let val = event_target_value(&ev);
                                                    set_slip_url_input.set(val);
                                                }
                                                style="width:100%;padding:0.6rem 0.8rem;border-radius:6px;border:1px solid var(--border-color,rgba(255,255,255,0.2));background:var(--bg-secondary,#1a1a2e);color:var(--text-primary,#fff);font-size:0.9rem;margin-top:0.3rem;"
                                            />
                                        </details>

                                        <button
                                            class="btn btn-success btn-block"
                                            on:click=move |_| handle_upload_slip()
                                        >
                                            "Upload Slip"
                                        </button>
                                    </div>

                                </div>

                                // Back to home
                                <a href="/" style="color:var(--text-secondary);font-size:0.85rem;margin-top:1.5rem;text-decoration:none;">
                                    "← Back to home"
                                </a>
                            }
                                .into_any()
                        }

                        // ===== Wallet Connected — Ready to send TX =====
                        DepositPageState::WalletConnected(data, wallet_name, public_key) => {
                            let wallet_name_send = wallet_name.clone();
                            let pk_send = public_key.clone();
                            let wallet_icon = match wallet_name.as_str() {
                                "Phantom" => "👻",
                                "Backpack" => "🎒",
                                "Solflare" => "☀️",
                                "Coinbase" => "🪙",
                                _ => "💼",
                            };
                            let pk_short = if public_key.len() > 12 {
                                format!("{}...{}", &public_key[..4], &public_key[public_key.len()-4..])
                            } else {
                                public_key.clone()
                            };
                            view! {
                                <div class="card" style="margin-top:1.5rem;text-align:center;width:100%;max-width:480px;">
                                    <div class="card-header">
                                        <h2 class="card-title">"🪙 USDC Deposit"
                                        </h2>
                                        <span class="badge badge-info">
                                            {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                        </span>
                                    </div>
                                    <div style="background:var(--bg-secondary,#1a1a2e);border-radius:8px;padding:1rem;margin-bottom:1rem;display:flex;align-items:center;justify-content:center;gap:0.75rem;">
                                        <span style="font-size:1.5rem;">{wallet_icon}</span>
                                        <div style="text-align:left;">
                                            <div style="font-size:0.8rem;color:var(--text-secondary);">"Connected via " {wallet_name.clone()}</div>
                                            <div style="font-weight:600;font-family:monospace;">{pk_short}</div>
                                        </div>
                                        <span class="badge badge-success" style="margin-left:auto;">"✅ Connected"</span>
                                    </div>
                                    <p style="color:var(--text-secondary);font-size:0.9rem;margin-bottom:1rem;">
                                        "Click below to send your deposit transaction. You'll be asked to approve the transaction in your wallet."
                                    </p>
                                    <button
                                        class="btn btn-primary btn-block"
                                        style="font-size:1.1rem;padding:0.8rem;"
                                        on:click=move |_| handle_send_deposit(wallet_name_send.clone(), pk_send.clone())
                                    >
                                        "Send " {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)} " Deposit"
                                    </button>
                                    <button
                                        class="btn btn-outline btn-sm"
                                        style="margin-top:0.75rem;"
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
                                <div class="card" style="margin-top:1.5rem;text-align:center;width:100%;max-width:480px;">
                                    <div class="card-header">
                                        <h2 class="card-title">"⏳ Confirming Deposit..."</h2>
                                        <span class="badge badge-info">
                                            {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                        </span>
                                    </div>
                                    <div style="margin:1.5rem 0;">
                                        <span class="spinner spinner-lg" style="width:48px;height:48px;border-width:3px;"></span>
                                    </div>
                                    <p style="color:var(--text-secondary);font-size:0.9rem;margin-bottom:0.5rem;">
                                        "Your transaction has been submitted! Waiting for on-chain confirmation..."
                                    </p>
                                    <div style="background:var(--bg-secondary,#1a1a2e);border-radius:8px;padding:0.75rem;margin-top:0.75rem;font-family:monospace;font-size:0.75rem;color:var(--text-secondary);word-break:break-all;">
                                        {format!("TX: {}", &sig_display)}
                                    </div>
                                    <p style="font-size:0.75rem;color:var(--text-secondary);margin-top:0.75rem;">
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
                            let solscan_url = format!("https://solscan.io/tx/{}?cluster=devnet", tx_sig);
                            view! {
                                <div class="card" style="margin-top:1.5rem;text-align:center;width:100%;max-width:480px;">
                                    <div class="card-header">
                                        <h2 class="card-title">"✅ Deposit Confirmed!"</h2>
                                        <span class="badge badge-success">"On-chain verified"</span>
                                    </div>
                                    <div style="font-size:3rem;margin:1rem 0;">"🎉"</div>
                                    <p style="font-size:1.1rem;font-weight:600;margin-bottom:0.5rem;">
                                        {format!("{:.2} USDC deposited", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                    </p>
                                    <p style="color:var(--text-secondary);font-size:0.9rem;margin-bottom:1rem;">
                                        "Your deposit has been confirmed on Solana. You're all set for the event!"
                                    </p>
                                    <div style="background:var(--bg-secondary,#1a1a2e);border-radius:8px;padding:0.75rem;margin-bottom:1rem;font-family:monospace;font-size:0.75rem;color:var(--text-secondary);word-break:break-all;">
                                        {format!("TX: {}", &sig_display)}
                                    </div>
                                    <a href=&solscan_url target="_blank" style="color:var(--accent,#14f195);font-size:0.85rem;">
                                        "View on Solscan ↗"
                                    </a>
                                    <div style="margin-top:1rem;padding:0.75rem;background:var(--bg-secondary,#1a1a2e);border-radius:8px;border:1px dashed var(--border-color,rgba(255,255,255,0.2));">
                                        <p style="font-size:0.85rem;color:var(--text-secondary);margin:0;">
                                            "💰 Refund will be available after the event."
                                        </p>
                                    </div>
                                    <div style="margin-top:1.25rem;">
                                        <a href="/" class="btn btn-primary">"Go Home"</a>
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
                            let qr_data_url = generate_qr_data_url_js(&pay_url_qr, 256);
                            let copied = pay_url_copied.get();
                            let copy_btn_text = if copied { "✅ Copied!" } else { "📋 Copy Link" };
                            let copy_btn_class = if copied { "btn btn-success btn-sm" } else { "btn btn-outline btn-sm" };
                            view! {
                                <div class="card" style="margin-top:1.5rem;text-align:center;width:100%;max-width:480px;">
                                    <div class="card-header">
                                        <h2 class="card-title">"🪙 USDC Payment Ready"</h2>
                                        <span class="badge badge-info">
                                            {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                        </span>
                                    </div>
                                    <p style="color:var(--text-secondary);font-size:0.9rem;margin-bottom:1rem;">
                                        "Scan this QR code with a Solana wallet, or copy the link below:"
                                    </p>
                                    {match qr_data_url {
                                        Some(url) => view! {
                                            <div style="background:white;border-radius:12px;padding:1rem;display:inline-block;margin-bottom:1rem;">
                                                <img src=url alt="Solana Pay QR" style="width:256px;height:256px;" />
                                            </div>
                                        }.into_any(),
                                        None => view! { <div></div> }.into_any(),
                                    }}
                                    <div style="background:var(--bg-secondary,#1a1a2e);border:1px solid var(--border-color,rgba(255,255,255,0.15));border-radius:8px;padding:1rem;margin-bottom:1rem;word-break:break-all;font-size:0.8rem;color:var(--text-secondary);text-align:left;">
                                        {pay_url_display}
                                    </div>
                                    <button
                                        class=copy_btn_class
                                        on:click=move |_| handle_copy_url(pay_url_copy.clone())
                                    >
                                        {copy_btn_text}
                                    </button>
                                    <p style="font-size:0.8rem;color:var(--text-secondary);margin-top:1rem;">
                                        "After payment, your deposit will be verified automatically."
                                    </p>
                                </div>

                                <a href="/" style="color:var(--text-secondary);font-size:0.85rem;margin-top:1.5rem;text-decoration:none;">
                                    "← Back to home"
                                </a>
                            }
                                .into_any()
                        }

                        // ===== THB Uploading =====
                        DepositPageState::ThbUploading(_) => {
                            view! {
                                <div class="loading visible" style="margin-top:2rem;">
                                    <span class="spinner spinner-lg"></span>
                                    " Uploading slip..."
                                </div>
                            }
                                .into_any()
                        }

                        // ===== THB Uploaded =====
                        DepositPageState::ThbUploaded => {
                            view! {
                                <div class="card" style="margin-top:2rem;text-align:center;">
                                    <div class="card-header">
                                        <h2 class="card-title">"✅ Slip Uploaded"</h2>
                                    </div>
                                    <p style="color:var(--text-secondary);margin-bottom:1rem;">
                                        "Your payment slip has been submitted for verification. You'll be notified once it's confirmed."
                                    </p>
                                    <span class="badge badge-warning">"⏳ Pending Verification"</span>
                                    <div style="margin-top:1rem;">
                                        <a href="/" class="btn btn-primary">"Go Home"</a>
                                    </div>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Refund: Choose Wallet =====
                        DepositPageState::RefundChooseWallet(data) => {
                            let wallets = detected_wallets.get();
                            let data_for_back = data.clone();
                            view! {
                                <div class="card" style="margin-top:1.5rem;text-align:center;width:100%;max-width:480px;">
                                    <div class="card-header">
                                        <h2 class="card-title">"💸 Claim Refund"</h2>
                                        <span class="badge badge-info">
                                            {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                        </span>
                                    </div>
                                    <p style="color:var(--text-secondary);font-size:0.9rem;margin-bottom:1rem;">
                                        "Connect the wallet you used to deposit. Your refund will be sent to the same wallet."
                                    </p>
                                    {if wallets.is_empty() {
                                        view! {
                                            <div style="padding:1rem;background:var(--bg-secondary,#1a1a2e);border-radius:8px;margin-bottom:1rem;">
                                                <p style="color:var(--text-secondary);font-size:0.85rem;margin:0;">
                                                    "No Solana wallet detected. Please install a wallet extension (Phantom, Backpack, Solflare) and refresh."
                                                </p>
                                            </div>
                                        }.into_any()
                                    } else {
                                        let wallets_for_click = wallets.clone();
                                        view! {
                                            <div style="display:flex;flex-direction:column;gap:0.5rem;margin-bottom:1rem;">
                                                {wallets_for_click.into_iter().map(|w| {
                                                    let w_clone = w.clone();
                                                    let wallet_icon = match w.as_str() {
                                                        "Phantom" => "👻",
                                                        "Backpack" => "🎒",
                                                        "Solflare" => "☀️",
                                                        "Coinbase" => "🪙",
                                                        _ => "💼",
                                                    };
                                                    view! {
                                                        <button
                                                            class="btn btn-primary btn-block"
                                                            style="display:flex;align-items:center;justify-content:center;gap:0.5rem;"
                                                            on:click={
                                                                let w = w.clone();
                                                                move |_| handle_refund_connect_wallet(w.clone())
                                                            }
                                                        >
                                                            <span>{wallet_icon}</span>
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
                            let wallet_icon = match wallet_name.as_str() {
                                "Phantom" => "👻",
                                "Backpack" => "🎒",
                                "Solflare" => "☀️",
                                "Coinbase" => "🪙",
                                _ => "💼",
                            };
                            let pk_short = if public_key.len() > 12 {
                                format!("{}...{}", &public_key[..4], &public_key[public_key.len()-4..])
                            } else {
                                public_key.clone()
                            };
                            let data_for_back = data.clone();
                            view! {
                                <div class="card" style="margin-top:1.5rem;text-align:center;width:100%;max-width:480px;">
                                    <div class="card-header">
                                        <h2 class="card-title">"💸 Claim Refund"</h2>
                                        <span class="badge badge-info">
                                            {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                        </span>
                                    </div>
                                    <div style="background:var(--bg-secondary,#1a1a2e);border-radius:8px;padding:1rem;margin-bottom:1rem;display:flex;align-items:center;justify-content:center;gap:0.75rem;">
                                        <span style="font-size:1.5rem;">{wallet_icon}</span>
                                        <div style="text-align:left;">
                                            <div style="font-size:0.8rem;color:var(--text-secondary);">"Connected via " {wallet_name.clone()}</div>
                                            <div style="font-weight:600;font-family:monospace;">{pk_short}</div>
                                        </div>
                                        <span class="badge badge-success" style="margin-left:auto;">"✅ Connected"</span>
                                    </div>
                                    <p style="color:var(--text-secondary);font-size:0.9rem;margin-bottom:1rem;">
                                        "Click below to claim your refund. You'll approve the transaction in your wallet."
                                    </p>
                                    <button
                                        class="btn btn-success btn-block"
                                        style="font-size:1.1rem;padding:0.8rem;"
                                        on:click=move |_| handle_claim_refund(wallet_name_send.clone(), pk_send.clone())
                                    >
                                        "💸 Claim " {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)} " Refund"
                                    </button>
                                    <button
                                        class="btn btn-outline btn-sm"
                                        style="margin-top:0.75rem;"
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
                                <div class="card" style="margin-top:1.5rem;text-align:center;width:100%;max-width:480px;">
                                    <div class="card-header">
                                        <h2 class="card-title">"⏳ Processing Refund..."</h2>
                                        <span class="badge badge-info">
                                            {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                        </span>
                                    </div>
                                    <div style="margin:1.5rem 0;">
                                        <span class="spinner spinner-lg" style="width:48px;height:48px;border-width:3px;"></span>
                                    </div>
                                    <p style="color:var(--text-secondary);font-size:0.9rem;margin-bottom:0.5rem;">
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
                            let solscan_url = format!("https://solscan.io/tx/{}?cluster=devnet", tx_sig);
                            view! {
                                <div class="card" style="margin-top:1.5rem;text-align:center;width:100%;max-width:480px;">
                                    <div class="card-header">
                                        <h2 class="card-title">"🎉 Refund Confirmed!"</h2>
                                        <span class="badge badge-success">"On-chain verified"</span>
                                    </div>
                                    <div style="font-size:3rem;margin:1rem 0;">"💰"</div>
                                    <p style="font-size:1.1rem;font-weight:600;margin-bottom:0.5rem;">
                                        {format!("{:.2} USDC refunded", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                    </p>
                                    <p style="color:var(--text-secondary);font-size:0.9rem;margin-bottom:1rem;">
                                        "Your refund has been confirmed on Solana. The funds should appear in your wallet shortly."
                                    </p>
                                    <div style="background:var(--bg-secondary,#1a1a2e);border-radius:8px;padding:0.75rem;margin-bottom:1rem;font-family:monospace;font-size:0.75rem;color:var(--text-secondary);word-break:break-all;">
                                        {format!("TX: {}", &sig_display)}
                                    </div>
                                    <a href=&solscan_url target="_blank" style="color:var(--accent,#14f195);font-size:0.85rem;">
                                        "View on Solscan ↗"
                                    </a>
                                    <div style="margin-top:1.25rem;">
                                        <a href="/" class="btn btn-primary">"Go Home"</a>
                                    </div>
                                </div>
                            }
                                .into_any()
                        }
                    }
                }}

                // Footer
                <div class="claim-footer" style="margin-top:2rem;">
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
