//! Deposit page — attendees pay their event deposit (USDC or THB).
//!
//! Public page (no auth required) accessed via `/deposit/:attendee_id?event_id=xxx`.
//! Flow:
//! 1. Extract attendee_id from URL path, event_id from query params
//! 2. GET /api/deposit/status/{attendee_id}?event_id={event_id}
//! 3. If deposit not enabled → show message
//! 4. If already deposited → show status
//! 5. If not deposited → show dual-track payment options (USDC / THB)
//! 6. USDC: calls deposit_usdc() → shows Solana Pay URL (copy-to-clipboard)
//! 7. THB: text input for slip URL (MVP) → calls upload_thb_slip()

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;

use crate::api::{
    self, DepositStatusResponse, ThbSlipUploadRequest, UsdcDepositRequest,
};
use crate::components::{self, Toast, ToastType};
use crate::utils::format_timestamp;
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// JS interop
// ---------------------------------------------------------------------------

#[wasm_bindgen(module = "/js/qr_generate.js")]
extern "C" {
    /// Copy text to the system clipboard.
    #[wasm_bindgen(js_name = "copyToClipboard")]
    fn copy_to_clipboard_js(text: &str) -> bool;
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
    /// USDC QR URL generated and ready to display.
    UsdcQrReady(DepositStatusResponse, String),
    /// THB slip is being uploaded.
    ThbUploading(DepositStatusResponse),
    /// THB slip uploaded successfully.
    ThbUploaded,
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

    // --- USDC Pay handler ---
    let handle_pay_usdc = move || {
        let current_state = state.get();
        let (deposit_data, attendee_id, event_id) = match &current_state {
            DepositPageState::ChoosePayment(d) => {
                // Re-extract attendee_id and event_id from URL
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
                    log::info!("[deposit] USDC deposit initiated, solana_pay_url received");
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

        let slip_url = slip_url_input.get();
        if slip_url.trim().is_empty() {
            components::show_toast(
                &set_toast,
                "Please enter the slip URL.",
                ToastType::Warning,
            );
            return;
        }

        // Transition to uploading state
        set_state.set(DepositPageState::ThbUploading(deposit_data));

        leptos::task::spawn_local(async move {
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
                    // Re-extract deposit data to go back to ChoosePayment
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
                                    <a href="/" class="btn btn-primary" style="margin-top:1rem;">"Go Home"</a>
                                </div>
                            }
                                .into_any()
                        }

                        // ===== Choose Payment =====
                        DepositPageState::ChoosePayment(data) => {
                            let data_clone = data.clone();
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
                                                {format!("{} USDC", data.deposit_amount_usdc)}
                                            </span>
                                        </div>
                                        <p style="color:var(--text-secondary);font-size:0.9rem;margin-bottom:1rem;">
                                            "Pay via Solana Pay. Enter your wallet address, then click the button to generate a payment link."
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
                                            class="btn btn-primary btn-block"
                                            on:click=move |_| handle_pay_usdc()
                                        >
                                            "Pay with USDC"
                                        </button>
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
                                        <div style="margin-bottom:0.75rem;">
                                            <input
                                                type="text"
                                                class="form-input"
                                                placeholder="Paste slip URL (upload coming soon)"
                                                prop:value=move || slip_url_input.get()
                                                on:input=move |ev| {
                                                    let val = event_target_value(&ev);
                                                    set_slip_url_input.set(val);
                                                }
                                                style="width:100%;padding:0.6rem 0.8rem;border-radius:6px;border:1px solid var(--border-color,rgba(255,255,255,0.2));background:var(--bg-secondary,#1a1a2e);color:var(--text-primary,#fff);font-size:0.9rem;"
                                            />
                                            <p style="font-size:0.75rem;color:var(--text-secondary);margin-top:0.3rem;">
                                                "📎 File upload coming soon — paste the URL for now."
                                            </p>
                                        </div>
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

                        // ===== USDC QR Ready =====
                        DepositPageState::UsdcQrReady(data, pay_url) => {
                            let pay_url_display = pay_url.clone();
                            let pay_url_copy = pay_url.clone();
                            let copied = pay_url_copied.get();
                            let copy_btn_text = if copied { "✅ Copied!" } else { "📋 Copy Link" };
                            let copy_btn_class = if copied { "btn btn-success btn-sm" } else { "btn btn-outline btn-sm" };
                            view! {
                                <div class="card" style="margin-top:1.5rem;text-align:center;width:100%;max-width:480px;">
                                    <div class="card-header">
                                        <h2 class="card-title">"🪙 USDC Payment Ready"</h2>
                                        <span class="badge badge-info">
                                            {format!("{} USDC", data.deposit_amount_usdc)}
                                        </span>
                                    </div>
                                    <p style="color:var(--text-secondary);font-size:0.9rem;margin-bottom:1rem;">
                                        "Open this link in a Solana-compatible wallet to complete payment:"
                                    </p>
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
