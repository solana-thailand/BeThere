//! Admin escrow management — deactivate, claim forfeited, close event.
//!
//! Shows escrow lifecycle actions for an event that has on-chain escrow
//! initialized. Requires organizer wallet connection to sign transactions.
//!
//! Flow:
//! 1. Connect organizer wallet
//! 2. Deactivate event (stops new deposits, allows refunds)
//! 3. Claim forfeited (transfer no-show deposits to organizer)
//! 4. Close event (reclaim rent, close escrow PDA)

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::api;
use crate::components::{self, ToastType};
use crate::icons::{Icon, IconName};

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

async fn connect_wallet_js(wallet_name: &str) -> crate::wallet_error::WalletResult {
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

async fn sign_and_send_tx_js(wallet_name: &str, transaction_b64: &str) -> crate::wallet_error::WalletResult {
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

// ===== Types =====

/// Escrow lifecycle actions.
#[derive(Debug, Clone, Copy, PartialEq)]
enum EscrowAction {
    Deactivate,
    ClaimForfeited,
    CloseEvent,
}

impl EscrowAction {
    fn label(&self) -> &'static str {
        match self {
            Self::Deactivate => "Deactivate Event",
            Self::ClaimForfeited => "Claim Forfeited",
            Self::CloseEvent => "Close Event",
        }
    }

    fn icon(&self) -> IconName {
        match self {
            Self::Deactivate => IconName::Pause,
            Self::ClaimForfeited => IconName::Coin,
            Self::CloseEvent => IconName::Lock,
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::Deactivate => "Stops new deposits. Refunds still allowed. Required before closing.",
            Self::ClaimForfeited => "Transfer forfeited deposits (no-shows) to your USDC account.",
            Self::CloseEvent => "Reclaim rent and close the escrow account. Requires empty vault.",
        }
    }

    fn button_class(&self) -> &'static str {
        match self {
            Self::Deactivate => "btn btn-outline",
            Self::ClaimForfeited => "btn-primary",
            Self::CloseEvent => "btn btn-outline",
        }
    }

    fn button_label(&self) -> &'static str {
        match self {
            Self::Deactivate => "Deactivate Event",
            Self::ClaimForfeited => "Claim Funds",
            Self::CloseEvent => "Close & Reclaim",
        }
    }

    fn loading_label(&self) -> &'static str {
        match self {
            Self::Deactivate => "Deactivating...",
            Self::ClaimForfeited => "Claiming...",
            Self::CloseEvent => "Closing...",
        }
    }
}

// ===== Shared Step Card Component =====

/// Renders one escrow lifecycle step card.
///
/// Handles 4 visual states: done (✓), signing (spinner), disabled (greyed),
/// and actionable (button). The confirm-danger pattern (Step 3 Close Event)
/// uses `set_confirming` to toggle between first-click and confirm-click.
#[component]
fn EscrowStepCard(
    action: EscrowAction,
    step_number: u8,
    done: bool,
    signing: bool,
    ready: bool,
    confirming: bool,
    on_trigger: WriteSignal<Option<EscrowAction>>,
    set_confirming: Option<WriteSignal<bool>>,
) -> impl IntoView {
    let card_class = if done {
        "step-card step-card-done"
    } else {
        "step-card"
    };
    let step_symbol = match step_number {
        1 => "①",
        2 => "②",
        3 => "③",
        _ => "",
    };

    // Pre-compute button config for the actionable state.
    // This avoids complex closure captures inside view! branches.
    let is_confirm_step = set_confirming.is_some();

    view! {
        <div class=card_class>
            <div class="step-card-info">
                <div class="step-card-title-icon">
                    <Icon icon=action.icon() class="icon-sm"/>
                    <span class="step-card-title">{format!("Step {}: {}", step_number, action.label())}</span>
                </div>
                <div class="step-card-desc">{action.description()}</div>
            </div>
            {if done {
                view! {
                    <span class="badge-done admin-escrow-badge-done">
                        "✓ Done"
                    </span>
                }.into_any()
            } else if signing {
                view! {
                    <div class="step-spinner">
                        <span class="spinner spinner-sm"></span>
                        <span>{action.loading_label()}</span>
                    </div>
                }.into_any()
            } else if !ready {
                // Disabled state — step not yet available
                view! {
                    <div class="step-card-actions step-card-disabled">
                        <span class="step-number">{step_symbol}</span>
                        <button
                            class=action.button_class()
                            class:step-card-disabled=true
                            disabled=true
                        >
                            {action.button_label()}
                        </button>
                    </div>
                }.into_any()
            } else if !is_confirm_step {
                // Simple action — no confirm-danger pattern
                let trigger = on_trigger.clone();
                view! {
                    <div class="step-card-actions">
                        <span class="step-number">{step_symbol}</span>
                        <button
                            class=action.button_class()
                            on:click=move |_| trigger.set(Some(action))
                        >
                            {action.button_label()}
                        </button>
                    </div>
                }.into_any()
            } else {
                // Confirm-danger pattern (Step 3: Close Event)
                // First click → show "⚠ Confirm Close?", second click → execute
                let trigger = on_trigger.clone();
                let set_cc = set_confirming.unwrap();
                let set_cc_reset = set_cc.clone();
                let (btn_class, btn_label) = if confirming {
                    ("btn btn-confirm-danger".to_string(), "⚠ Confirm Close?".to_string())
                } else {
                    (action.button_class().to_string(), action.button_label().to_string())
                };
                view! {
                    <div class="step-card-actions">
                        <span class="step-number">{step_symbol}</span>
                        <button
                            class=btn_class
                            on:click=move |_| {
                                if !confirming {
                                    set_cc.set(true);
                                    // Auto-reset confirm after 5s
                                    let reset = set_cc_reset.clone();
                                    gloo::timers::callback::Timeout::new(5000, move || {
                                        reset.set(false);
                                    }).forget();
                                } else {
                                    set_cc.set(false);
                                    trigger.set(Some(action));
                                }
                            }
                        >
                            {btn_label}
                        </button>
                    </div>
                }.into_any()
            }}
        </div>
    }
}

#[component]
pub fn AdminEscrow(
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    active_event_id: ReadSignal<Option<String>>,
) -> impl IntoView {
    // Wallet info (persists across actions)
    let (wallet_name, set_wallet_name) = signal(String::new());
    let (wallet_pk, set_wallet_pk) = signal(String::new());

    // Track completed actions
    let (completed_actions, set_completed_actions) = signal(Vec::<EscrowAction>::new());

    // Signing state — Some(action) while waiting for wallet signature
    let (signing_action, set_signing_action) = signal(None::<EscrowAction>);

    // Action trigger — set to trigger async execution
    let (action_to_execute, set_action_to_execute) = signal(None::<EscrowAction>);

    // Per-action results — persists Solscan links across steps
    let (action_results, set_action_results) = signal(Vec::<(EscrowAction, Result<String, String>)>::new());

    // Step ordering — Deactivate first, then Claim (optional) and Close.
    // Claim Forfeited is skippable: if no deposits exist, on-chain close_event
    // validates accounting independently (total_deposited == total_refunded + total_forfeited).
    let (step1_done, set_step1_done) = signal(false);
    let (_step2_done, set_step2_done) = signal(false);
    let (confirm_close, set_confirm_close) = signal(false);

    // Reset state when event changes — also pre-populate step progress
    // from server-side escrow_status so the UI reflects reality.
    {
        let set_wn = set_wallet_name.clone();
        let set_wp = set_wallet_pk.clone();
        let set_ca = set_completed_actions.clone();
        let set_sa = set_signing_action.clone();
        let set_ar = set_action_results.clone();
        let set_s1 = set_step1_done.clone();
        let set_s2 = set_step2_done.clone();
        let set_cc = set_confirm_close.clone();
        Effect::new(move |_| {
            let eid = active_event_id.get();
            set_wn.set(String::new());
            set_wp.set(String::new());
            set_ca.set(Vec::new());
            set_sa.set(None);
            set_ar.set(Vec::new());
            set_s1.set(false);
            set_s2.set(false);
            set_cc.set(false);

            // Fetch server-side escrow_status to pre-populate step state.
            // This handles the case where escrow was deactivated in a previous
            // session — the UI should show Step 1 as already done.
            if let Some(ref event_id) = eid {
                let set_s1 = set_s1.clone();
                let set_s2 = set_s2.clone();
                let set_ca = set_ca.clone();
                let event_id = event_id.clone();
                leptos::task::spawn_local(async move {
                    match api::get_event_detail(&event_id).await {
                        Ok(detail) => {
                            match detail.event.escrow_status {
                                api::EscrowStatus::Deactivated => {
                                    log::info!("[admin-escrow] escrow already deactivated on server — pre-completing step 1");
                                    set_s1.set(true);
                                    set_ca.update(|v| v.push(EscrowAction::Deactivate));
                                }
                                api::EscrowStatus::Closed | api::EscrowStatus::Cancelled => {
                                    log::info!("[admin-escrow] escrow already closed/cancelled — pre-completing all steps");
                                    set_s1.set(true);
                                    set_s2.set(true);
                                    set_ca.update(|v| {
                                        v.push(EscrowAction::Deactivate);
                                        v.push(EscrowAction::ClaimForfeited);
                                        v.push(EscrowAction::CloseEvent);
                                    });
                                }
                                _ => {
                                    log::info!("[admin-escrow] escrow status: {:?} — starting from step 1", detail.event.escrow_status.as_str());
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("[admin-escrow] failed to fetch event for escrow status: {e}");
                        }
                    }
                });
            }
        });
    }

    // Detect available wallets — poll sync detection with delays to wait
    // for late-injecting wallet extensions (Phantom injects asynchronously).
    let (wallets, set_wallets) = signal(Vec::<String>::new());
    let has_wallets = move || !wallets.get().is_empty();

    {
        let set_w = set_wallets;
        leptos::task::spawn_local(async move {
            // Try sync detection immediately
            let mut detected = get_detected_wallets_js();
            if detected.is_empty() {
                // Poll up to ~3s (10 attempts × 300ms) for wallet injection
                for _ in 0..10 {
                    gloo::timers::future::TimeoutFuture::new(300).await;
                    detected = get_detected_wallets_js();
                    if !detected.is_empty() {
                        break;
                    }
                }
            }
            log::info!("[admin-escrow] detected wallets: {:?}", detected);
            set_w.set(detected);
        });
    }

    let is_connected = move || !wallet_name.get().is_empty();

    // ── Wallet connect handler ──
    let handle_connect = move |wallet_name_str: String| {
        let set_wn = set_wallet_name.clone();
        let set_wp = set_wallet_pk.clone();
        let set_t = set_toast.clone();
        leptos::task::spawn_local(async move {
            match connect_wallet_js(&wallet_name_str).await {
                crate::wallet_error::WalletResult::Success(pk) => {
                    log::info!("[admin-escrow] wallet connected: {} ({})", wallet_name_str, pk);
                    set_wn.set(wallet_name_str);
                    set_wp.set(pk);
                }
                crate::wallet_error::WalletResult::Error(e) => {
                    components::show_toast(&set_t, &crate::wallet_error::user_friendly_message(&e), ToastType::Error);
                }
                crate::wallet_error::WalletResult::UnknownFailure => {
                    components::show_toast(&set_t, "Failed to connect wallet", ToastType::Error);
                }
            }
        });
    };

    // ── Action execution (Effect watches action_to_execute) ──
    Effect::new(move |_| {
        let action = match action_to_execute.get() {
            Some(a) => a,
            None => return,
        };

        let wn = wallet_name.get();
        let eid = active_event_id.get().unwrap_or_default();
        let set_sa = set_signing_action.clone();
        let set_ar = set_action_results.clone();
        let set_done = set_completed_actions.clone();
        let set_s1 = set_step1_done.clone();
        let set_s2 = set_step2_done.clone();
        let set_t = set_toast.clone();
        let set_trigger = set_action_to_execute.clone();

        set_sa.set(Some(action));

        leptos::task::spawn_local(async move {
            let tx_result: Result<String, api::ApiError> = match action {
                EscrowAction::Deactivate => {
                    api::deactivate_event(&api::DeactivateEventRequest { event_id: eid.clone() })
                        .await
                        .map(|r| r.transaction)
                }
                EscrowAction::ClaimForfeited => {
                    api::claim_forfeited(&api::ClaimForfeitedRequest { event_id: eid.clone() })
                        .await
                        .map(|r| r.transaction)
                }
                EscrowAction::CloseEvent => {
                    api::close_event(&api::CloseEventRequest { event_id: eid.clone() })
                        .await
                        .map(|r| r.transaction)
                }
            };

            match tx_result {
                Ok(transaction_b64) => {
                    // SEC-014: Verify wallet cluster matches expected network.
                    let expected_cluster = crate::utils::get_cluster();
                    if let Err(cluster_err) = crate::pages::escrow_init::check_wallet_cluster(&wn, &expected_cluster).await {
                        log::error!("[admin-escrow] cluster mismatch: {cluster_err}");
                        set_ar.update(|v| v.push((action, Err(cluster_err.clone()))));
                        return;
                    }
                    log::info!("[admin-escrow] {} TX built, signing...", action.label());

                    // Pre-sign simulation (Solana Foundation Security Checklist).
                    match crate::pages::escrow_init::simulate_transaction_js(&wn, &transaction_b64).await {
                        Ok(sim) if sim.ok => {}
                        Ok(sim) => {
                            let err_msg = sim.error.unwrap_or_else(|| "Simulation failed".to_string());
                            log::error!("[admin-escrow] {} simulation failed: {err_msg}", action.label());
                            set_ar.update(|v| v.push((action, Err(format!("Transaction would fail: {err_msg}")))));
                            return;
                        }
                        Err(e) => { log::warn!("[admin-escrow] simulate error (not blocking): {e}"); }
                    }

                    match sign_and_send_tx_js(&wn, &transaction_b64).await {
                        crate::wallet_error::WalletResult::Success(signature) => {
                            log::info!("[admin-escrow] {} TX confirmed: {}", action.label(), signature);
                            set_done.update(|v| v.push(action));
                            match action {
                                EscrowAction::Deactivate => set_s1.set(true),
                                EscrowAction::ClaimForfeited => set_s2.set(true),
                                EscrowAction::CloseEvent => {}
                            }
                            set_ar.update(|v| v.push((action, Ok(signature.clone()))));
                            components::show_toast(
                                &set_t,
                                &format!(
                                    "{} confirmed! {}",
                                    action.label(),
                                    &signature[..16.min(signature.len())]
                                ),
                                ToastType::Success,
                            );
                        }
                        crate::wallet_error::WalletResult::Error(e) => {
                            let msg = crate::wallet_error::user_friendly_message(&e);
                            log::error!("[admin-escrow] {} TX error: code={:?} msg={}", action.label(), e.code, e.raw_message);
                            set_ar.update(|v| v.push((action, Err(msg.clone()))));
                            components::show_toast(&set_t, &msg, ToastType::Error);
                        }
                        crate::wallet_error::WalletResult::UnknownFailure => {
                            log::error!("[admin-escrow] {} TX rejected", action.label());
                            let msg = format!("{} transaction failed", action.label());
                            set_ar.update(|v| v.push((action, Err(msg.clone()))));
                            components::show_toast(&set_t, &msg, ToastType::Error);
                        }
                    }
                }
                Err(e) => {
                    log::error!("[admin-escrow] {} failed: {e}", action.label());
                    let msg = format!("{}: {e}", action.label());
                    set_ar.update(|v| v.push((action, Err(msg.clone()))));
                    components::show_toast(&set_t, &msg, ToastType::Error);
                }
            }
            set_sa.set(None);
            set_trigger.set(None); // clear trigger so it can be re-triggered
        });
    });

    // Helpers
    let is_done = move |action: EscrowAction| -> bool { completed_actions.get().contains(&action) };
    let has_event = move || active_event_id.get().is_some();

    view! {
        <div class="admin-escrow">
            <div class="admin-section-header">
                <h3>"Escrow Management"</h3>
                <p class="admin-section-subtitle">
                    "Manage on-chain escrow lifecycle — deactivate, claim forfeited deposits, close."
                </p>
            </div>

            // No event selected
            <Show when=move || !has_event() fallback=|| view! { <div></div> }>
                <div class="admin-empty-state">
                    "Select an event with escrow enabled to manage."
                </div>
            </Show>

            // Event selected
            <Show when=move || has_event() fallback=|| view! { <div></div> }>

                // ── Wallet Connect ──
                <Show when=move || !is_connected() fallback=|| view! { <div></div> }>
                    <div class="panel-box">
                        <div class="panel-title">"Connect Organizer Wallet"</div>
                        <div class="hint-text admin-escrow-hint-mb">
                            "Connect the wallet that created this event's escrow to sign management transactions."
                        </div>
                        <Show when=move || has_wallets() fallback=|| view! {
                            <div class="badge-warn-text">
                                "No Solana wallet detected. Install Phantom or Solflare."
                            </div>
                        }>
                            <div class="flex-wrap-row">
                                {move || {
                                    wallets.get().iter().map(|w| {
                                        let wn = w.clone();
                                        let wn_click = w.clone();
                                        view! {
                                            <button
                                                class="btn-primary admin-escrow-connect-btn"
                                                on:click=move |_| handle_connect(wn_click.clone())
                                            >
                                                {format!("Connect {}", wn)}
                                            </button>
                                        }
                                    }).collect_view()
                                }}
                            </div>
                        </Show>
                    </div>
                </Show>

                // ── Wallet Connected ──
                <Show when=move || is_connected() fallback=|| view! { <div></div> }>
                    // Wallet info bar
                    <div class="wallet-bar">
                        <div class="wallet-bar-address">
                            {move || {
                                let wn = wallet_name.get();
                                let pk = wallet_pk.get();
                                format!("{} ({})", wn, &pk[..8.min(pk.len())])
                            }}
                        </div>
                        <button
                            class="btn btn-outline admin-escrow-disconnect-btn"
                            on:click=move |_| {
                                set_wallet_name.set(String::new());
                                set_wallet_pk.set(String::new());
                            }
                        >
                            "Disconnect"
                        </button>
                    </div>

                    // ── Per-Action Results ──
                    {move || {
                        let results = action_results.get();
                        results.iter().map(|(action, result)| {
                            match result {
                                Ok(signature) => {
                                    let sig = signature.clone();
                                    let solscan = crate::utils::solscan_tx_url(&sig, &crate::utils::get_cluster());
                                    view! {
                                        <div class="escrow-result escrow-result-success">
                                            <div class="escrow-result-text">
                                                {format!("{} confirmed", action.label())}
                                            </div>
                                            <div class="escrow-result-link">
                                                <a href=solscan target="_blank" rel="noopener">
                                                    "Solscan"
                                                </a>
                                            </div>
                                        </div>
                                    }.into_any()
                                }
                                Err(msg) => {
                                    let m = msg.clone();
                                    view! {
                                        <div class="escrow-result escrow-result-error">
                                            <div class="escrow-result-text">
                                                {format!("{}: {m}", action.label())}
                                            </div>
                                        </div>
                                    }.into_any()
                                }
                            }
                        }).collect_view()
                    }}

                    // ── Lifecycle Steps ──
                    <div class="step-list">
                        // Step 1: Deactivate
                        {move || {
                            let action = EscrowAction::Deactivate;
                            view! {
                                <EscrowStepCard
                                    action
                                    step_number=1
                                    done=is_done(action)
                                    signing=signing_action.get() == Some(action)
                                    ready=true
                                    confirming=false
                                    on_trigger=set_action_to_execute.clone()
                                    set_confirming=None
                                />
                            }
                        }}

                        // Step 2: Claim Forfeited (optional — skip if no deposits)
                        {move || {
                            let action = EscrowAction::ClaimForfeited;
                            let ready = step1_done.get();
                            view! {
                                <EscrowStepCard
                                    action
                                    step_number=2
                                    done=is_done(action)
                                    signing=signing_action.get() == Some(action)
                                    ready
                                    confirming=false
                                    on_trigger=set_action_to_execute.clone()
                                    set_confirming=None
                                />
                            }
                        }}

                        // Step 3: Close Event (ready after deactivate — claim is optional)
                        {move || {
                            let action = EscrowAction::CloseEvent;
                            let ready = step1_done.get();
                            let confirming = confirm_close.get();
                            view! {
                                <EscrowStepCard
                                    action
                                    step_number=3
                                    done=is_done(action)
                                    signing=signing_action.get() == Some(action)
                                    ready
                                    confirming
                                    on_trigger=set_action_to_execute.clone()
                                    set_confirming=Some(set_confirm_close.clone())
                                />
                            }
                        }}
                    </div>

                    // ── Info note ──
                    <div class="info-note admin-escrow-info-mt">
                        <strong>"Order matters:"</strong>
                        " Deactivate first. Claim Forfeited is optional (skip if no deposits). "
                        "Close reclaims rent and permanently closes the escrow account."
                    </div>
                </Show>
            </Show>
        </div>
    }
}
