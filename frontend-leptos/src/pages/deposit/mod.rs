//! Deposit page — attendees pay their event deposit (USDC or THB).
//!
//! Public page (no auth required) accessed via `/deposit/:attendee_id?event_id=xxx`.

pub mod already_deposited;
pub mod choose_payment;
pub mod close_deposit;
pub mod components;
pub mod handlers;
pub mod js_interop;
pub mod refund;
pub mod thb_payment;
pub mod types;
pub mod usdc_payment;

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params;

use crate::api;
use crate::components::{self as app_components, Toast};

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
            if let Ok(resp) = crate::api::fetch::get(&url, &[]).await {
                if resp.status() == 200 {
                    if let Ok(data) = crate::api::fetch::response_json::<serde_json::Value>(&resp).await {
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
                    } else if let Some(status) = &data.status {
                        if status.rejected {
                            set_state.set(DepositPageState::ThbRejected(data));
                        } else {
                            set_state.set(DepositPageState::AlreadyDeposited(data));
                        }
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
                    gloo_timers::future::TimeoutFuture::new(300).await;
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

    // --- Handler closures (extracted to handlers.rs) ---
    let handle_connect_wallet = handlers::make_connect_wallet(state, set_state, set_toast);
    let handle_send_deposit = handlers::make_send_deposit(state, set_state, set_toast, params.clone());
    let handle_poll_confirmation = handlers::make_poll_confirmation(state, set_state, set_toast);
    let handle_pay_usdc_qr = handlers::make_pay_usdc_qr(state, set_state, set_toast, wallet_input, params.clone());
    let handle_upload_slip = handlers::make_upload_slip(
        state, set_state, set_toast,
        slip_url_input, bank_account_input, bank_name_input, account_name_input,
        file_input_ref.clone(), params.clone(),
    );
    let handle_copy_url = handlers::make_copy_url(set_toast, set_pay_url_copied);
    let handle_qr_poll = handlers::make_qr_poll_confirmation(state, set_state, params.clone());
    let handle_refund_connect_wallet = handlers::make_refund_connect_wallet(state, set_state, set_toast);
    let handle_claim_refund = handlers::make_claim_refund(state, set_state, set_toast, params.clone());
    let handle_close_deposit_connect_wallet = handlers::make_close_deposit_connect_wallet(state, set_state, set_toast);
    let handle_close_deposit = handlers::make_close_deposit(state, set_state, set_toast, params.clone());

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
                            <span class="dep-note-text">
                                {format!("Welcome, {email}")}
                            </span>
                            <button
                                class="btn btn-outline btn-xs"
                                on:click=move |_| {
                                    leptos::task::spawn_local(async move {
                                        let _ = crate::api::fetch::post("/api/auth/logout", &[], None).await;
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
                    let pc = payment_choice.get();
                    match deposit_step(&s, pc) {
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
                            choose_payment::choose_payment_view(
                                data.clone(),
                                detected_wallets,
                                has_wallets,
                                payment_choice,
                                set_payment_choice,
                                wallet_input,
                                set_wallet_input,
                                slip_url_input,
                                set_slip_url_input,
                                file_input_ref.clone(),
                                slip_preview,
                                set_slip_preview,
                                bank_account_input,
                                set_bank_account_input,
                                bank_name_input,
                                set_bank_name_input,
                                account_name_input,
                                set_account_name_input,
                                show_bank_dropdown,
                                set_show_bank_dropdown,
                                handle_connect_wallet.clone(),
                                handle_pay_usdc_qr.clone(),
                                handle_upload_slip.clone(),
                            ).into_any()
                        }

                        // ===== Wallet Connected =====
                        DepositPageState::WalletConnected(data, wallet_name, public_key) => {
                            usdc_payment::wallet_connected_view(
                                &data,
                                &wallet_name,
                                &public_key,
                                handle_send_deposit.clone(),
                                set_state,
                                set_payment_choice,
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
                                handle_copy_url.clone(),
                                handle_qr_poll.clone(),
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

                        // ===== THB Rejected =====
                        DepositPageState::ThbRejected(data) => {
                            thb_payment::thb_rejected_view(&data, set_state, set_payment_choice)
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
