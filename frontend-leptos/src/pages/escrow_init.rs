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
    fn detected_wallets() -> Vec<String>;

    #[wasm_bindgen(js_name = "connectWallet")]
    fn connect_wallet_js_raw(wallet_name: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = "signAndSendTransaction")]
    fn sign_and_send_tx_js_raw(wallet_name: &str, transaction_b64: &str) -> js_sys::Promise;

    /// SEC-014: Detect wallet's connected cluster via genesis hash.
    /// Returns "devnet", "mainnet-beta", "testnet", "localnet", or null.
    #[wasm_bindgen(js_name = "getWalletCluster")]
    fn get_wallet_cluster_js_raw(wallet_name: &str) -> js_sys::Promise;

    /// Pre-sign simulation: simulates TX before requesting wallet signature.
    /// Returns JSON string: { ok: bool, skipped: bool, error: string?, logs: string[] }
    #[wasm_bindgen(js_name = "simulateTransactionB64")]
    fn simulate_tx_js_raw(wallet_name: &str, transaction_b64: &str) -> js_sys::Promise;
}

/// Detect installed Solana wallet extensions.
pub fn get_detected_wallets_js() -> Vec<String> {
    detected_wallets()
}

pub async fn connect_wallet_js(wallet_name: &str) -> crate::wallet_error::WalletResult {
    if wallet_name.is_empty() {
        log::warn!("[escrow-init] connect_wallet_js: empty wallet name");
        return crate::wallet_error::WalletResult::UnknownFailure;
    }
    let promise = connect_wallet_js_raw(wallet_name);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[escrow-init] connect_wallet_js error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}

pub async fn sign_and_send_tx_js(wallet_name: &str, transaction_b64: &str) -> crate::wallet_error::WalletResult {
    if wallet_name.is_empty() {
        log::warn!("[escrow-init] sign_and_send_tx_js: empty wallet name");
        return crate::wallet_error::WalletResult::UnknownFailure;
    }
    let promise = sign_and_send_tx_js_raw(wallet_name, transaction_b64);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => crate::wallet_error::parse_wallet_js_value(&val),
        Err(e) => {
            log::error!("[escrow-init] sign_and_send_tx_js error: {:?}", e);
            crate::wallet_error::WalletResult::UnknownFailure
        }
    }
}

/// SEC-014: Detect the wallet's connected cluster.
/// Returns the cluster name ("devnet", "mainnet-beta", etc.) or None if undetectable.
pub async fn get_wallet_cluster_js(wallet_name: &str) -> Option<String> {
    if wallet_name.is_empty() {
        log::warn!("[escrow-init] get_wallet_cluster_js: empty wallet name");
        return None;
    }
    let promise = get_wallet_cluster_js_raw(wallet_name);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => {
            if val.is_null() || val.is_undefined() {
                log::warn!("[escrow-init] get_wallet_cluster_js: wallet returned null");
                None
            } else {
                val.as_string()
            }
        }
        Err(e) => {
            log::error!("[escrow-init] get_wallet_cluster_js error: {:?}", e);
            None
        }
    }
}

/// SEC-014: Check if the wallet's cluster matches the expected cluster.
/// Returns Ok(()) if they match, or Err with a descriptive message.
pub async fn check_wallet_cluster(wallet_name: &str, expected_cluster: &str) -> Result<(), String> {
    match get_wallet_cluster_js(wallet_name).await {
        Some(wallet_cluster) => {
            if wallet_cluster == expected_cluster {
                log::info!(
                    "[escrow-init] cluster check passed: wallet={wallet_cluster}, expected={expected_cluster}"
                );
                Ok(())
            } else {
                let msg = format!(
                    "Wallet is on {wallet_cluster} but app expects {expected_cluster}. \
                     Switch your wallet network to {expected_cluster} and try again."
                );
                log::error!("[escrow-init] {msg}");
                Err(msg)
            }
        }
        None => {
            // Cannot detect cluster — allow through with a warning log.
            // Some wallets don't expose their RPC endpoint, so we can't check.
            log::warn!(
                "[escrow-init] cannot detect wallet cluster, skipping check (expected={expected_cluster})"
            );
            Ok(())
        }
    }
}

/// Result of a pre-sign simulation.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SimulateResult {
    pub ok: bool,
    #[serde(default)]
    pub skipped: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub logs: Vec<String>,
}

/// Simulate a transaction before requesting wallet signature.
/// Follows Solana Foundation Security Checklist: "Simulate first."
/// Returns Ok(SimulateResult) on success, or Err(msg) if simulation failed.
pub async fn simulate_transaction_js(wallet_name: &str, transaction_b64: &str) -> Result<SimulateResult, String> {
    if wallet_name.is_empty() {
        log::warn!("[simulate] empty wallet name, skipping");
        return Ok(SimulateResult { ok: true, skipped: true, error: None, logs: vec![] });
    }
    let promise = simulate_tx_js_raw(wallet_name, transaction_b64);
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(val) => {
            let json_str = val.as_string().unwrap_or_default();
            match serde_json::from_str::<SimulateResult>(&json_str) {
                Ok(result) => {
                    if result.ok {
                        log::info!("[simulate] TX simulation passed (skipped={})", result.skipped);
                    } else {
                        log::warn!("[simulate] TX simulation failed: {:?}", result.error);
                    }
                    Ok(result)
                }
                Err(e) => {
                    log::warn!("[simulate] failed to parse result: {e}, skipping");
                    Ok(SimulateResult { ok: true, skipped: true, error: None, logs: vec![] })
                }
            }
        }
        Err(e) => {
            log::warn!("[simulate] JS error: {e:?}, skipping");
            Ok(SimulateResult { ok: true, skipped: true, error: None, logs: vec![] })
        }
    }
}

// ===== State Machine =====

/// State machine for the escrow lifecycle (init → deactivate → close).
#[derive(Debug, Clone, PartialEq)]
pub enum EscrowInitState {
    /// No wallet connected yet.
    Idle,
    /// Wallet connected, ready to init escrow.
    WalletConnected {
        wallet_name: String,
        public_key: String,
    },
    /// Escrow init TX being signed.
    Initializing {
        wallet_name: String,
    },
    /// Escrow initialized on-chain — can deactivate.
    Done {
        escrow_address: String,
        vault_address: String,
        on_chain_event_id: u64,
        signature: String,
    },
    /// Deactivate TX being signed.
    Deactivating {
        wallet_name: String,
    },
    /// Escrow deactivated — vault still exists, can close.
    Deactivated {
        escrow_address: String,
        on_chain_event_id: u64,
    },
    /// Close event TX being signed.
    Closing {
        wallet_name: String,
    },
    /// Escrow closed — rent reclaimed, all on-chain accounts gone.
    Closed {
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
    form: ReadSignal<super::event_form::EventForm>,
    /// Writer to update form escrow fields on success.
    #[prop(name = "set_form")]
    set_form: WriteSignal<super::event_form::EventForm>,
    /// Writer for toast notifications.
    #[prop(name = "set_toast")]
    set_toast: WriteSignal<Option<components::ToastMessage>>,
) -> impl IntoView {
    // If event already has escrow, start in Done state
    let initial_state = {
        let f = form.get();
        match f.escrow_status {
            api::EscrowStatus::Initialized => EscrowInitState::Done {
                escrow_address: f.escrow_address.clone(),
                vault_address: String::new(),
                on_chain_event_id: f.on_chain_event_id.parse::<u64>().unwrap_or(0),
                signature: String::new(),
            },
            api::EscrowStatus::Deactivated => EscrowInitState::Deactivated {
                escrow_address: f.escrow_address.clone(),
                on_chain_event_id: f.on_chain_event_id.parse::<u64>().unwrap_or(0),
            },
            api::EscrowStatus::Closed => EscrowInitState::Closed {
                signature: String::new(),
            },
            _ => {
                if !f.escrow_address.is_empty() {
                    // Legacy: escrow_address set but no status — treat as Initialized
                    EscrowInitState::Done {
                        escrow_address: f.escrow_address.clone(),
                        vault_address: String::new(),
                        on_chain_event_id: f.on_chain_event_id.parse::<u64>().unwrap_or(0),
                        signature: String::new(),
                    }
                } else {
                    EscrowInitState::Idle
                }
            }
        }
    };
    let (state, set_state) = signal(initial_state);
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());

    // Wallet detection — poll with delays for late-injecting wallet extensions.
    {
        let set_dw = set_detected_wallets;
        leptos::task::spawn_local(async move {
            let mut wallets = get_detected_wallets_js();
            if wallets.is_empty() {
                for _ in 0..10 {
                    gloo_timers::future::TimeoutFuture::new(300).await;
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
    let is_deactivating =
        move || matches!(state.get(), EscrowInitState::Deactivating { .. });
    let is_deactivated =
        move || matches!(state.get(), EscrowInitState::Deactivated { .. });
    let is_closing = move || matches!(state.get(), EscrowInitState::Closing { .. });
    let is_closed = move || matches!(state.get(), EscrowInitState::Closed { .. });
    let is_error = move || matches!(state.get(), EscrowInitState::Error { .. });

    // Store event_id in a signal so reactive closures can clone it repeatedly.
    let (event_id_sig, _set_event_id) = signal(event_id.clone());
    // Persist wallet name across state transitions for deactivate/close flows.
    let (wallet_name_sig, set_wallet_name) = signal(String::new());
    let set_t = set_toast;
    let set_f = set_form;

    view! {
        // ===== Idle: wallet detection + connect buttons =====
        <Show when=is_idle>
            <div class="panel-box-dashed escrow-panel-mt">
                <div class="panel-label">
                    "On-Chain Escrow Setup"
                </div>
                <div class="panel-hint u-mb-sm">
                    "Single transaction: create vault ATA + initialize event escrow. Requires organizer wallet."
                </div>
                <Show when=move || !detected_wallets.get().is_empty() fallback=|| view! {
                    <div class="panel-hint">
                        "No Solana wallets detected. Install Phantom or another wallet extension."
                    </div>
                }>
                    <div class="flex-wrap-row">
                        {move || detected_wallets.get().iter().map(|wn| {
                            let wn_c = wn.clone();
                            let set_s = set_state;
                            let set_t = set_t;
                            let set_f = set_f;
                            let set_wn = set_wallet_name;
                            view! {
                                <button
                                    class="btn btn-outline btn-sm"
                                    on:click=move |_| {
                                        let wn = wn_c.clone();
                                        let set_s = set_s;
                                        let set_t = set_t;
                                        let set_f = set_f;
                                        let set_wn = set_wn;
                                        leptos::task::spawn_local(async move {
                                            match connect_wallet_js(&wn).await {
                                                crate::wallet_error::WalletResult::Success(pk) => {
                                                    log::info!("[escrow-init] connected {} pk={}", wn, &pk[..8.min(pk.len())]);
                                                    // Auto-fill organizer wallet in the form.
                                                    // Safe now: parent uses Show (not {move ||}), so this
                                                    // won't re-create the EscrowInitPanel.
                                                    set_f.update(|f| {
                                                        if f.organizer_wallet.is_empty() {
                                                            f.organizer_wallet = pk.clone();
                                                        }
                                                    });
                                                    set_wn.set(wn.clone());
                                                    set_s.set(EscrowInitState::WalletConnected {
                                                        wallet_name: wn,
                                                        public_key: pk,
                                                    });
                                                }
                                                crate::wallet_error::WalletResult::Error(e) => {
                                                    log::warn!("[escrow-init] wallet connect error: code={:?} msg={}", e.code, e.raw_message);
                                                    components::show_toast(
                                                        &set_t,
                                                        &crate::wallet_error::user_friendly_message(&e),
                                                        components::ToastType::Error,
                                                    );
                                                }
                                                crate::wallet_error::WalletResult::UnknownFailure => {
                                                    log::warn!("[escrow-init] wallet connect failed or rejected");
                                                    components::show_toast(
                                                        &set_t,
                                                        "Wallet connection failed",
                                                        components::ToastType::Error,
                                                    );
                                                }
                                            }
                                        });
                                    }
                                >
                                    {format!("Connect {}", wn)}
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
                    <div class="panel-box-dashed escrow-panel-mt">
                        <div class="flex-row-center u-mb-sm">
                            <div>
                                <div class="panel-label u-mb-0">
                                    "On-Chain Escrow Setup"
                                </div>
                                <div class="panel-hint u-mt-xs">
                                    {format!("Connected: {} ({})", wn, &pk[..8.min(pk.len())])}
                                </div>
                            </div>
                            <button
                                class="btn btn-outline btn-sm"
                                on:click=move |_| set_state.set(EscrowInitState::Idle)
                            >
                                "Disconnect"
                            </button>
                        </div>

                        // Single step: Initialize Escrow (vault ATA + event escrow in one TX)
                        <div class="step-card">
                            <div class="flex-row-center u-gap-lg">
                                <div>
                                    <div class="step-card-title">
                                        "Initialize Escrow"
                                    </div>
                                    <div class="step-card-desc">
                                        "Creates vault ATA + event escrow PDA in a single transaction."
                                    </div>
                                </div>
                                <button
                                    class="btn-primary btn-sm u-nowrap"
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
                                                // Include expected_updated_at for optimistic concurrency.
                                                // Prevents conflict if user later clicks Save on the main form.
                                                expected_updated_at: if f.updated_at.is_empty() { None } else { Some(f.updated_at.clone()) },
                                                ..Default::default()
                                            };
                                            match api::update_event(&eid, &save_body).await {
                                                Ok(updated) => {
                                                    // Sync form updated_at so subsequent Save clicks
                                                    // don't hit a stale-version conflict.
                                                    set_f.update(|f| {
                                                        f.updated_at = updated.updated_at.clone();
                                                    });
                                                    log::info!("[escrow-init] event saved, building escrow TX...");
                                                }
                                                Err(e) => {
                                                    log::error!("[escrow-init] failed to save event: {e}");
                                                    set_s.set(EscrowInitState::Error {
                                                        message: format!("Failed to save event: {e}"),
                                                    });
                                                    return;
                                                }
                                            }

                                            let req = api::InitEscrowRequest {
                                                event_id: eid.clone(),
                                            };
                                            match api::init_escrow(&req).await {
                                                Ok(resp) => {
                                                    // SEC-014: Verify wallet cluster matches expected network.
                                                    let expected_cluster = crate::utils::get_cluster();
                                                    if let Err(cluster_err) = check_wallet_cluster(&wn, &expected_cluster).await {
                                                        log::error!("[escrow-init] cluster mismatch: {cluster_err}");
                                                        set_s.set(EscrowInitState::Error {
                                                            message: cluster_err,
                                                        });
                                                        return;
                                                    }

                                                    log::info!("[escrow-init] escrow TX built, signing...");

                                                    // Pre-sign simulation (Solana Foundation Security Checklist).
                                                    match simulate_transaction_js(&wn, &resp.transaction).await {
                                                        Ok(sim) if sim.ok => {
                                                            if !sim.skipped {
                                                                log::info!("[escrow-init] simulation passed");
                                                            }
                                                        }
                                                        Ok(sim) => {
                                                            let err_msg = sim.error.unwrap_or_else(|| "Simulation failed".to_string());
                                                            log::error!("[escrow-init] simulation failed: {err_msg}");
                                                            set_s.set(EscrowInitState::Error {
                                                                message: format!("Transaction would fail: {err_msg}"),
                                                            });
                                                            return;
                                                        }
                                                        Err(e) => {
                                                            log::warn!("[escrow-init] simulation error (not blocking): {e}");
                                                        }
                                                    }

                                                    match sign_and_send_tx_js(&wn, &resp.transaction).await {
                                                        crate::wallet_error::WalletResult::Success(signature) => {
                                                            log::info!("[escrow-init] escrow TX confirmed: {}", signature);
                                                            set_f.update(|f| {
                                                                f.escrow_address = resp.escrow_address.clone();
                                                                if resp.on_chain_event_id > 0 {
                                                                    f.on_chain_event_id = resp.on_chain_event_id.to_string();
                                                                }
                                                                f.organizer_wallet = pk.clone();
                                                                f.escrow_status = api::EscrowStatus::Initialized;
                                                            });
                                                            set_s.set(EscrowInitState::Done {
                                                                escrow_address: resp.escrow_address.clone(),
                                                                vault_address: resp.vault_address,
                                                                on_chain_event_id: resp.on_chain_event_id,
                                                                signature,
                                                            });
                                                            // Persist escrow state to backend
                                                            let sync_body = api::UpdateEventBody {
                                                                escrow_address: Some(resp.escrow_address.clone()),
                                                                escrow_status: Some(api::EscrowStatus::Initialized),
                                                                on_chain_event_id: Some(resp.on_chain_event_id),
                                                                expected_updated_at: None, // skip concurrency check — escrow init is authoritative
                                                                ..Default::default()
                                                            };
                                                            match api::update_event(&eid, &sync_body).await {
                                                                Ok(_) => {
                                                                    log::info!("[escrow-init] escrow state persisted to backend");
                                                                }
                                                                Err(e) => {
                                                                    log::error!("[escrow-init] FAILED to persist escrow state: {e}");
                                                                    // Non-fatal — on-chain state is the source of truth
                                                                    components::show_toast(
                                                                        &set_t,
                                                                        "Escrow created on-chain but state sync failed — refresh to verify",
                                                                        components::ToastType::Warning,
                                                                    );
                                                                }
                                                            }
                                                            components::show_toast(
                                                                &set_t,
                                                                "Escrow initialized on-chain!",
                                                                components::ToastType::Success,
                                                            );
                                                        }
                                                        crate::wallet_error::WalletResult::Error(e) => {
                                                            let msg = crate::wallet_error::user_friendly_message(&e);
                                                            log::error!("[escrow-init] escrow TX error: code={:?} msg={}", e.code, e.raw_message);
                                                            set_s.set(EscrowInitState::Error {
                                                                message: msg,
                                                            });
                                                        }
                                                        crate::wallet_error::WalletResult::UnknownFailure => {
                                                            log::error!("[escrow-init] escrow TX rejected");
                                                            set_s.set(EscrowInitState::Error {
                                                                message: "Escrow transaction failed".to_string(),
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
                                                                    f.escrow_status = d.escrow_status.clone();
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
                                    "Create & Sign"
                                </button>
                            </div>
                        </div>
                    </div>
                }.into_any()
            }}
        </Show>

        // ===== Sync/Recover: no wallet needed, server-side on-chain verification =====
        <Show when=is_idle>
            <div class="panel-box-dashed escrow-sync-panel">
                <div class="panel-label escrow-sync-label">
                    "Escrow already exists on-chain?"
                </div>
                <div class="panel-hint u-mt-2xs escrow-sync-hint">
                    "If you already initialized escrow but the page lost sync, click below to verify on-chain and restore the state."
                </div>
                {move || {
                    let eid = event_id_sig.get();
                    let set_s = set_state;
                    let set_t = set_t;
                    let set_f = set_form;
                    let sync_eid = eid.clone();
                    view! {
                        <button
                            class="btn btn-outline btn-sm u-mt-xs escrow-sync-btn"
                            on:click=move |_| {
                                let eid = sync_eid.clone();
                                let set_s = set_s;
                                let set_t = set_t;
                                let set_f = set_f;
                                leptos::task::spawn_local(async move {
                                    log::info!("[escrow-sync] calling confirm-init for event: {eid}");
                                    let req = api::ConfirmEscrowInitRequest {
                                        event_id: eid.clone(),
                                    };
                                    match api::confirm_escrow_init(&req).await {
                                        Ok(resp) => {
                                            log::info!(
                                                "[escrow-sync] confirmed: escrow={} on_chain_event_id={}",
                                                resp.escrow_address,
                                                resp.on_chain_event_id
                                            );
                                            set_f.update(|f| {
                                                f.escrow_address = resp.escrow_address.clone();
                                                f.on_chain_event_id = resp.on_chain_event_id.to_string();
                                                f.escrow_status = api::EscrowStatus::Initialized;
                                            });
                                            set_s.set(EscrowInitState::Done {
                                                escrow_address: resp.escrow_address,
                                                vault_address: String::new(),
                                                on_chain_event_id: resp.on_chain_event_id,
                                                signature: String::new(),
                                            });
                                            components::show_toast(
                                                &set_t,
                                                "Escrow state synced from on-chain!",
                                                components::ToastType::Success,
                                            );
                                        }
                                        Err(e) => {
                                            log::error!("[escrow-sync] confirm-init failed: {e}");
                                            components::show_toast(
                                                &set_t,
                                                &format!("No escrow found on-chain: {e}"),
                                                components::ToastType::Error,
                                            );
                                        }
                                    }
                                });
                            }
                        >
                            "🔄 Sync from On-Chain"
                        </button>
                    }
                }}
            </div>
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
                    <div class="panel-box-dashed escrow-panel-mt">
                        <div class="flex-row-gap">
                            <span class="spinner spinner-sm"></span>
                            <span class="panel-label u-mb-0">
                                {format!("Initializing escrow via {}...", wn)}
                            </span>
                        </div>
                        <div class="panel-hint u-mt-2xs">
                            "Approve the transaction in your wallet."
                        </div>
                    </div>
                }.into_any()
            }}
        </Show>

        // ===== Done: escrow initialized — show info + Deactivate button =====
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
                let eid = event_id_sig.get();
                let wn = wallet_name_sig.get();
                let set_s = set_state;
                let set_t = set_t;
                let set_wn = set_wallet_name;
                let dw = detected_wallets.get();
                let has_wallet = !wn.is_empty();
                view! {
                    <div class="panel-success escrow-panel-mt-sm">
                        <div class="step-card-title badge-done">
                            "Escrow initialized on-chain"
                        </div>
                        <div class="panel-hint u-mt-2xs">
                            <span class="text-label">"Escrow: "</span>
                            <code class="code-xs">{ea}</code>
                        </div>
                        <div class="panel-hint u-mt-xs">
                            <span class="text-label">"Vault: "</span>
                            <code class="code-xs">{va}</code>
                        </div>
                        <div class="panel-hint u-mt-xs">
                            <span class="text-label">"On-chain event ID: "</span>
                            <code class="code-xs">{oeid}</code>
                        </div>
                        <div class="code-xs u-mt-2xs">
                            <a href=solscan target="_blank" rel="noopener" class="link-accent">
                                "View on Solscan ↗"
                            </a>
                        </div>
                        // Wallet connect or Deactivate button
                        {if has_wallet {
                            view! {
                                <div class="flex-wrap-row escrow-gap-sm u-mt-sm">
                                    <button
                                        class="btn btn-outline btn-sm btn-danger"
                                        on:click=move |_| {
                                            let eid = eid.clone();
                                            let wn = wn.clone();
                                            let set_s = set_s;
                                            let set_t = set_t;
                                            set_s.set(EscrowInitState::Deactivating { wallet_name: wn.clone() });
                                            leptos::task::spawn_local(async move {
                                                let req = api::DeactivateEventRequest { event_id: eid.clone() };
                                                match api::deactivate_event(&req).await {
                                                    Ok(resp) => {
                                                        // Pre-sign simulation.
                                                        match simulate_transaction_js(&wn, &resp.transaction).await {
                                                            Ok(sim) if sim.ok => {}
                                                            Ok(sim) => {
                                                                let err_msg = sim.error.unwrap_or_else(|| "Simulation failed".to_string());
                                                                log::error!("[escrow] deactivate simulation failed: {err_msg}");
                                                                set_s.set(EscrowInitState::Error { message: format!("Transaction would fail: {err_msg}") });
                                                                return;
                                                            }
                                                            Err(e) => { log::warn!("[escrow] simulate error (not blocking): {e}"); }
                                                        }
                                                        match sign_and_send_tx_js(&wn, &resp.transaction).await {
                                                            crate::wallet_error::WalletResult::Success(sig) => {
                                                                log::info!("[escrow] deactivate TX confirmed: {}", sig);
                                                                // Persist escrow_status=deactivated server-side
                                                                let sync_ok = match api::update_event(&eid.clone(), &api::UpdateEventBody {
                                                                    escrow_status: Some(api::EscrowStatus::Deactivated),
                                                                    expected_updated_at: None,
                                                                    ..Default::default()
                                                                }).await {
                                                                    Ok(_) => {
                                                                        log::info!("[escrow] escrow_status persisted as Deactivated");
                                                                        true
                                                                    }
                                                                    Err(e) => {
                                                                        log::error!("[escrow] FAILED to persist escrow_status=Deactivated: {e}");
                                                                        components::show_toast(
                                                                            &set_t,
                                                                            "Escrow deactivated on-chain but state sync failed — refresh to verify",
                                                                            components::ToastType::Warning,
                                                                        );
                                                                        false
                                                                    }
                                                                };
                                                                set_f.update(|f| {
                                                                    f.escrow_status = api::EscrowStatus::Deactivated;
                                                                });
                                                                set_s.set(EscrowInitState::Deactivated {
                                                                    escrow_address: String::new(),
                                                                    on_chain_event_id: 0,
                                                                });
                                                                if sync_ok {
                                                                    components::show_toast(
                                                                        &set_t,
                                                                        "Event escrow deactivated — no more deposits accepted",
                                                                        components::ToastType::Success,
                                                                    );
                                                                }
                                                            }
                                                            crate::wallet_error::WalletResult::Error(e) => {
                                                                let msg = crate::wallet_error::user_friendly_message(&e);
                                                                log::error!("[escrow] deactivate TX error: code={:?} msg={}", e.code, e.raw_message);
                                                                set_s.set(EscrowInitState::Error {
                                                                    message: msg,
                                                                });
                                                            }
                                                            crate::wallet_error::WalletResult::UnknownFailure => {
                                                                log::error!("[escrow] deactivate TX rejected");
                                                                set_s.set(EscrowInitState::Error {
                                                                    message: "Deactivate transaction failed".to_string(),
                                                                });
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        log::error!("[escrow] deactivate failed: {e}");
                                                        set_s.set(EscrowInitState::Error {
                                                            message: format!("Failed to deactivate: {e}"),
                                                        });
                                                    }
                                                }
                                            });
                                        }
                                    >
                                        "Deactivate Event"
                                    </button>
                                </div>
                            }.into_any()
                        } else {
                            // No wallet connected — show connect buttons
                            view! {
                                <div class="u-mt-sm">
                                    <div class="panel-hint escrow-text-muted u-mb-xs">
                                        "Connect wallet to deactivate or close this escrow."
                                    </div>
                                    <div class="flex-wrap-row escrow-gap-sm">
                                        {dw.iter().map(|w| {
                                            let wname = w.clone();
                                            let set_wn = set_wn;
                                            let set_t2 = set_t;
                                            view! {
                                                <button
                                                    class="btn btn-outline btn-sm"
                                                    on:click=move |_| {
                                                        let wn = wname.clone();
                                                        let set_wn = set_wn;
                                                        let set_t2 = set_t2;
                                                        leptos::task::spawn_local(async move {
                                                            match connect_wallet_js(&wn).await {
                                                                crate::wallet_error::WalletResult::Success(_pk) => {
                                                                    log::info!("[escrow] connected {} for deactivate", wn);
                                                                    set_wn.set(wn);
                                                                }
                                                                crate::wallet_error::WalletResult::Error(e) => {
                                                                    components::show_toast(
                                                                        &set_t2,
                                                                        &crate::wallet_error::user_friendly_message(&e),
                                                                        components::ToastType::Error,
                                                                    );
                                                                }
                                                                crate::wallet_error::WalletResult::UnknownFailure => {
                                                                    components::show_toast(
                                                                        &set_t2,
                                                                        "Wallet connection failed",
                                                                        components::ToastType::Error,
                                                                    );
                                                                }
                                                            }
                                                        });
                                                    }
                                                >
                                                    {format!("Connect {}", wname)}
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </div>
                            }.into_any()
                        }}
                        <div class="panel-hint escrow-text-muted u-mt-xs">
                            "Deactivate stops new deposits. Refunds still allowed. Then close to reclaim rent SOL."
                        </div>
                    </div>
                }.into_any()
            }}
        </Show>

        // ===== Deactivating: spinner =====
        <Show when=is_deactivating>
            {move || {
                let s = state.get();
                let wn = match &s {
                    EscrowInitState::Deactivating { wallet_name } => wallet_name.clone(),
                    _ => String::new(),
                };
                view! {
                    <div class="panel-box-dashed escrow-panel-mt">
                        <div class="flex-row-gap">
                            <span class="spinner spinner-sm"></span>
                            <span class="panel-label u-mb-0">
                                {format!("Deactivating escrow via {}...", wn)}
                            </span>
                        </div>
                        <div class="panel-hint u-mt-2xs">
                            "Approve the deactivate transaction in your wallet."
                        </div>
                    </div>
                }.into_any()
            }}
        </Show>

        // ===== Deactivated: escrow inactive — show Close Event button =====
        <Show when=is_deactivated>
            {move || {
                let eid = event_id_sig.get();
                let wn = wallet_name_sig.get();
                let set_s = set_state;
                let set_t = set_t;
                view! {
                    <div class="panel-warning">
                        <div class="step-card-title escrow-title-warning">
                            "Escrow deactivated"
                        </div>
                        <div class="panel-hint u-mt-2xs">
                            "Event escrow is no longer accepting deposits. Refunds are still allowed."
                        </div>
                        <div class="panel-hint escrow-hint-warning u-mt-xs">
                            "Vault must be empty (all USDC refunded/claimed) before closing."
                        </div>
                        <div class="flex-wrap-row escrow-gap-sm u-mt-sm">
                            <button
                                class="btn btn-primary btn-sm"
                                on:click=move |_| {
                                    let eid = eid.clone();
                                    let wn = wn.clone();
                                    let set_s = set_s;
                                    let set_t = set_t;
                                    set_s.set(EscrowInitState::Closing { wallet_name: wn.clone() });
                                    leptos::task::spawn_local(async move {
                                        let req = api::CloseEventRequest { event_id: eid.clone() };
                                        match api::close_event(&req).await {
                                            Ok(resp) => {
                                                // Pre-sign simulation.
                                                match simulate_transaction_js(&wn, &resp.transaction).await {
                                                    Ok(sim) if sim.ok => {}
                                                    Ok(sim) => {
                                                        let err_msg = sim.error.unwrap_or_else(|| "Simulation failed".to_string());
                                                        log::error!("[escrow] close simulation failed: {err_msg}");
                                                        set_s.set(EscrowInitState::Error { message: format!("Transaction would fail: {err_msg}") });
                                                        return;
                                                    }
                                                    Err(e) => { log::warn!("[escrow] simulate error (not blocking): {e}"); }
                                                }
                                                match sign_and_send_tx_js(&wn, &resp.transaction).await {
                                                    crate::wallet_error::WalletResult::Success(sig) => {
                                                        log::info!("[escrow] close_event TX confirmed: {}", sig);
                                                        // Persist escrow_status=closed server-side
                                                        let sync_ok = match api::update_event(&eid.clone(), &api::UpdateEventBody {
                                                            escrow_status: Some(api::EscrowStatus::Closed),
                                                            expected_updated_at: None,
                                                            ..Default::default()
                                                        }).await {
                                                            Ok(_) => {
                                                                log::info!("[escrow] escrow_status persisted as Closed");
                                                                true
                                                            }
                                                            Err(e) => {
                                                                log::error!("[escrow] FAILED to persist escrow_status=Closed: {e}");
                                                                components::show_toast(
                                                                    &set_t,
                                                                    "Escrow closed on-chain but state sync failed — refresh to verify",
                                                                    components::ToastType::Warning,
                                                                );
                                                                false
                                                            }
                                                        };
                                                        // Clear escrow fields from form
                                                        set_f.update(|f| {
                                                            f.escrow_address = String::new();
                                                            f.on_chain_event_id = String::new();
                                                            f.escrow_status = api::EscrowStatus::Closed;
                                                        });
                                                        set_s.set(EscrowInitState::Closed { signature: sig });
                                                        if sync_ok {
                                                            components::show_toast(
                                                                &set_t,
                                                                "Event escrow closed — rent SOL reclaimed!",
                                                                components::ToastType::Success,
                                                            );
                                                        }
                                                    }
                                                    crate::wallet_error::WalletResult::Error(e) => {
                                                        let msg = crate::wallet_error::user_friendly_message(&e);
                                                        log::error!("[escrow] close_event TX error: code={:?} msg={}", e.code, e.raw_message);
                                                        set_s.set(EscrowInitState::Error {
                                                            message: msg,
                                                        });
                                                    }
                                                    crate::wallet_error::WalletResult::UnknownFailure => {
                                                        log::error!("[escrow] close_event TX rejected");
                                                        set_s.set(EscrowInitState::Error {
                                                            message: "Close event transaction failed".to_string(),
                                                        });
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                log::error!("[escrow] close_event failed: {e}");
                                                set_s.set(EscrowInitState::Error {
                                                    message: format!("Failed to close event: {e}"),
                                                });
                                            }
                                        }
                                    });
                                }
                            >
                                "Close Event & Reclaim Rent"
                            </button>
                        </div>
                    </div>
                }.into_any()
            }}
        </Show>

        // ===== Closing: spinner =====
        <Show when=is_closing>
            {move || {
                let s = state.get();
                let wn = match &s {
                    EscrowInitState::Closing { wallet_name } => wallet_name.clone(),
                    _ => String::new(),
                };
                view! {
                    <div class="panel-box-dashed escrow-panel-mt">
                        <div class="flex-row-gap">
                            <span class="spinner spinner-sm"></span>
                            <span class="panel-label u-mb-0">
                                {format!("Closing escrow via {}...", wn)}
                            </span>
                        </div>
                        <div class="panel-hint u-mt-2xs">
                            "Approve the close transaction. Rent SOL will be returned to your wallet."
                        </div>
                    </div>
                }.into_any()
            }}
        </Show>

        // ===== Closed: escrow gone, rent reclaimed =====
        <Show when=is_closed>
            {move || {
                let s = state.get();
                let sig = match &s {
                    EscrowInitState::Closed { signature } => signature.clone(),
                    _ => String::new(),
                };
                let solscan = crate::utils::solscan_tx_url(&sig, &crate::utils::get_cluster());
                view! {
                    <div class="panel-success escrow-panel-mt-sm">
                        <div class="step-card-title badge-done">
                            "Escrow closed — rent reclaimed!"
                        </div>
                        <div class="panel-hint u-mt-2xs">
                            "The event escrow and vault have been closed on-chain. Rent SOL has been returned to your wallet."
                        </div>
                        <div class="code-xs u-mt-2xs">
                            <a href=solscan target="_blank" rel="noopener" class="link-accent">
                                "View on Solscan ↗"
                            </a>
                        </div>
                        <div class="flex-wrap-row escrow-gap-sm u-mt-sm">
                            <button
                                class="btn btn-outline btn-sm"
                                on:click=move |_| {
                                    let eid = event_id_sig.get();
                                    let set_s = set_state;
                                    let set_f = set_form;
                                    let set_t = set_t;
                                    leptos::task::spawn_local(async move {
                                        // Reset escrow fields server-side so re-init is allowed
                                        let ts = form.get().updated_at.clone();
                                        let reset_body = api::UpdateEventBody {
                                            escrow_address: Some(String::new()),
                                            escrow_status: Some(api::EscrowStatus::None),
                                            on_chain_event_id: Some(0),
                                            expected_updated_at: if ts.is_empty() { None } else { Some(ts) },
                                            ..Default::default()
                                        };
                                        match api::update_event(&eid, &reset_body).await {
                                            Ok(resp) => {
                                                log::info!("[escrow-reset] escrow fields reset to none — ready for re-init");
                                                set_f.update(|f| {
                                                    f.escrow_address = String::new();
                                                    f.on_chain_event_id = String::new();
                                                    f.organizer_wallet = String::new();
                                                    f.escrow_status = api::EscrowStatus::None;
                                                    if !resp.updated_at.is_empty() {
                                                        f.updated_at = resp.updated_at.clone();
                                                    }
                                                });
                                                set_s.set(EscrowInitState::Idle);
                                                components::show_toast(&set_t, "Escrow reset — ready to initialize a new escrow", components::ToastType::Success);
                                            }
                                            Err(e) => {
                                                log::error!("[escrow-reset] failed to reset escrow: {e}");
                                                components::show_toast(&set_t, &format!("Failed to reset escrow: {e}"), components::ToastType::Error);
                                            }
                                        }
                                    });
                                }
                            >
                                "Re-initialize Escrow"
                            </button>
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
                    <div class="panel-error escrow-panel-mt">
                        <div class="panel-hint escrow-text-error">
                            {msg.to_string()}
                        </div>
                        <button
                            class="btn btn-outline btn-sm u-mt-2xs"
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
