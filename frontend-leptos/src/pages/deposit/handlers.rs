//! Handler closures for the deposit page.
//!
//! Extracted from `mod.rs` to keep the main deposit component under 1024 lines.
//! Each function creates a self-contained handler closure that captures the reactive
//! signals it needs.

use leptos::prelude::*;

use crate::api::{
    self, CloseDepositRequest, ConfirmDepositResponse, DepositStatusResponse, RefundTxRequest,
    ThbSlipUploadRequest, UsdcDepositRequest,
};
use crate::components::{self as app_components, ToastType};

use super::js_interop;
use super::types::*;

// ---------------------------------------------------------------------------
// Connect wallet (deposit flow)
// ---------------------------------------------------------------------------

/// Create handler: connect a wallet for the deposit payment flow.
pub fn make_connect_wallet(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
) -> impl Fn(String) + Clone + Send + Sync + 'static {
    move |wallet_name: String| {
        let deposit_data = match &state.get() {
            DepositPageState::ChoosePayment(d) => d.clone(),
            _ => return,
        };

        let wallet_name_clone = wallet_name.clone();
        let deposit_data_for_state = deposit_data.clone();
        leptos::task::spawn_local(async move {
            match js_interop::connect_wallet(&wallet_name_clone).await {
                crate::wallet_error::WalletResult::Success(pubkey) => {
                    log::info!(
                        "[deposit] wallet connected: {} ({})",
                        wallet_name_clone,
                        pubkey
                    );
                    set_state.set(DepositPageState::WalletConnected(
                        deposit_data_for_state,
                        wallet_name_clone,
                        pubkey,
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!(
                        "[deposit] wallet connect error: code={:?} msg={}",
                        e.code,
                        e.raw_message
                    );
                    app_components::show_toast(
                        &set_toast,
                        &crate::wallet_error::user_friendly_message(&e),
                        ToastType::Error,
                    );
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    app_components::show_toast(
                        &set_toast,
                        "Failed to connect wallet. Please try again.",
                        ToastType::Error,
                    );
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Send deposit TX
// ---------------------------------------------------------------------------

/// Create handler: initiate and send a USDC deposit transaction.
pub fn make_send_deposit(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
    params: DepositParamsSignal,
) -> impl Fn(String, String) + Clone + Send + Sync + 'static {
    move |wallet_name: String, public_key: String| {
        let current_state = state.get();
        let deposit_data = match &current_state {
            DepositPageState::WalletConnected(d, _, _) => d.clone(),
            _ => return,
        };

        let attendee_id = match params.get() {
            Ok(p) => p.attendee_id.unwrap_or_default(),
            Err(_) => String::new(),
        };
        let event_id = extract_event_id_from_url();

        let pk_for_api = public_key.clone();
        let wallet_name_for_tx = wallet_name.clone();
        let deposit_data_for_state = deposit_data.clone();
        let event_id_str = event_id.unwrap_or_default();

        leptos::task::spawn_local(async move {
            let body = UsdcDepositRequest {
                event_id: event_id_str.clone(),
                attendee_id: attendee_id.clone(),
                wallet_address: pk_for_api,
            };
            let deposit_resp = match api::deposit_usdc(&body).await {
                Ok(r) => r,
                Err(e) => {
                    log::error!("[deposit] USDC deposit initiate failed: {e}");
                    app_components::show_toast(
                        &set_toast,
                        &format!("Failed to initiate deposit: {e}"),
                        ToastType::Error,
                    );
                    return;
                }
            };

            let callback_url = if deposit_resp.solana_pay_url.starts_with("solana:") {
                &deposit_resp.solana_pay_url[7..]
            } else {
                &deposit_resp.solana_pay_url
            };

            let tx_b64 = match js_interop::fetch_tx_from_callback(callback_url).await {
                Some(tx) => tx,
                None => {
                    log::error!("[deposit] failed to fetch TX from callback");
                    app_components::show_toast(
                        &set_toast,
                        "Failed to build deposit transaction. Please try again.",
                        ToastType::Error,
                    );
                    return;
                }
            };

            // SEC-014: Verify wallet cluster matches expected network
            let expected_cluster = crate::utils::get_cluster();
            if let Err(cluster_err) =
                crate::pages::escrow_init::check_wallet_cluster(&wallet_name_for_tx, &expected_cluster).await
            {
                log::error!("[deposit] cluster mismatch: {cluster_err}");
                app_components::show_toast(&set_toast, &cluster_err, ToastType::Error);
                return;
            }

            match crate::pages::escrow_init::simulate_transaction_js(
                &wallet_name_for_tx,
                &tx_b64,
            )
            .await
            {
                Ok(sim) if sim.ok => {}
                Ok(sim) => {
                    let err_msg = sim
                        .error
                        .unwrap_or_else(|| "Simulation failed".to_string());
                    log::error!("[deposit] simulation failed: {err_msg}");
                    app_components::show_toast(
                        &set_toast,
                        &format!("Transaction would fail: {err_msg}"),
                        ToastType::Error,
                    );
                    return;
                }
                Err(e) => {
                    log::warn!("[deposit] simulate error (not blocking): {e}");
                }
            }

            match js_interop::sign_and_send_tx(&wallet_name_for_tx, &tx_b64).await {
                crate::wallet_error::WalletResult::Success(signature) => {
                    log::info!("[deposit] TX sent, signature: {signature}");

                    let _ = api::record_deposit_tx(&event_id_str, &attendee_id, &signature).await;

                    set_state.set(DepositPageState::AwaitingConfirmation(
                        deposit_data_for_state.clone(),
                        wallet_name_for_tx.clone(),
                        signature.clone(),
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!(
                        "[deposit] wallet sign+send error: code={:?} msg={}",
                        e.code,
                        e.raw_message
                    );
                    app_components::show_toast(
                        &set_toast,
                        &crate::wallet_error::user_friendly_message(&e),
                        ToastType::Error,
                    );
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    log::error!("[deposit] wallet sign+send failed");
                    app_components::show_toast(
                        &set_toast,
                        "Transaction failed. Please try again.",
                        ToastType::Error,
                    );
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Shared confirmation polling
// ---------------------------------------------------------------------------

/// Polling configuration for the two deposit confirmation tiers.
///
/// - **Wallet TX**: 30 attempts × 2s = ~60s total
/// - **QR payment**: 100 attempts × 3s = ~5min total
pub struct PollConfig {
    pub max_attempts: u32,
    pub interval_ok_ms: u32,
    pub interval_err_ms: u32,
}

impl PollConfig {
    /// Wallet flow: fast polling (~60s).
    pub fn wallet() -> Self {
        Self { max_attempts: 30, interval_ok_ms: 2000, interval_err_ms: 3000 }
    }

    /// QR flow: long polling (~5min) with early-exit state check.
    pub fn qr() -> Self {
        Self { max_attempts: 100, interval_ok_ms: 3000, interval_err_ms: 3000 }
    }
}

/// Shared async polling loop for deposit confirmation.
///
/// Polls `api::confirm_deposit` until confirmed, max attempts reached, or an
/// optional `should_stop` closure returns `true`.
pub async fn poll_deposit_confirmation(
    event_id: &str,
    attendee_id: &str,
    config: &PollConfig,
    set_state: &WriteSignal<DepositPageState>,
    deposit_data: &DepositStatusResponse,
    should_stop: Option<&dyn Fn() -> bool>,
) -> PollOutcome {
    let mut attempts = 0u32;
    while attempts < config.max_attempts {
        if let Some(check) = should_stop {
            if check() {
                return PollOutcome::Cancelled;
            }
        }
        match api::confirm_deposit(event_id, attendee_id).await {
            Ok(ConfirmDepositResponse {
                confirmed: true,
                tx_signature: Some(sig),
                ..
            }) => {
                log::info!("[deposit] confirmed on-chain: {sig}");
                set_state.set(DepositPageState::DepositConfirmed(
                    deposit_data.clone(),
                    sig.clone(),
                ));
                return PollOutcome::Confirmed(sig);
            }
            Ok(_) => {
                attempts += 1;
                if attempts < config.max_attempts {
                    gloo_timers::future::TimeoutFuture::new(config.interval_ok_ms).await;
                }
            }
            Err(e) => {
                log::warn!("[deposit] confirmation poll error: {e}");
                attempts += 1;
                if attempts < config.max_attempts {
                    gloo_timers::future::TimeoutFuture::new(config.interval_err_ms).await;
                }
            }
        }
    }
    PollOutcome::Timeout
}

/// Result of the deposit confirmation polling loop.
#[derive(Debug)]
pub enum PollOutcome {
    /// Deposit confirmed on-chain with the given signature.
    Confirmed(String),
    /// Max attempts reached without confirmation.
    Timeout,
    /// Caller requested cancellation (e.g. user navigated away).
    Cancelled,
}

// ---------------------------------------------------------------------------
// Poll for deposit confirmation (wallet flow)
// ---------------------------------------------------------------------------

/// Create handler: poll backend for on-chain deposit confirmation.
pub fn make_poll_confirmation(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
) -> impl Fn(String, String, String) + Clone + Send + Sync + 'static {
    move |event_id: String, attendee_id: String, _tx_sig: String| {
        let set_state = set_state;
        let set_toast = set_toast;
        leptos::task::spawn_local(async move {
            let deposit_data = match &state.get() {
                DepositPageState::AwaitingConfirmation(d, _, _) => d.clone(),
                _ => return,
            };
            let config = PollConfig::wallet();
            let outcome = poll_deposit_confirmation(
                &event_id,
                &attendee_id,
                &config,
                &set_state,
                &deposit_data,
                None,
            )
            .await;
            if matches!(outcome, PollOutcome::Timeout) {
                app_components::show_toast(
                    &set_toast,
                    "Confirmation is taking longer than expected. Your deposit may still be processing.",
                    ToastType::Warning,
                );
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Poll for deposit confirmation (QR flow)
// ---------------------------------------------------------------------------

/// Create handler: poll backend for QR-based deposit confirmation.
///
/// Similar to `make_poll_confirmation` but with longer timeout and an
/// early-exit check that stops if the page state leaves `UsdcQrReady`.
pub fn make_qr_poll_confirmation(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    params: DepositParamsSignal,
) -> impl Fn() + Clone + Send + Sync + 'static {
    move || {
        let set_state = set_state;
        let state = state;
        let deposit_data = match &state.get() {
            DepositPageState::UsdcQrReady(d, _) => d.clone(),
            _ => return,
        };
        let event_id = extract_event_id_from_url().unwrap_or_default();
        let attendee_id = match params.get() {
            Ok(p) => p.attendee_id.unwrap_or_default(),
            Err(_) => String::new(),
        };
        leptos::task::spawn_local(async move {
            let config = PollConfig::qr();
            let outcome = poll_deposit_confirmation(
                &event_id,
                &attendee_id,
                &config,
                &set_state,
                &deposit_data,
                Some(&|| !matches!(&state.get(), DepositPageState::UsdcQrReady(_, _))),
            )
            .await;
            if matches!(outcome, PollOutcome::Timeout) {
                log::warn!("[deposit] QR poll timed out after 5 min");
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Pay USDC via QR
// ---------------------------------------------------------------------------

/// Create handler: initiate USDC QR payment (no wallet needed).
pub fn make_pay_usdc_qr(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
    wallet_input: ReadSignal<String>,
    params: DepositParamsSignal,
) -> impl Fn() + Clone + Send + Sync + 'static {
    move || {
        let current_state = state.get();
        let (deposit_data, attendee_id, event_id) = match &current_state {
            DepositPageState::ChoosePayment(d) => {
                let aid = match params.get() {
                    Ok(p) => p.attendee_id.unwrap_or_default(),
                    Err(_) => return,
                };
                let eid = extract_event_id_from_url();
                (d.clone(), aid, eid)
            }
            _ => return,
        };

        let wallet = wallet_input.get();
        if wallet.trim().is_empty() {
            app_components::show_toast(
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
                    app_components::show_toast(
                        &set_toast,
                        &format!("Failed to initiate USDC payment: {e}"),
                        ToastType::Error,
                    );
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Upload THB slip
// ---------------------------------------------------------------------------

/// Create handler: upload THB PromptPay slip with bank account info.
#[allow(clippy::too_many_arguments)]
pub fn make_upload_slip(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
    slip_url_input: ReadSignal<String>,
    bank_account_input: ReadSignal<String>,
    bank_name_input: ReadSignal<String>,
    account_name_input: ReadSignal<String>,
    file_input_ref: NodeRef<leptos::html::Input>,
    params: DepositParamsSignal,
) -> impl Fn() + Clone + Send + Sync + 'static {
    move || {
        let current_state = state.get();
        let (deposit_data, attendee_id, event_id) = match &current_state {
            DepositPageState::ChoosePayment(d) => {
                let aid = match params.get() {
                    Ok(p) => p.attendee_id.unwrap_or_default(),
                    Err(_) => return,
                };
                let eid = extract_event_id_from_url();
                (d.clone(), aid, eid)
            }
            _ => return,
        };

        let deposit_data_for_err = deposit_data.clone();
        let deposit_data_slug = deposit_data.event_slug.clone();
        set_state.set(DepositPageState::ThbUploading(deposit_data));

        let file_ref = file_input_ref.clone();
        let text_slip_url = slip_url_input.get();
        let bank_account_input_for_upload = bank_account_input.get();
        let bank_name_input_for_upload = bank_name_input.get();
        let account_name_input_for_upload = account_name_input.get();
        let params = params.clone();

        leptos::task::spawn_local(async move {
            let slip_url = match file_ref.get() {
                Some(el) => {
                    let js_val: wasm_bindgen::JsValue = el.into();
                    match js_interop::read_file_as_data_url(&js_val).await {
                        Some(data_url) => data_url,
                        None => {
                            if text_slip_url.trim().is_empty() {
                                set_state.set(DepositPageState::ChoosePayment(
                                    deposit_data_for_err,
                                ));
                                app_components::show_toast(
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
                        app_components::show_toast(
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
                    let aid = match params.get() {
                        Ok(p) => p.attendee_id.unwrap_or_default(),
                        Err(_) => String::new(),
                    };
                    let eid = extract_event_id_from_url().unwrap_or_default();
                    set_state.set(DepositPageState::ThbUploaded(aid, eid, deposit_data_slug));
                }
                Err(e) => {
                    log::error!("[deposit] THB slip upload failed: {e}");
                    // 401 = JWT missing/expired. The deposit page itself is
                    // public (loads deposit status without auth), but
                    // `/api/deposit/thb/upload` is gated by `require_identity`.
                    // Without this branch, the user sees a generic "Failed to
                    // upload slip" toast and has no path forward — able to
                    // view but unable to act. Route to `ThbAuthRequired`,
                    // which renders a clear "session expired, sign in" CTA
                    // and unblocks the user.
                    if e.status == 401 {
                        app_components::show_toast(
                            &set_toast,
                            "Session expired. Please sign in again to upload your slip.",
                            ToastType::Warning,
                        );
                        let aid = match params.get() {
                            Ok(p) => p.attendee_id.unwrap_or_default(),
                            Err(_) => return,
                        };
                        let eid = extract_event_id_from_url();
                        match api::get_deposit_status(&aid, eid.as_deref()).await {
                            Ok(data) => {
                                set_state.set(DepositPageState::ThbAuthRequired(data));
                            }
                            Err(_) => {
                                set_state.set(DepositPageState::Error(
                                    "Failed to reload deposit status.".to_string(),
                                ));
                            }
                        }
                        return;
                    }
                    let error_msg = if e.to_string().contains("413")
                        || e.to_string().contains("too large")
                    {
                        "Image is too large to upload. Please resize or compress it to under 3MB and try again."
                            .to_string()
                    } else if e.to_string().contains("File size exceeds") {
                        e.to_string()
                    } else {
                        format!("Failed to upload slip: {e}")
                    };
                    app_components::show_toast(&set_toast, &error_msg, ToastType::Error);
                    let aid = match params.get() {
                        Ok(p) => p.attendee_id.unwrap_or_default(),
                        Err(_) => return,
                    };
                    let eid = extract_event_id_from_url();
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
    }
}

// ---------------------------------------------------------------------------
// Copy payment URL
// ---------------------------------------------------------------------------

/// Create handler: copy payment URL to clipboard.
pub fn make_copy_url(
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
    set_pay_url_copied: WriteSignal<bool>,
) -> impl Fn(String) + Clone + Send + Sync + 'static {
    move |url: String| {
        if js_interop::copy_to_clipboard(&url) {
            set_pay_url_copied.set(true);
            app_components::show_toast(&set_toast, "Payment link copied!", ToastType::Success);
            set_timeout(
                move || set_pay_url_copied.set(false),
                std::time::Duration::from_secs(3),
            );
        } else {
            app_components::show_toast(
                &set_toast,
                "Failed to copy. Please copy the link manually.",
                ToastType::Error,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Refund: connect wallet
// ---------------------------------------------------------------------------

/// Create handler: connect wallet for the refund flow.
pub fn make_refund_connect_wallet(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
) -> impl Fn(String) + Clone + Send + Sync + 'static {
    move |wallet_name: String| {
        let deposit_data = match &state.get() {
            DepositPageState::RefundChooseWallet(d) => Some(d.clone()),
            _ => None,
        };
        let deposit_data = match deposit_data {
            Some(d) => d,
            None => return,
        };

        let wallet_name_clone = wallet_name.clone();
        let deposit_data_for_state = deposit_data.clone();
        leptos::task::spawn_local(async move {
            match js_interop::connect_wallet(&wallet_name_clone).await {
                crate::wallet_error::WalletResult::Success(pubkey) => {
                    log::info!(
                        "[deposit] refund wallet connected: {} ({})",
                        wallet_name_clone,
                        pubkey
                    );
                    set_state.set(DepositPageState::RefundWalletConnected(
                        deposit_data_for_state,
                        wallet_name_clone,
                        pubkey,
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!(
                        "[deposit] refund wallet connect error: code={:?} msg={}",
                        e.code,
                        e.raw_message
                    );
                    app_components::show_toast(
                        &set_toast,
                        &crate::wallet_error::user_friendly_message(&e),
                        ToastType::Error,
                    );
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    app_components::show_toast(
                        &set_toast,
                        "Failed to connect wallet. Please try again.",
                        ToastType::Error,
                    );
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Claim refund
// ---------------------------------------------------------------------------

/// Create handler: build, sign, and send a refund transaction.
pub fn make_claim_refund(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
    params: DepositParamsSignal,
) -> impl Fn(String, String) + Clone + Send + Sync + 'static {
    move |wallet_name: String, public_key: String| {
        let current_state = state.get();
        let deposit_data = match &current_state {
            DepositPageState::RefundWalletConnected(d, _, _) => d.clone(),
            _ => return,
        };

        let attendee_id = match params.get() {
            Ok(p) => p.attendee_id.unwrap_or_default(),
            Err(_) => String::new(),
        };
        let event_id = extract_event_id_from_url().unwrap_or_default();

        let wallet_name_for_tx = wallet_name.clone();
        let pk_for_tx = public_key.clone();
        let deposit_data_for_state = deposit_data.clone();

        set_state.set(DepositPageState::RefundSigning(
            deposit_data.clone(),
            wallet_name.clone(),
            public_key.clone(),
        ));

        leptos::task::spawn_local(async move {
            let body = RefundTxRequest {
                event_id: event_id.clone(),
                attendee_id: attendee_id.clone(),
                wallet_address: pk_for_tx.clone(),
            };
            let refund_resp = match api::build_refund_tx(&body).await {
                Ok(r) => r,
                Err(e) => {
                    log::error!("[deposit] refund TX build failed: {e}");
                    app_components::show_toast(
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
                app_components::show_toast(
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

            // SEC-014
            let expected_cluster = crate::utils::get_cluster();
            if let Err(cluster_err) =
                crate::pages::escrow_init::check_wallet_cluster(&wallet_name_for_tx, &expected_cluster).await
            {
                log::error!("[deposit] cluster mismatch (refund): {cluster_err}");
                app_components::show_toast(&set_toast, &cluster_err, ToastType::Error);
                return;
            }

            match crate::pages::escrow_init::simulate_transaction_js(
                &wallet_name_for_tx,
                &tx_b64,
            )
            .await
            {
                Ok(sim) if sim.ok => {}
                Ok(sim) => {
                    let err_msg = sim
                        .error
                        .unwrap_or_else(|| "Simulation failed".to_string());
                    log::error!("[deposit] refund simulation failed: {err_msg}");
                    app_components::show_toast(
                        &set_toast,
                        &format!("Transaction would fail: {err_msg}"),
                        ToastType::Error,
                    );
                    return;
                }
                Err(e) => {
                    log::warn!("[deposit] simulate error (not blocking): {e}");
                }
            }

            match js_interop::sign_and_send_tx(&wallet_name_for_tx, &tx_b64).await {
                crate::wallet_error::WalletResult::Success(signature) => {
                    log::info!("[deposit] refund TX sent, signature: {signature}");
                    set_state.set(DepositPageState::RefundConfirmed(deposit_data, signature));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!(
                        "[deposit] refund wallet sign+send error: code={:?} msg={}",
                        e.code,
                        e.raw_message
                    );
                    app_components::show_toast(
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
                    app_components::show_toast(
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
    }
}

// ---------------------------------------------------------------------------
// Close deposit: connect wallet
// ---------------------------------------------------------------------------

/// Create handler: connect wallet for the close-deposit (reclaim rent) flow.
pub fn make_close_deposit_connect_wallet(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
) -> impl Fn(String) + Clone + Send + Sync + 'static {
    move |wallet_name: String| {
        let deposit_data = match &state.get() {
            DepositPageState::CloseDepositChooseWallet(d) => Some(d.clone()),
            _ => None,
        };
        let deposit_data = match deposit_data {
            Some(d) => d,
            None => return,
        };

        let wallet_name_clone = wallet_name.clone();
        let deposit_data_for_state = deposit_data.clone();
        leptos::task::spawn_local(async move {
            match js_interop::connect_wallet(&wallet_name_clone).await {
                crate::wallet_error::WalletResult::Success(pubkey) => {
                    log::info!(
                        "[deposit] close-deposit wallet connected: {} ({})",
                        wallet_name_clone,
                        pubkey
                    );
                    set_state.set(DepositPageState::CloseDepositWalletConnected(
                        deposit_data_for_state,
                        wallet_name_clone,
                        pubkey,
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!(
                        "[deposit] close-deposit wallet connect error: code={:?} msg={}",
                        e.code,
                        e.raw_message
                    );
                    app_components::show_toast(
                        &set_toast,
                        &crate::wallet_error::user_friendly_message(&e),
                        ToastType::Error,
                    );
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    app_components::show_toast(
                        &set_toast,
                        "Failed to connect wallet. Please try again.",
                        ToastType::Error,
                    );
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Close deposit (reclaim rent)
// ---------------------------------------------------------------------------

/// Create handler: build, sign, and send a close-deposit transaction.
pub fn make_close_deposit(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
    params: DepositParamsSignal,
) -> impl Fn(String, String) + Clone + Send + Sync + 'static {
    move |wallet_name: String, public_key: String| {
        let current_state = state.get();
        let deposit_data = match &current_state {
            DepositPageState::CloseDepositWalletConnected(d, _, _) => d.clone(),
            _ => return,
        };

        let attendee_id = match params.get() {
            Ok(p) => p.attendee_id.unwrap_or_default(),
            Err(_) => String::new(),
        };
        let event_id = extract_event_id_from_url().unwrap_or_default();

        let wallet_name_for_tx = wallet_name.clone();
        let pk_for_tx = public_key.clone();
        let deposit_data_for_state = deposit_data.clone();

        set_state.set(DepositPageState::CloseDepositSigning(
            deposit_data.clone(),
            wallet_name.clone(),
            public_key.clone(),
        ));

        leptos::task::spawn_local(async move {
            let req = CloseDepositRequest {
                event_id: event_id.clone(),
                attendee_id: attendee_id.clone(),
                wallet_address: pk_for_tx.clone(),
            };
            let close_resp = match api::close_deposit(&req).await {
                Ok(r) => r,
                Err(e) => {
                    log::error!("[deposit] close-deposit TX build failed: {e}");
                    app_components::show_toast(
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
                app_components::show_toast(
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

            // SEC-014
            let expected_cluster = crate::utils::get_cluster();
            if let Err(cluster_err) =
                crate::pages::escrow_init::check_wallet_cluster(&wallet_name_for_tx, &expected_cluster).await
            {
                log::error!("[deposit] cluster mismatch (close): {cluster_err}");
                app_components::show_toast(&set_toast, &cluster_err, ToastType::Error);
                return;
            }

            match crate::pages::escrow_init::simulate_transaction_js(
                &wallet_name_for_tx,
                &tx_b64,
            )
            .await
            {
                Ok(sim) if sim.ok => {}
                Ok(sim) => {
                    let err_msg = sim
                        .error
                        .unwrap_or_else(|| "Simulation failed".to_string());
                    log::error!("[deposit] close simulation failed: {err_msg}");
                    app_components::show_toast(
                        &set_toast,
                        &format!("Transaction would fail: {err_msg}"),
                        ToastType::Error,
                    );
                    return;
                }
                Err(e) => {
                    log::warn!("[deposit] simulate error (not blocking): {e}");
                }
            }

            match js_interop::sign_and_send_tx(&wallet_name_for_tx, &tx_b64).await {
                crate::wallet_error::WalletResult::Success(signature) => {
                    log::info!(
                        "[deposit] close-deposit TX sent, signature: {signature}",
                    );
                    set_state.set(DepositPageState::CloseDepositConfirmed(
                        deposit_data,
                        signature,
                    ));
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    log::error!(
                        "[deposit] close-deposit wallet sign+send error: code={:?} msg={}",
                        e.code,
                        e.raw_message
                    );
                    app_components::show_toast(
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
                    app_components::show_toast(
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
    }
}
