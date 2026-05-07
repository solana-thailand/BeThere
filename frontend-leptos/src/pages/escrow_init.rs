//! Escrow initialization panel — single-TX flow.
//!
//! Extracted from `events_page.rs` to keep file sizes manageable.
//! Handles wallet detection, connection, and single-transaction escrow
//! initialization (vault ATA + event escrow in one TX).

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::api;
use crate::components;

// ===== Solana Wallet JS Interop =====

#[wasm_bindgen(module = "/js/solana_wallet.js")]
extern "C" {
    #[wasm_bindgen(js_name = "getDetectedWallets")]
    fn get_detected_wallets_js() -> Vec<String>;

    #[wasm_bindgen(js_name = "connectWallet")]
    fn connect_wallet_js_raw(wallet_name: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = "signAndSendTransaction")]
    fn sign_and_send_tx_js_raw(wallet_name: &str, transaction_b64: &str) -> js_sys::Promise;
}

async fn connect_wallet_js(wallet_name: &str) -> Option<String> {
    if wallet_name.is_empty() {
        log::warn!("[escrow-init] connect_wallet_js: empty wallet name");
        return None;
    }
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
            log::error!("[escrow-init] connect_wallet_js error: {:?}", e);
            None
        }
    }
}

async fn sign_and_send_tx_js(wallet_name: &str, transaction_b64: &str) -> Option<String> {
    if wallet_name.is_empty() {
        log::warn!("[escrow-init] sign_and_send_tx_js: empty wallet name");
        return None;
    }
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
            log::error!("[escrow-init] sign_and_send_tx_js error: {:?}", e);
            None
        }
    }
}

// ===== State Machine =====

/// State machine for the single-TX escrow initialization flow.
#[derive(Debug, Clone, PartialEq)]
pub enum EscrowInitState {
    /// No wallet connected yet.
    Idle,
    /// Wallet connected, ready to init escrow.
    WalletConnected {
        wallet_name: String,
        public_key: String,
    },
    /// Escrow TX being signed.
    Initializing {
        wallet_name: String,
    },
    /// Escrow initialized on-chain.
    Done {
        escrow_address: String,
        vault_address: String,
        on_chain_event_id: u64,
        signature: String,
    },
    /// Error during any step.
    Error {
        message: String,
    },
}

// ===== Form State (mirrors EventForm fields needed) =====

/// Form fields that the escrow init panel updates on success.
#[derive(Debug, Clone, Default)]
pub struct EscrowFormFields {
    pub escrow_address: String,
    pub on_chain_event_id: String,
}

// ===== Component =====

/// Escrow initialization panel component.
///
/// Single-TX flow: creates vault ATA + event escrow in one transaction.
/// Requires wallet connection. Updates form fields on success.
#[component]
pub fn EscrowInitPanel(
    /// Event ID being edited.
    #[prop(name = "event_id")]
    event_id: String,
    /// Reader for current form state.
    #[prop(name = "form")]
    form: ReadSignal<super::events_page::EventForm>,
    /// Writer to update form escrow fields on success.
    #[prop(name = "set_form")]
    set_form: WriteSignal<super::events_page::EventForm>,
    /// Writer for toast notifications.
    #[prop(name = "set_toast")]
    set_toast: WriteSignal<Option<components::ToastMessage>>,
) -> impl IntoView {
    let (state, set_state) = signal(EscrowInitState::Idle);
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());

    // Wallet detection — poll with delays for late-injecting wallet extensions.
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
            set_dw.set(wallets);
        });
    }

    // Helper: check if state matches a variant (reactive — reads signal).
    let is_idle = move || matches!(state.get(), EscrowInitState::Idle);
    let is_wallet_connected =
        move || matches!(state.get(), EscrowInitState::WalletConnected { .. });
    let is_initializing =
        move || matches!(state.get(), EscrowInitState::Initializing { .. });
    let is_done = move || matches!(state.get(), EscrowInitState::Done { .. });
    let is_error = move || matches!(state.get(), EscrowInitState::Error { .. });

    // Store event_id in a signal so reactive closures can clone it repeatedly.
    let (event_id_sig, _set_event_id) = signal(event_id.clone());
    let set_t = set_toast;
    let set_f = set_form;

    view! {
        // ===== Idle: wallet detection + connect buttons =====
        <Show when=is_idle>
            <div style="margin-top:1rem;padding:0.75rem;border:1px dashed var(--border);border-radius:8px;background:var(--bg-secondary)">
                <div style="font-size:0.85rem;font-weight:600;color:var(--text-primary);margin-bottom:0.5rem">
                    "⛓ On-Chain Escrow Setup"
                </div>
                <div style="font-size:0.75rem;color:var(--text-secondary);margin-bottom:0.75rem">
                    "Single transaction: create vault ATA + initialize event escrow. Requires organizer wallet."
                </div>
                <Show when=move || !detected_wallets.get().is_empty() fallback=|| view! {
                    <div style="font-size:0.75rem;color:var(--text-secondary)">
                        "No Solana wallets detected. Install Phantom or another wallet extension."
                    </div>
                }>
                    <div style="display:flex;flex-wrap:wrap;gap:0.5rem">
                        {move || detected_wallets.get().iter().map(|wn| {
                            let wn_c = wn.clone();
                            let set_s = set_state;
                            let set_t = set_t;
                            let set_f = set_f;
                            view! {
                                <button
                                    class="btn btn-outline"
                                    style="font-size:0.8rem;padding:0.4rem 0.8rem"
                                    on:click=move |_| {
                                        let wn = wn_c.clone();
                                        let set_s = set_s;
                                        let set_t = set_t;
                                        let set_f = set_f;
                                        leptos::task::spawn_local(async move {
                                            match connect_wallet_js(&wn).await {
                                                Some(pk) => {
                                                    log::info!("[escrow-init] connected {} pk={}", wn, &pk[..8.min(pk.len())]);
                                                    // Auto-fill organizer wallet in the form.
                                                    // Safe now: parent uses Show (not {move ||}), so this
                                                    // won't re-create the EscrowInitPanel.
                                                    set_f.update(|f| {
                                                        if f.organizer_wallet.is_empty() {
                                                            f.organizer_wallet = pk.clone();
                                                        }
                                                    });
                                                    set_s.set(EscrowInitState::WalletConnected {
                                                        wallet_name: wn,
                                                        public_key: pk,
                                                    });
                                                }
                                                None => {
                                                    log::warn!("[escrow-init] wallet connect failed or rejected");
                                                    components::show_toast(
                                                        &set_t,
                                                        "Wallet connection rejected",
                                                        components::ToastType::Error,
                                                    );
                                                }
                                            }
                                        });
                                    }
                                >
                                    {format!("🔗 Connect {}", wn)}
                                </button>
                            }
                        }).collect_view()}
                    </div>
                </Show>
            </div>
        </Show>

        // ===== Wallet Connected: show init escrow button =====
        <Show when=is_wallet_connected>
            {move || {
                let s = state.get();
                let (wn, pk) = match &s {
                    EscrowInitState::WalletConnected { wallet_name, public_key } => (wallet_name.clone(), public_key.clone()),
                    _ => (String::new(), String::new()),
                };
                let eid = event_id_sig.get();
                let form = form;
                let set_s = set_state;
                let set_t = set_t;
                let set_f = set_f;
                view! {
                    <div style="margin-top:1rem;padding:0.75rem;border:1px dashed var(--border);border-radius:8px;background:var(--bg-secondary)">
                        <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:0.75rem">
                            <div>
                                <div style="font-size:0.85rem;font-weight:600;color:var(--text-primary)">
                                    "⛓ On-Chain Escrow Setup"
                                </div>
                                <div style="font-size:0.75rem;color:var(--text-secondary);margin-top:0.2rem">
                                    {format!("Connected: {} ({})", wn, &pk[..8.min(pk.len())])}
                                </div>
                            </div>
                            <button
                                class="btn btn-outline"
                                style="font-size:0.7rem;padding:0.2rem 0.5rem"
                                on:click=move |_| set_state.set(EscrowInitState::Idle)
                            >
                                "Disconnect"
                            </button>
                        </div>

                        // Single step: Initialize Escrow (vault ATA + event escrow in one TX)
                        <div style="padding:0.5rem 0.75rem;border:1px solid var(--border);border-radius:6px;background:var(--bg-primary)">
                            <div style="display:flex;align-items:center;justify-content:space-between;gap:1rem">
                                <div>
                                    <div style="font-size:0.8rem;font-weight:600;color:var(--text-primary)">
                                        "Initialize Escrow"
                                    </div>
                                    <div style="font-size:0.7rem;color:var(--text-secondary)">
                                        "Creates vault ATA + event escrow PDA in a single transaction."
                                    </div>
                                </div>
                                <button
                                    class="btn-primary"
                                    style="white-space:nowrap;font-size:0.8rem;padding:0.4rem 0.8rem"
                                    on:click=move |_| {
                                        let eid = eid.clone();
                                        let pk = pk.clone();
                                        let wn = wn.clone();
                                        let set_s = set_s;
                                        let set_t = set_t;
                                        let set_f = set_f;
                                        let form = form;
                                        set_s.set(EscrowInitState::Initializing { wallet_name: wn.clone() });
                                        leptos::task::spawn_local(async move {
                                            // Save the full form so the backend has all fields
                                            // (deposit_enabled, deposit_amount_usdc, organizer_wallet, etc.)
                                            let f = form.get();
                                            let save_body = api::UpdateEventBody {
                                                organizer_wallet: Some(pk.clone()),
                                                deposit_enabled: Some(f.deposit_enabled),
                                                deposit_amount_usdc: Some((f.deposit_amount_usdc.parse::<f64>().unwrap_or(0.0) * 1_000_000.0) as u64),
                                                deposit_amount_thb: Some(f.deposit_amount_thb.parse::<u64>().unwrap_or(0)),
                                                refund_deadline_hours: Some(f.refund_deadline_hours.parse::<u32>().unwrap_or(0)),
                                                ..Default::default()
                                            };
                                            if let Err(e) = api::update_event(&eid, &save_body).await {
                                                log::error!("[escrow-init] failed to save event: {e}");
                                                set_s.set(EscrowInitState::Error {
                                                    message: format!("Failed to save event: {e}"),
                                                });
                                                return;
                                            }
                                            log::info!("[escrow-init] event saved, building escrow TX...");

                                            let req = api::InitEscrowRequest {
                                                event_id: eid.clone(),
                                            };
                                            match api::init_escrow(&req).await {
                                                Ok(resp) => {
                                                    log::info!("[escrow-init] escrow TX built, signing...");
                                                    match sign_and_send_tx_js(&wn, &resp.transaction).await {
                                                        Some(signature) => {
                                                            log::info!("[escrow-init] escrow TX confirmed: {}", signature);
                                                            set_f.update(|f| {
                                                                f.escrow_address = resp.escrow_address.clone();
                                                                if resp.on_chain_event_id > 0 {
                                                                    f.on_chain_event_id = resp.on_chain_event_id.to_string();
                                                                }
                                                                f.organizer_wallet = pk.clone();
                                                            });
                                                            set_s.set(EscrowInitState::Done {
                                                                escrow_address: resp.escrow_address,
                                                                vault_address: resp.vault_address,
                                                                on_chain_event_id: resp.on_chain_event_id,
                                                                signature,
                                                            });
                                                            components::show_toast(
                                                                &set_t,
                                                                "Escrow initialized on-chain!",
                                                                components::ToastType::Success,
                                                            );
                                                        }
                                                        None => {
                                                            log::error!("[escrow-init] escrow TX rejected");
                                                            set_s.set(EscrowInitState::Error {
                                                                message: "Escrow transaction rejected or failed".to_string(),
                                                            });
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let err_msg = format!("{e}");
                                                    // If escrow already exists, reload event and show success
                                                    if err_msg.contains("already has escrow") {
                                                        log::info!("[escrow-init] escrow already exists, reloading event...");
                                                        match api::get_event_detail(&eid).await {
                                                            Ok(data) => {
                                                                let d = &data.event;
                                                                set_f.update(|f| {
                                                                    f.escrow_address = d.escrow_address.clone();
                                                                    f.on_chain_event_id = if d.on_chain_event_id > 0 {
                                                                        d.on_chain_event_id.to_string()
                                                                    } else {
                                                                        f.on_chain_event_id.clone()
                                                                    };
                                                                    f.organizer_wallet = d.organizer_wallet.clone();
                                                                });
                                                                set_s.set(EscrowInitState::Done {
                                                                    escrow_address: d.escrow_address.clone(),
                                                                    vault_address: String::new(),
                                                                    on_chain_event_id: d.on_chain_event_id,
                                                                    signature: String::new(),
                                                                });
                                                                components::show_toast(
                                                                    &set_t,
                                                                    "Escrow already initialized",
                                                                    components::ToastType::Success,
                                                                );
                                                            }
                                                            Err(re) => {
                                                                log::error!("[escrow-init] failed to reload event: {re}");
                                                                set_s.set(EscrowInitState::Error {
                                                                    message: format!("Escrow exists but failed to reload: {re}"),
                                                                });
                                                            }
                                                        }
                                                    } else {
                                                        log::error!("[escrow-init] init_escrow failed: {err_msg}");
                                                        set_s.set(EscrowInitState::Error {
                                                            message: format!("Failed to build escrow TX: {err_msg}"),
                                                        });
                                                    }
                                                }
                                            }
                                        });
                                    }
                                >
                                    "⚡ Create & Sign"
                                </button>
                            </div>
                        </div>
                    </div>
                }.into_any()
            }}
        </Show>

        // ===== Initializing: spinner =====
        <Show when=is_initializing>
            {move || {
                let s = state.get();
                let wn = match &s {
                    EscrowInitState::Initializing { wallet_name } => wallet_name.clone(),
                    _ => String::new(),
                };
                view! {
                    <div style="margin-top:1rem;padding:0.75rem;border:1px dashed var(--border);border-radius:8px;background:var(--bg-secondary)">
                        <div style="display:flex;align-items:center;gap:0.5rem">
                            <span class="spinner spinner-sm"></span>
                            <span style="font-size:0.85rem;color:var(--text-primary)">
                                {format!("Initializing escrow via {}...", wn)}
                            </span>
                        </div>
                        <div style="font-size:0.75rem;color:var(--text-secondary);margin-top:0.25rem">
                            "Approve the transaction in your wallet."
                        </div>
                    </div>
                }.into_any()
            }}
        </Show>

        // ===== Done: success panel =====
        <Show when=is_done>
            {move || {
                let s = state.get();
                let (ea, va, oeid, sig) = match &s {
                    EscrowInitState::Done { escrow_address, vault_address, on_chain_event_id, signature } => {
                        (escrow_address.clone(), vault_address.clone(), *on_chain_event_id, signature.clone())
                    }
                    _ => (String::new(), String::new(), 0u64, String::new()),
                };
                let solscan = crate::utils::solscan_tx_url(&sig, &crate::utils::get_cluster());
                view! {
                    <div style="margin-top:0.75rem;padding:0.5rem 0.75rem;border:1px solid var(--success,green);border-radius:6px;background:rgba(0,128,0,0.05)">
                        <div style="font-size:0.8rem;color:var(--success,green);font-weight:600">
                            "✅ Escrow initialized on-chain"
                        </div>
                        <div style="font-size:0.75rem;margin-top:0.25rem">
                            <span style="color:var(--text-secondary)">"Escrow: "</span>
                            <code style="font-size:0.7rem">{ea}</code>
                        </div>
                        <div style="font-size:0.75rem;margin-top:0.2rem">
                            <span style="color:var(--text-secondary)">"Vault: "</span>
                            <code style="font-size:0.7rem">{va}</code>
                        </div>
                        <div style="font-size:0.75rem;margin-top:0.2rem">
                            <span style="color:var(--text-secondary)">"On-chain event ID: "</span>
                            <code style="font-size:0.7rem">{oeid}</code>
                        </div>
                        <div style="font-size:0.7rem;margin-top:0.25rem">
                            <a href=solscan target="_blank" rel="noopener" style="color:var(--accent)">
                                "View on Solscan ↗"
                            </a>
                        </div>
                    </div>
                }.into_any()
            }}
        </Show>

        // ===== Error: retry =====
        <Show when=is_error>
            {move || {
                let s = state.get();
                let msg = match &s {
                    EscrowInitState::Error { message } => message.clone(),
                    _ => String::new(),
                };
                view! {
                    <div style="margin-top:1rem;padding:0.5rem 0.75rem;border:1px solid var(--error,red);border-radius:6px;background:rgba(255,0,0,0.05)">
                        <div style="font-size:0.8rem;color:var(--error,red)">
                            {format!("❌ {msg}")}
                        </div>
                        <button
                            class="btn btn-outline"
                            style="font-size:0.7rem;padding:0.2rem 0.5rem;margin-top:0.25rem"
                            on:click=move |_| set_state.set(EscrowInitState::Idle)
                        >
                            "Retry"
                        </button>
                    </div>
                }.into_any()
            }}
        </Show>
    }
}
