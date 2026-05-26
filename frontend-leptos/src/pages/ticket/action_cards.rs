//! Action card components for deposit, refund, claim, and reclaim flows.

use crate::api::{self, DepositMethod, RolloverDepositRequest};
use crate::components::{self, ToastType};
use crate::icons::{Icon, IconName, wallet_icon_name};
use crate::utils;
use crate::wallet_error;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/js/solana_wallet.js")]
extern "C" {
    #[wasm_bindgen(js_name = "getDetectedWallets")]
    fn get_detected_wallets_js() -> Vec<String>;
}

/// Deposit required action card — prompts attendee to pay their deposit.
#[component]
pub fn DepositActionCard(
    /// Deposit amount in USDC (smallest unit, e.g. 15000000 = 15 USDC). 0 = not configured.
    amount_usdc: u64,
    /// Deposit amount in THB. 0 = not configured.
    amount_thb: u64,
    /// Whether the on-chain escrow is closed (USDC deposit unavailable).
    #[prop(default = false)]
    escrow_closed: bool,
    /// Deadline in hours after registration
    deadline_hours: Option<u32>,
    /// Link to the deposit page
    #[prop(into)]
    deposit_href: String,
) -> impl IntoView {
    let show_usdc = amount_usdc > 0 && !escrow_closed;
    let show_thb = amount_thb > 0;

    // Build primary label
    let primary_label = if show_thb {
        format!("{amount_thb} THB")
    } else if show_usdc {
        format!("${:.2} USDC", amount_usdc as f64 / 1_000_000.0)
    } else {
        "Deposit Required".to_string()
    };

    view! {
        <div class="ticket-action-card ticket-action-card--deposit">
            <div class="ticket-action-icon">
                <Icon icon=IconName::CreditCard class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">
                    {format!("Deposit Required: {primary_label}")}
                </div>
                // Show secondary payment method when both are available
                {if show_thb && show_usdc {
                    let usdc_str = format!("${:.2} USDC", amount_usdc as f64 / 1_000_000.0);
                    view! {
                        <div class="ticket-action-desc" style="margin-bottom:0.25rem;">
                            <span style="color:var(--text-secondary);font-size:0.8rem;">
                                "Also payable as "{usdc_str}" via Solana"
                            </span>
                        </div>
                    }.into_any()
                } else if amount_usdc > 0 && escrow_closed {
                    view! {
                        <div class="ticket-action-desc" style="margin-bottom:0.25rem;">
                            <span style="color:var(--text-secondary);font-size:0.8rem;">
                                "USDC deposit is no longer available (escrow closed)"
                            </span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
                <div class="ticket-action-desc">
                    {if let Some(hours) = deadline_hours {
                        format!("Complete your deposit within {hours} hours of registration to keep your in-person spot.")
                    } else {
                        "Complete your deposit to secure your in-person spot.".to_string()
                    }}
                </div>
                <a href=deposit_href class="btn btn-primary btn-sm ticket-action-btn">
                    <Icon icon=IconName::CreditCard class="icon-sm" />
                    " Pay Deposit Now"
                </a>
            </div>
        </div>
    }
}

/// Deposit verified notice — shown when deposit has been confirmed.
#[component]
pub fn DepositVerifiedCard() -> impl IntoView {
    view! {
        <div class="ticket-action-card ticket-action-card--verified">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Check class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">"Deposit: Verified ✓"</div>
            </div>
        </div>
    }
}

/// Deposit pending notice — shown while deposit is being verified.
#[component]
pub fn DepositPendingCard(
    /// Deposit method — controls the messaging
    method: DepositMethod,
) -> impl IntoView {
    let (label, desc) = match method {
        DepositMethod::Thb => (
            "Payment Slip: Pending Verification",
            "Your payment slip has been submitted. We'll verify it shortly — check back in a few minutes.",
        ),
        DepositMethod::Usdc => (
            "Deposit: Pending Confirmation",
            "Your deposit is being confirmed on-chain.",
        ),
        DepositMethod::CreditThb | DepositMethod::CreditUsdc => (
            "Credit Deposit: Pending",
            "Your credit deposit is being processed.",
        ),
    };

    view! {
        <div class="ticket-action-card ticket-action-card--pending">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Hourglass class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">{label}</div>
                <div class="ticket-action-desc">{desc}</div>
            </div>
        </div>
    }
}

/// Refund processed notice — shown when a deposit refund has been completed.
#[component]
pub fn RefundCard(
    /// URL to the refund proof/receipt (empty = hidden)
    #[prop(into)]
    refund_proof_url: String,
) -> impl IntoView {
    let url = refund_proof_url.clone();
    view! {
        <div class="ticket-action-card ticket-action-card--refund">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Recycle class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">"Refund: Processed ✓"</div>
                {if !url.is_empty() {
                    view! {
                        <a
                            href=url
                            target="_blank"
                            rel="noopener noreferrer"
                            class="ticket-action-link"
                        >
                            "View Refund Receipt →"
                        </a>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
            </div>
        </div>
    }
}

/// NFT claim CTA — shown when attendee is checked in but hasn't claimed their NFT.
#[component]
pub fn ClaimActionCard(
    /// Link to the claim page
    #[prop(into)]
    claim_href: String,
) -> impl IntoView {
    view! {
        <div class="ticket-action-card ticket-action-card--claim">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Gift class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">"You're checked in!"</div>
                <a href=claim_href class="btn btn-primary btn-sm ticket-action-btn">
                    <Icon icon=IconName::Gift class="icon-sm" />
                    " Claim Your NFT Badge →"
                </a>
            </div>
        </div>
    }
}

/// Reclaim spot prompt — shown when deposit deadline passed but spots are still available.
#[component]
pub fn ReclaimActionCard(
    /// Link to the deposit page for reclaiming
    #[prop(into)]
    reclaim_href: String,
) -> impl IntoView {
    view! {
        <div class="ticket-action-card ticket-action-card--reclaim">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Warning class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">"Deadline Passed — Reclaim Your Spot"</div>
                <div class="ticket-action-desc">
                    "Your deposit deadline has passed and you've been moved to the online track. \
                     However, in-person spots are still available!"
                </div>
                <a href=reclaim_href class="btn btn-success btn-sm ticket-action-btn">
                    <Icon icon=IconName::CreditCard class="icon-sm" />
                    " Deposit Now to Reclaim"
                </a>
            </div>
        </div>
    }
}

/// Moved to online track notice — shown when deposit deadline passed and no in-person spots.
#[component]
pub fn MovedOnlineCard() -> impl IntoView {
    view! {
        <div class="ticket-action-card ticket-action-card--moved-online">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Warning class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">"Moved to Online Track"</div>
                <div class="ticket-action-desc">
                    "Your deposit deadline has passed. In-person spots are now full, \
                     so you've been automatically moved to the online track. \
                     You can still claim your NFT after the event."
                </div>
            </div>
        </div>
    }
}

/// Rollover flow state machine.
#[derive(Clone)]
enum RolloverState {
    /// Initial CTA — prompts attendee to start.
    Ready,
    /// Choosing wallet to connect.
    ChooseWallet,
    /// Wallet connected, ready to sign.
    WalletConnected(String, String), // (wallet_name, public_key)
    /// Signing and sending TX.
    Signing(String, String), // (wallet_name, public_key)
    /// TX confirmed on-chain.
    Confirmed(String), // (tx_signature)
    /// Error state.
    Error(String),
}

/// Rollover deposit card — self-contained wallet signing flow.
///
/// Shown when attendee has a verified USDC deposit on a past event
/// and a new event from the same organizer is available.
#[component]
pub fn RolloverActionCard(
    /// Name of the target event to roll deposit into.
    #[prop(into)]
    target_event_name: String,
    /// Target event ID.
    #[prop(into)]
    target_event_id: String,
    /// Source event ID (current/past event).
    #[prop(into)]
    source_event_id: String,
    /// Attendee API ID.
    #[prop(into)]
    attendee_id: String,
) -> impl IntoView {
    let (state, set_state) = signal(RolloverState::Ready);
    let (toast, set_toast) = signal(None::<components::ToastMessage>);

    // Detect wallets on mount
    let (detected_wallets, _) = signal({
        let mut wallets = get_detected_wallets_js();
        if wallets.is_empty() {
            // Synchronous check only — if wallets inject late, user can retry
            wallets = get_detected_wallets_js();
        }
        wallets
    });

    // Connect wallet handler
    let handle_connect = move |wallet_name: String| {
        let wn = wallet_name.clone();
        leptos::task::spawn_local(async move {
            match crate::pages::escrow_init::connect_wallet_js(&wn).await {
                wallet_error::WalletResult::Success(pk) => {
                    log::info!("[rollover] wallet connected: {} ({})", wn, pk);
                    set_state.set(RolloverState::WalletConnected(wn, pk));
                }
                wallet_error::WalletResult::Error(e) => {
                    log::error!("[rollover] wallet connect error: {:?}", e.code);
                    components::show_toast(
                        &set_toast,
                        &wallet_error::user_friendly_message(&e),
                        ToastType::Error,
                    );
                }
                wallet_error::WalletResult::UnknownFailure => {
                    components::show_toast(
                        &set_toast,
                        "Failed to connect wallet. Please try again.",
                        ToastType::Error,
                    );
                }
            }
        });
    };

    // Sign and send rollover TX handler
    let handle_sign_and_send = move |wallet_name: String, public_key: String| {
        let wn = wallet_name.clone();
        let pk = public_key.clone();
        let source_eid = source_event_id.clone();
        let target_eid = target_event_id.clone();
        let aid = attendee_id.clone();

        set_state.set(RolloverState::Signing(wallet_name, public_key));

        leptos::task::spawn_local(async move {
            // Step 1: Build rollover TX via API
            let body = RolloverDepositRequest {
                source_event_id: source_eid,
                target_event_id: target_eid,
                attendee_id: aid,
                wallet_address: pk.clone(),
            };
            let resp = match api::rollover_deposit(&body).await {
                Ok(r) => r,
                Err(e) => {
                    log::error!("[rollover] TX build failed: {e}");
                    set_state.set(RolloverState::Error(format!("Failed to build transaction: {e}")));
                    return;
                }
            };

            let tx_b64 = resp.transaction;
            if tx_b64.is_empty() {
                set_state.set(RolloverState::Error("Transaction was empty.".to_string()));
                return;
            }

            // Step 2: Verify wallet cluster
            let expected_cluster = crate::utils::get_cluster();
            if let Err(cluster_err) =
                crate::pages::escrow_init::check_wallet_cluster(&wn, &expected_cluster).await
            {
                log::error!("[rollover] cluster mismatch: {cluster_err}");
                set_state.set(RolloverState::Error(cluster_err));
                return;
            }

            // Step 3: Pre-sign simulation
            match crate::pages::escrow_init::simulate_transaction_js(&wn, &tx_b64).await {
                Ok(sim) if sim.ok => {}
                Ok(sim) => {
                    let err_msg = sim.error.unwrap_or_else(|| "Simulation failed".to_string());
                    log::error!("[rollover] simulation failed: {err_msg}");
                    set_state.set(RolloverState::Error(format!("Transaction would fail: {err_msg}")));
                    return;
                }
                Err(e) => {
                    log::warn!("[rollover] simulate error (not blocking): {e}");
                }
            }

            // Step 4: Sign and send
            match crate::pages::escrow_init::sign_and_send_tx_js(&wn, &tx_b64).await {
                wallet_error::WalletResult::Success(signature) => {
                    log::info!("[rollover] TX confirmed: {}", signature);
                    set_state.set(RolloverState::Confirmed(signature));
                }
                wallet_error::WalletResult::Error(e) => {
                    log::error!("[rollover] sign+send error: {:?}", e.code);
                    set_state.set(RolloverState::Error(
                        wallet_error::user_friendly_message(&e),
                    ));
                }
                wallet_error::WalletResult::UnknownFailure => {
                    set_state.set(RolloverState::Error(
                        "Transaction failed. Please try again.".to_string(),
                    ));
                }
            }
        });
    };

    view! {
        <div class="ticket-action-card ticket-action-card--rollover">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Refresh class="icon-sm" />
            </div>
            <div>
                {move || match &state.get() {
                    RolloverState::Ready => view! {
                        <div class="ticket-action-title">"Roll Deposit to Next Event"</div>
                        <div class="ticket-action-desc">
                            {format!(
                                "Your deposit is ready to roll over to {}. \
                                 No extra payment needed — your USDC transfers atomically.",
                                target_event_name
                            )}
                        </div>
                        <button
                            class="btn btn-primary btn-sm ticket-action-btn"
                            on:click=move |_| set_state.set(RolloverState::ChooseWallet)
                        >
                            <Icon icon=IconName::Refresh class="icon-sm" />
                            " Roll to Next Event"
                        </button>
                    }.into_any(),

                    RolloverState::ChooseWallet => {
                        let wallets = detected_wallets.get();
                        view! {
                            <div class="ticket-action-title">"Connect Wallet to Rollover"</div>
                            <div class="ticket-action-desc">
                                "Connect the wallet you used for the original deposit."
                            </div>
                            {if wallets.is_empty() {
                                view! {
                                    <p class="ticket-action-desc" style="color:var(--text-secondary);">
                                        "No wallet detected. Install Phantom/Backpack/Solflare and refresh."
                                    </p>
                                }.into_any()
                            } else {
                                let btns: Vec<_> = wallets.into_iter().map(|w| {
                                    let w_click = w.clone();
                                    let w_label = w.clone();
                                    let wi = wallet_icon_name(&w);
                                    view! {
                                        <button
                                            class="btn btn-primary btn-sm"
                                            style="margin-right:0.25rem;margin-bottom:0.25rem;"
                                            on:click=move |_| handle_connect(w_click.clone())
                                        >
                                            <Icon icon=wi class="icon-sm" />
                                            " "{w_label}
                                        </button>
                                    }
                                }).collect();
                                view! { <div>{btns}</div> }.into_any()
                            }}
                            <button
                                class="btn btn-outline btn-xs"
                                style="margin-top:0.5rem;"
                                on:click=move |_| set_state.set(RolloverState::Ready)
                            >
                                "Cancel"
                            </button>
                        }.into_any()
                    },

                    RolloverState::WalletConnected(wn, _pk) => {
                        let wn_send = wn.clone();
                        let pk_send = _pk.clone();
                        let wn_display = wn.clone();
                        view! {
                            <div class="ticket-action-title">
                                {format!("Connected via {}", wn_display)}
                            </div>
                            <div class="ticket-action-desc">
                                "Click below to sign and send the rollover transaction."
                            </div>
                            <button
                                class="btn btn-success btn-sm ticket-action-btn"
                                on:click=move |_| handle_sign_and_send(wn_send.clone(), pk_send.clone())
                            >
                                <Icon icon=IconName::Refresh class="icon-sm" />
                                " Sign & Send Rollover"
                            </button>
                            <button
                                class="btn btn-outline btn-xs"
                                style="margin-top:0.25rem;"
                                on:click=move |_| set_state.set(RolloverState::Ready)
                            >
                                "Cancel"
                            </button>
                        }.into_any()
                    },

                    RolloverState::Signing(_, _) => view! {
                        <div class="ticket-action-title">"Processing Rollover..."</div>
                        <div class="ticket-action-desc" style="display:flex;align-items:center;gap:0.5rem;">
                            <span class="spinner spinner-sm"></span>
                            "Please approve the transaction in your wallet..."
                        </div>
                    }.into_any(),

                    RolloverState::Confirmed(sig) => {
                        let solscan = utils::solscan_tx_url(sig, &utils::get_cluster());
                        let sig_short = if sig.len() > 20 {
                            format!("{}...{}", &sig[..8], &sig[sig.len()-8..])
                        } else {
                            sig.clone()
                        };
                        view! {
                            <div class="ticket-action-title" style="color:var(--success);">
                                "Deposit Rolled Over ✓"
                            </div>
                            <div class="ticket-action-desc">
                                {format!(
                                    "Your deposit has been moved to {}. TX: {}",
                                    target_event_name, sig_short
                                )}
                            </div>
                            <a
                                href=solscan
                                target="_blank"
                                rel="noopener noreferrer"
                                class="ticket-action-link"
                            >
                                "View on Solscan →"
                            </a>
                        }.into_any()
                    },

                    RolloverState::Error(msg) => view! {
                        <div class="ticket-action-title" style="color:var(--danger);">
                            "Rollover Failed"
                        </div>
                        <div class="ticket-action-desc">{msg.clone()}</div>
                        <button
                            class="btn btn-outline btn-xs"
                            style="margin-top:0.25rem;"
                            on:click=move |_| set_state.set(RolloverState::Ready)
                        >
                            "Try Again"
                        </button>
                    }.into_any(),
                }}
            </div>
        </div>
    }
}
