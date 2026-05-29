//! Deposit page — attendees pay their event deposit (USDC or THB).
//!
//! Public page (no auth required) accessed via `/deposit/:attendee_id?event_id=xxx`.

pub mod already_deposited;
pub mod close_deposit;
pub mod components;
pub mod js_interop;
pub mod refund;
pub mod thb_payment;
pub mod types;
pub mod usdc_payment;

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params;

use crate::api::{
    self, CloseDepositRequest, ConfirmDepositResponse, RefundTxRequest,
    ThbSlipUploadRequest, UsdcDepositRequest,
};
use crate::components::{self as app_components, Toast, ToastType};
use crate::icons::{wallet_icon_name, Icon, IconName};

use self::types::*;

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
    let (toast, set_toast) = signal(None::<app_components::ToastMessage>);
    let (slip_url_input, set_slip_url_input) = signal(String::new());
    let (wallet_input, set_wallet_input) = signal(String::new());
    let (pay_url_copied, set_pay_url_copied) = signal(false);
    let (bank_account_input, set_bank_account_input) = signal(String::new());
    let (bank_name_input, set_bank_name_input) = signal(String::new());
    let (account_name_input, set_account_name_input) = signal(String::new());
    let (show_bank_dropdown, set_show_bank_dropdown) = signal(false);
    let (payment_choice, set_payment_choice) = signal(None::<PaymentChoice>);
    let file_input_ref = NodeRef::<leptos::html::Input>::new();
    let (slip_preview, set_slip_preview) = signal(None::<String>);

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

        let event_id = extract_event_id_from_url();

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
    });

    // --- Detect installed wallets on mount ---
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());
    {
        let set_dw = set_detected_wallets;
        leptos::task::spawn_local(async move {
            let mut wallets = self::js_interop::get_detected_wallets();
            if wallets.is_empty() {
                for _ in 0..10 {
                    gloo::timers::future::TimeoutFuture::new(300).await;
                    wallets = self::js_interop::get_detected_wallets();
                    if !wallets.is_empty() {
                        break;
                    }
                }
            }
            log::info!("[deposit] detected wallets: {:?}", wallets);
            set_dw.set(wallets);
        });
    }

    // --- Inline handler: connect wallet (deposit flow) ---
    let handle_connect_wallet = {
        let state = state;
        let set_state = set_state;
        let set_toast = set_toast;
        move |wallet_name: String| {
            let deposit_data = match &state.get() {
                DepositPageState::ChoosePayment(d) => d.clone(),
                _ => return,
            };

            let wallet_name_clone = wallet_name.clone();
            let deposit_data_for_state = deposit_data.clone();
            leptos::task::spawn_local(async move {
                match self::js_interop::connect_wallet(&wallet_name_clone).await {
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
    };

    // --- Inline handler: send deposit TX ---
    let handle_send_deposit = {
        let state = state;
        let set_state = set_state;
        let set_toast = set_toast;
        let params = params.clone();
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
                // Step 1: Initiate deposit with backend
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

                // Step 2: Extract callback URL from Solana Pay URL
                let callback_url = if deposit_resp.solana_pay_url.starts_with("solana:") {
                    &deposit_resp.solana_pay_url[7..]
                } else {
                    &deposit_resp.solana_pay_url
                };

                // Step 3: Fetch serialized TX from callback
                let tx_b64 = match self::js_interop::fetch_tx_from_callback(callback_url).await {
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

                // Pre-sign simulation
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

                // Step 4: Sign and send via wallet
                match self::js_interop::sign_and_send_tx(&wallet_name_for_tx, &tx_b64).await {
                    crate::wallet_error::WalletResult::Success(signature) => {
                        log::info!("[deposit] TX sent, signature: {}", signature);

                        // Step 5: Record TX signature with backend
                        let _ = api::record_deposit_tx(&event_id_str, &attendee_id, &signature).await;

                        // Step 6: Start polling for confirmation
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
    };

    // --- Inline handler: poll for deposit confirmation ---
    let handle_poll_confirmation = {
        let state = state;
        let set_state = set_state;
        let set_toast = set_toast;
        move |event_id: String, attendee_id: String, _tx_sig: String| {
            leptos::task::spawn_local(async move {
                let mut attempts = 0u32;
                let max_attempts = 30;
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

                app_components::show_toast(
                    &set_toast,
                    "Confirmation is taking longer than expected. Your deposit may still be processing.",
                    ToastType::Warning,
                );
            });
        }
    };

    // --- Inline handler: pay USDC via QR ---
    let handle_pay_usdc_qr = {
        let state = state;
        let set_state = set_state;
        let set_toast = set_toast;
        let wallet_input = wallet_input;
        let params = params.clone();
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
    };

    // --- Inline handler: upload THB slip ---
    #[allow(clippy::too_many_arguments)]
    let handle_upload_slip = {
        let state = state;
        let set_state = set_state;
        let set_toast = set_toast;
        let slip_url_input = slip_url_input;
        let bank_account_input = bank_account_input;
        let bank_name_input = bank_name_input;
        let account_name_input = account_name_input;
        let file_input_ref = file_input_ref.clone();
        let params = params.clone();
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
                        match self::js_interop::read_file_as_data_url(&js_val).await {
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
    };

    // --- Inline handler: copy payment URL ---
    let handle_copy_url = {
        let set_toast = set_toast;
        let set_pay_url_copied = set_pay_url_copied;
        move |url: String| {
            if self::js_interop::copy_to_clipboard(&url) {
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
    };

    // --- Inline handler: refund connect wallet ---
    let handle_refund_connect_wallet = {
        let state = state;
        let set_state = set_state;
        let set_toast = set_toast;
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
                match self::js_interop::connect_wallet(&wallet_name_clone).await {
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
    };

    // --- Inline handler: claim refund ---
    let handle_claim_refund = {
        let state = state;
        let set_state = set_state;
        let set_toast = set_toast;
        let params = params.clone();
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

                // Pre-sign simulation
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

                match self::js_interop::sign_and_send_tx(&wallet_name_for_tx, &tx_b64).await {
                    crate::wallet_error::WalletResult::Success(signature) => {
                        log::info!("[deposit] refund TX sent, signature: {}", signature);
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
    };

    // --- Inline handler: close deposit connect wallet ---
    let handle_close_deposit_connect_wallet = {
        let state = state;
        let set_state = set_state;
        let set_toast = set_toast;
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
                match self::js_interop::connect_wallet(&wallet_name_clone).await {
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
    };

    // --- Inline handler: close deposit (reclaim rent) ---
    let handle_close_deposit = {
        let state = state;
        let set_state = set_state;
        let set_toast = set_toast;
        let params = params.clone();
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

                // Pre-sign simulation
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

                match self::js_interop::sign_and_send_tx(&wallet_name_for_tx, &tx_b64).await {
                    crate::wallet_error::WalletResult::Success(signature) => {
                        log::info!(
                            "[deposit] close-deposit TX sent, signature: {}",
                            signature
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
    };

    let has_wallets = move || !detected_wallets.get().is_empty();

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
                                                                                self::js_interop::navigate_to("/");
                                    });
                                }
                            >
                                "Sign out"
                            </button>
                        </div>
                    }.into_any(),
                    None => ().into_any(),
                }}

                // Event context header
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

                // Step progress indicator
                {move || {
                    let s = state.get();
                    match deposit_step(&s) {
                        Some((flow, current, total)) => {
                            components::deposit_stepper(flow, current, total)
                        }
                        None => ().into_any(),
                    }
                }}

                {move || {
                    let s = state.get();
                    match s {
                        // ===== Loading =====
                        DepositPageState::Loading => already_deposited::loading_view(),

                        // ===== Error =====
                        DepositPageState::Error(msg) => already_deposited::error_view(&msg),

                        // ===== Not Enabled =====
                        DepositPageState::NotEnabled => already_deposited::not_enabled_view(),

                        // ===== Already Deposited =====
                        DepositPageState::AlreadyDeposited(data) => {
                            already_deposited::already_deposited_view(&data, &set_state)
                        }

                        // ===== Choose Payment =====
                        DepositPageState::ChoosePayment(data) => {
                            let data_clone = data.clone();
                            let wallets = detected_wallets.get();
                            let is_dev_mode = data.dev_mode;
                            let usdc_accepted = data.usdc_deposits_accepted;
                            let show_usdc = is_dev_mode && usdc_accepted;
                            let deposit_deadline = data_clone.deposit_deadline_hours;
                            let deadline_expired = data_clone.deadline_expired;
                            let can_reclaim = data_clone.in_person_available.unwrap_or(false);
                            view! {
                                // Deadline expired banner
                                {if deadline_expired && !can_reclaim {
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
                                    view! {
                                        <p class="subtitle subtitle-lg">
                                            "Choose your preferred payment method to secure your spot."
                                        </p>

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

                                    {move || match payment_choice.get() {
                                        None => view! {
                                            <div class="deposit-method-cards">
                                                {if show_usdc {
                                                    view! {
                                                        <div class="deposit-method-card"
                                                            on:click=move |_| set_payment_choice.set(Some(PaymentChoice::Usdc))>
                                                            <div class="deposit-method-header">
                                                                <h3 class="deposit-method-title">"Pay with USDC"</h3>
                                                                <span class="badge badge-info">
                                                                    {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                                                </span>
                                                            </div>
                                                            <p class="deposit-method-desc">
                                                                "Pay via Solana wallet or QR code."
                                                            </p>
                                                            <span class="badge badge-muted">"🧪 Dev Mode"</span>
                                                        </div>
                                                    }.into_any()
                                                } else {
                                                    view! { <div></div> }.into_any()
                                                }}
                                                <div class="deposit-method-card"
                                                    on:click=move |_| set_payment_choice.set(Some(PaymentChoice::Thb))>
                                                    <div class="deposit-method-header">
                                                        <h3 class="deposit-method-title">"Pay with THB"</h3>
                                                        <span class="badge badge-warning">
                                                            {format!("{} THB", data_clone.deposit_amount_thb)}
                                                        </span>
                                                    </div>
                                                    <p class="deposit-method-desc">
                                                        "Transfer via PromptPay and upload your payment slip."
                                                    </p>
                                                </div>
                                            </div>
                                        }.into_any(),

                                        Some(PaymentChoice::Usdc) => view! {
                                            <button class="btn btn-outline btn-sm" style="margin-bottom:0.75rem"
                                                on:click=move |_| set_payment_choice.set(None)>
                                                "← Change method"
                                            </button>

                                    {if show_usdc {
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

                                                {if has_wallets() {
                                                    let wallets_for_click = wallets.clone();
                                                    let hcw = handle_connect_wallet.clone();
                                                    view! {
                                                        <div class="wallet-list">
                                                            <p class="wallet-prompt">
                                                                <Icon icon=IconName::Link class="icon-sm" />" Connect your Solana wallet:"
                                                            </p>
                                                            {wallets_for_click.into_iter().map(|w| {
                                                                let w_clone = w.clone();
                                                                let wallet_icon = wallet_icon_name(&w);
                                                                let hcw = hcw.clone();
                                                                view! {
                                                                    <button
                                                                        class="btn btn-primary btn-block wallet-btn-inner"
                                                                        on:click={
                                                                            let w = w.clone();
                                                                            move |_| hcw(w.clone())
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
                                                        on:click={
                                                            let h = handle_pay_usdc_qr.clone();
                                                            move |_| h()
                                                        }
                                                    >
                                                        "Generate QR Code"
                                                    </button>
                                                </div>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }}

                                        }.into_any(),

                                        Some(PaymentChoice::Thb) => view! {
                                            <button class="btn btn-outline btn-sm" style="margin-bottom:0.75rem"
                                                on:click=move |_| set_payment_choice.set(None)>
                                                "← Change method"
                                            </button>

                                    <div class="card">
                                        <div class="card-header">
                                            <h2 class="card-title">"Pay with THB"</h2>
                                            <span class="badge badge-warning">
                                                {format!("{} THB", data_clone.deposit_amount_thb)}
                                            </span>
                                        </div>

                                        <div style="background:rgba(255,255,255,0.04);border-radius:var(--radius);padding:0.75rem;margin-bottom:1rem;">
                                            <p style="font-size:0.8rem;font-weight:600;color:var(--text-primary);margin-bottom:0.5rem;">
                                                "How to pay:"
                                            </p>
                                            <ol style="font-size:0.75rem;color:var(--text-secondary);margin:0;padding-left:1.25rem;display:flex;flex-direction:column;gap:0.25rem;">
                                                <li>"Scan the QR code below with your banking app"</li>
                                                <li>"Transfer "{format!("{} THB", data_clone.deposit_amount_thb)}" via PromptPay"</li>
                                                <li>"Take a screenshot of the payment confirmation"</li>
                                                <li>"Upload the screenshot below and submit"</li>
                                            </ol>
                                        </div>

                                        <p class="hint-desc" style="margin-bottom:0.75rem;">
                                            "Transfer via PromptPay and upload your payment slip."
                                        </p>

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
                                                        let pp_qr_string = self::js_interop::generate_promptpay_qr(&pp_id, pp_amount, &pp_reference);
                                                        let pp_qr_image = pp_qr_string.and_then(|s| self::js_interop::generate_qr_data_url(&s, 256));
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
                                                                            self::js_interop::download_data_url(&url_for_save, &format!("promptpay-{pp_amount_for_filename}THB-qr.png"));
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
                                                    }}
                                                    <p class="qr-hint-text">
                                                        "Open your banking app → Scan QR → Pay"
                                                    </p>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }}

                                        <div class="dep-divider-section">
                                            <label class="upload-label">
                                                <Icon icon=IconName::Clip class="icon-sm" />" Upload payment slip"
                                            </label>
                                            <p style="color:var(--text-secondary);font-size:0.75rem;margin:0.25rem 0 0.5rem;">
                                                "Take a screenshot or photo of your transfer confirmation. Max 3MB (JPEG, PNG, WebP)."
                                            </p>
                                            <input
                                                type="file"
                                                accept="image/jpeg,image/png,image/webp"
                                                node_ref=file_input_ref
                                                class="file-input-styled"
                                                on:change=move |_| {
                                                    let file_ref = file_input_ref.clone();
                                                    leptos::task::spawn_local(async move {
                                                        if let Some(el) = file_ref.get() {
                                                            let js_val: wasm_bindgen::JsValue = el.into();
                                                            let preview = self::js_interop::read_file_as_data_url(&js_val).await;
                                                            set_slip_preview.set(preview);
                                                        }
                                                    });
                                                }
                                            />

                                            {move || {
                                                match slip_preview.get() {
                                                    Some(url) => view! {
                                                        <div class="slip-preview-container">
                                                            <img src=&url class="slip-preview-img" />
                                                            <button
                                                                class="slip-preview-remove"
                                                                on:click=move |_| {
                                                                    set_slip_preview.set(None);
                                                                    if let Some(el) = file_input_ref.get() {
                                                                        el.set_value("");
                                                                    }
                                                                }
                                                            >
                                                                "✕"
                                                            </button>
                                                        </div>
                                                    }.into_any(),
                                                    None => view! { <div></div> }.into_any(),
                                                }
                                            }}

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
                                                disabled=move || {
                                                    bank_account_input.get().trim().is_empty()
                                                    || bank_name_input.get().trim().is_empty()
                                                    || account_name_input.get().trim().is_empty()
                                                }
                                                on:click={
                                                    let h = handle_upload_slip.clone();
                                                    move |_| h()
                                                }
                                            >
                                                "Upload Slip"
                                            </button>
                                            <p class="hint-desc" style="margin-top:0.25rem;text-align:center;font-size:0.75rem;">
                                                "Bank account, bank name, and account holder name are required for refund."
                                            </p>
                                        </div>
                                    </div>

                                        }.into_any(),
                                    }}

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

                        // ===== Wallet Connected =====
                        DepositPageState::WalletConnected(data, wallet_name, public_key) => {
                            usdc_payment::wallet_connected_view(
                                &data,
                                &wallet_name,
                                &public_key,
                                handle_send_deposit.clone(),
                                set_state,
                            )
                        }

                        // ===== Awaiting Confirmation =====
                        DepositPageState::AwaitingConfirmation(data, wallet_name, tx_sig) => {
                            usdc_payment::awaiting_confirmation_view(
                                &data,
                                &wallet_name,
                                &tx_sig,
                                state,
                                set_state,
                                set_toast,
                                params.clone(),
                                handle_poll_confirmation.clone(),
                            )
                        }

                        // ===== Deposit Confirmed =====
                        DepositPageState::DepositConfirmed(data, tx_sig) => {
                            usdc_payment::deposit_confirmed_view(&data, &tx_sig, params.clone())
                        }

                        // ===== USDC QR Ready =====
                        DepositPageState::UsdcQrReady(data, pay_url) => {
                            usdc_payment::usdc_qr_ready_view(
                                &data,
                                &pay_url,
                                pay_url_copied,
                                state,
                                set_state,
                                set_toast,
                                params.clone(),
                                handle_copy_url.clone(),
                            )
                        }

                        // ===== THB Uploading =====
                        DepositPageState::ThbUploading(_) => {
                            thb_payment::thb_uploading_view()
                        }

                        // ===== THB Uploaded =====
                        DepositPageState::ThbUploaded(attendee_id, event_id, _event_slug) => {
                            thb_payment::thb_uploaded_view(&attendee_id, &event_id)
                        }

                        // ===== Refund: Choose Wallet =====
                        DepositPageState::RefundChooseWallet(data) => {
                            let wallets = detected_wallets.get();
                            refund::refund_choose_wallet_view(
                                &data,
                                &wallets,
                                set_state,
                                handle_refund_connect_wallet.clone(),
                            )
                        }

                        // ===== Refund: Wallet Connected =====
                        DepositPageState::RefundWalletConnected(data, wallet_name, public_key) => {
                            refund::refund_wallet_connected_view(
                                &data,
                                &wallet_name,
                                &public_key,
                                set_state,
                                handle_claim_refund.clone(),
                            )
                        }

                        // ===== Refund: Signing =====
                        DepositPageState::RefundSigning(data, _, _) => {
                            refund::refund_signing_view(&data)
                        }

                        // ===== Refund: Confirmed =====
                        DepositPageState::RefundConfirmed(data, tx_sig) => {
                            refund::refund_confirmed_view(&data, &tx_sig)
                        }

                        // ===== Close Deposit: Choose Wallet =====
                        DepositPageState::CloseDepositChooseWallet(data) => {
                            let wallets = detected_wallets.get();
                            close_deposit::close_deposit_choose_wallet_view(
                                &data,
                                &wallets,
                                set_state,
                                handle_close_deposit_connect_wallet.clone(),
                            )
                        }

                        // ===== Close Deposit: Wallet Connected =====
                        DepositPageState::CloseDepositWalletConnected(data, wallet_name, public_key) => {
                            close_deposit::close_deposit_wallet_connected_view(
                                &data,
                                &wallet_name,
                                &public_key,
                                set_state,
                                handle_close_deposit.clone(),
                            )
                        }

                        // ===== Close Deposit: Signing =====
                        DepositPageState::CloseDepositSigning(_, _, _) => {
                            close_deposit::close_deposit_signing_view()
                        }

                        // ===== Close Deposit: Confirmed =====
                        DepositPageState::CloseDepositConfirmed(data, tx_sig) => {
                            close_deposit::close_deposit_confirmed_view(&data, &tx_sig)
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
