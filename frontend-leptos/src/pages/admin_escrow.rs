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
        log::warn!("[wasm] connect_wallet_js: empty wallet name, returning None");
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
            log::error!("[wasm] connect_wallet_js error: {:?}", e);
            None
        }
    }
}

async fn sign_and_send_tx_js(wallet_name: &str, transaction_b64: &str) -> Option<String> {
    if wallet_name.is_empty() {
        log::warn!("[wasm] sign_and_send_tx_js: empty wallet name, returning None");
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
            log::error!("[wasm] sign_and_send_tx_js error: {:?}", e);
            None
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

    fn icon(&self) -> &'static str {
        match self {
            Self::Deactivate => "⏸",
            Self::ClaimForfeited => "💰",
            Self::CloseEvent => "🗑",
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
}

// ===== Component =====

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

    // Last result for showing success/error banner
    let (last_result, set_last_result) = signal(None::<(EscrowAction, Result<String, String>)>);

    // Reset state when event changes
    Effect::new(move |_| {
        let _ = active_event_id.get();
        set_wallet_name.set(String::new());
        set_wallet_pk.set(String::new());
        set_completed_actions.set(Vec::new());
        set_signing_action.set(None);
        set_last_result.set(None);
    });

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
                Some(pk) => {
                    log::info!("[admin-escrow] wallet connected: {} ({})", wallet_name_str, pk);
                    set_wn.set(wallet_name_str);
                    set_wp.set(pk);
                }
                None => {
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
        let set_lr = set_last_result.clone();
        let set_done = set_completed_actions.clone();
        let set_t = set_toast.clone();
        let set_trigger = set_action_to_execute.clone();

        set_sa.set(Some(action));
        set_lr.set(None);

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
                    log::info!("[admin-escrow] {} TX built, signing...", action.label());
                    match sign_and_send_tx_js(&wn, &transaction_b64).await {
                        Some(signature) => {
                            log::info!("[admin-escrow] {} TX confirmed: {}", action.label(), signature);
                            set_done.update(|v| v.push(action));
                            set_lr.set(Some((action, Ok(signature.clone()))));
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
                        None => {
                            log::error!("[admin-escrow] {} TX rejected", action.label());
                            let msg = format!("{} transaction rejected or failed", action.label());
                            set_lr.set(Some((action, Err(msg.clone()))));
                            components::show_toast(&set_t, &msg, ToastType::Error);
                        }
                    }
                }
                Err(e) => {
                    log::error!("[admin-escrow] {} failed: {e}", action.label());
                    let msg = format!("{}: {e}", action.label());
                    set_lr.set(Some((action, Err(msg.clone()))));
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
                <h3>"⛓ Escrow Management"</h3>
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
                    <div style="margin-bottom:1rem;padding:0.75rem;border:1px solid var(--border);border-radius:8px;background:var(--bg-secondary)">
                        <div style="font-size:0.85rem;font-weight:600;color:var(--text-primary);margin-bottom:0.5rem">
                            "🔗 Connect Organizer Wallet"
                        </div>
                        <div style="font-size:0.75rem;color:var(--text-secondary);margin-bottom:0.75rem">
                            "Connect the wallet that created this event's escrow to sign management transactions."
                        </div>
                        <Show when=move || has_wallets() fallback=|| view! {
                            <div style="font-size:0.75rem;color:var(--warning,orange)">
                                "⚠ No Solana wallet detected. Install Phantom or Solflare."
                            </div>
                        }>
                            <div style="display:flex;gap:0.5rem;flex-wrap:wrap">
                                {move || {
                                    wallets.get().iter().map(|w| {
                                        let wn = w.clone();
                                        let wn_click = w.clone();
                                        view! {
                                            <button
                                                class="btn-primary"
                                                style="font-size:0.8rem;padding:0.4rem 0.8rem"
                                                on:click=move |_| handle_connect(wn_click.clone())
                                            >
                                                {format!("🔗 Connect {}", wn)}
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
                    <div style="display:flex;align-items:center;justify-content:space-between;padding:0.5rem 0.75rem;border:1px solid var(--border);border-radius:6px;background:var(--bg-secondary);margin-bottom:0.75rem">
                        <div style="font-size:0.8rem;color:var(--text-primary)">
                            {move || {
                                let wn = wallet_name.get();
                                let pk = wallet_pk.get();
                                format!("🔗 {} ({})", wn, &pk[..8.min(pk.len())])
                            }}
                        </div>
                        <button
                            class="btn btn-outline"
                            style="font-size:0.7rem;padding:0.2rem 0.5rem"
                            on:click=move |_| {
                                set_wallet_name.set(String::new());
                                set_wallet_pk.set(String::new());
                            }
                        >
                            "Disconnect"
                        </button>
                    </div>

                    // ── Last Result Banner ──
                    {move || {
                        match &last_result.get() {
                            Some((action, Ok(signature))) => {
                                let sig = signature.clone();
                                let solscan = format!("https://solscan.io/tx/{sig}?cluster=devnet");
                                view! {
                                    <div style="margin-bottom:0.75rem;padding:0.5rem 0.75rem;border:1px solid var(--success,green);border-radius:6px;background:rgba(0,128,0,0.05)">
                                        <div style="font-size:0.8rem;color:var(--success,green);font-weight:600">
                                            {format!("✅ {} confirmed", action.label())}
                                        </div>
                                        <div style="font-size:0.7rem;margin-top:0.25rem">
                                            <a href=solscan target="_blank" rel="noopener" style="color:var(--accent)">
                                                "View on Solscan ↗"
                                            </a>
                                        </div>
                                    </div>
                                }.into_any()
                            }
                            Some((action, Err(msg))) => {
                                let m = msg.clone();
                                view! {
                                    <div style="margin-bottom:0.75rem;padding:0.5rem 0.75rem;border:1px solid var(--error,red);border-radius:6px;background:rgba(255,0,0,0.05)">
                                        <div style="font-size:0.8rem;color:var(--error,red)">
                                            {format!("❌ {}: {m}", action.label())}
                                        </div>
                                    </div>
                                }.into_any()
                            }
                            None => view! { <div></div> }.into_any(),
                        }
                    }}

                    // ── Lifecycle Steps ──
                    <div style="display:flex;flex-direction:column;gap:0.5rem">

                        // Step 1: Deactivate
                        {move || {
                            let action = EscrowAction::Deactivate;
                            let done = is_done(action);
                            let signing = signing_action.get() == Some(action);
                            let border = if done { "var(--success,green)" } else { "var(--border)" };
                            let check = if done { " ✅" } else { "" };
                            let trigger = set_action_to_execute.clone();
                            view! {
                                <div style=format!("display:flex;align-items:center;justify-content:space-between;gap:1rem;padding:0.5rem 0.75rem;border:1px solid {border};border-radius:6px;background:var(--bg-primary)")>
                                    <div>
                                        <div style="font-size:0.85rem;font-weight:600;color:var(--text-primary)">
                                            {format!("{} Step 1: {}{}", action.icon(), action.label(), check)}
                                        </div>
                                        <div style="font-size:0.7rem;color:var(--text-secondary)">
                                            {action.description()}
                                        </div>
                                    </div>
                                    {if done {
                                        view! { <span style="font-size:0.8rem;color:var(--success,green)">"Done"</span> }.into_any()
                                    } else if signing {
                                        view! {
                                            <div style="display:flex;align-items:center;gap:0.3rem">
                                                <span class="spinner spinner-sm"></span>
                                                <span style="font-size:0.75rem;color:var(--text-secondary)">"Signing..."</span>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <button
                                                class=action.button_class()
                                                style="white-space:nowrap;font-size:0.8rem;padding:0.4rem 0.8rem"
                                                on:click=move |_| trigger.set(Some(action))
                                            >
                                                "⚡ Sign TX"
                                            </button>
                                        }.into_any()
                                    }}
                                </div>
                            }
                        }}

                        // Step 2: Claim Forfeited
                        {move || {
                            let action = EscrowAction::ClaimForfeited;
                            let done = is_done(action);
                            let signing = signing_action.get() == Some(action);
                            let border = if done { "var(--success,green)" } else { "var(--border)" };
                            let check = if done { " ✅" } else { "" };
                            let trigger = set_action_to_execute.clone();
                            view! {
                                <div style=format!("display:flex;align-items:center;justify-content:space-between;gap:1rem;padding:0.5rem 0.75rem;border:1px solid {border};border-radius:6px;background:var(--bg-primary)")>
                                    <div>
                                        <div style="font-size:0.85rem;font-weight:600;color:var(--text-primary)">
                                            {format!("{} Step 2: {}{}", action.icon(), action.label(), check)}
                                        </div>
                                        <div style="font-size:0.7rem;color:var(--text-secondary)">
                                            {action.description()}
                                        </div>
                                    </div>
                                    {if done {
                                        view! { <span style="font-size:0.8rem;color:var(--success,green)">"Done"</span> }.into_any()
                                    } else if signing {
                                        view! {
                                            <div style="display:flex;align-items:center;gap:0.3rem">
                                                <span class="spinner spinner-sm"></span>
                                                <span style="font-size:0.75rem;color:var(--text-secondary)">"Signing..."</span>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <button
                                                class=action.button_class()
                                                style="white-space:nowrap;font-size:0.8rem;padding:0.4rem 0.8rem"
                                                on:click=move |_| trigger.set(Some(action))
                                            >
                                                "⚡ Sign TX"
                                            </button>
                                        }.into_any()
                                    }}
                                </div>
                            }
                        }}

                        // Step 3: Close Event
                        {move || {
                            let action = EscrowAction::CloseEvent;
                            let done = is_done(action);
                            let signing = signing_action.get() == Some(action);
                            let border = if done { "var(--success,green)" } else { "var(--border)" };
                            let check = if done { " ✅" } else { "" };
                            let trigger = set_action_to_execute.clone();
                            view! {
                                <div style=format!("display:flex;align-items:center;justify-content:space-between;gap:1rem;padding:0.5rem 0.75rem;border:1px solid {border};border-radius:6px;background:var(--bg-primary)")>
                                    <div>
                                        <div style="font-size:0.85rem;font-weight:600;color:var(--text-primary)">
                                            {format!("{} Step 3: {}{}", action.icon(), action.label(), check)}
                                        </div>
                                        <div style="font-size:0.7rem;color:var(--text-secondary)">
                                            {action.description()}
                                        </div>
                                    </div>
                                    {if done {
                                        view! { <span style="font-size:0.8rem;color:var(--success,green)">"Done"</span> }.into_any()
                                    } else if signing {
                                        view! {
                                            <div style="display:flex;align-items:center;gap:0.3rem">
                                                <span class="spinner spinner-sm"></span>
                                                <span style="font-size:0.75rem;color:var(--text-secondary)">"Signing..."</span>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <button
                                                class=action.button_class()
                                                style="white-space:nowrap;font-size:0.8rem;padding:0.4rem 0.8rem"
                                                on:click=move |_| trigger.set(Some(action))
                                            >
                                                "⚡ Sign TX"
                                            </button>
                                        }.into_any()
                                    }}
                                </div>
                            }
                        }}

                    </div>

                    // ── Info note ──
                    <div style="margin-top:0.75rem;padding:0.5rem 0.75rem;border:1px dashed var(--border);border-radius:6px;background:var(--bg-secondary);font-size:0.7rem;color:var(--text-secondary)">
                        <strong>"Order matters:"</strong>
                        " Deactivate before claiming forfeited. Claim before closing. "
                        "Close reclaims rent and permanently closes the escrow account."
                    </div>
                </Show>
            </Show>
        </div>
    }
}
