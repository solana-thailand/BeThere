//! Action card components for deposit, refund, claim, and reclaim flows.

use crate::api::{self, DepositMethod, HoldDepositRequest, RolloverDepositRequest};
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
                        <div class="ticket-action-desc ticket-action-alt-desc">
                            <span class="ticket-action-alt-text">
                                "Also payable as "{usdc_str}" via Solana"
                            </span>
                        </div>
                    }.into_any()
                } else if amount_usdc > 0 && escrow_closed {
                    view! {
                        <div class="ticket-action-desc ticket-action-alt-desc">
                            <span class="ticket-action-alt-text">
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
                <div class="ticket-action-title">"RSVP Deposit Returned ✓"</div>
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
    #[allow(dead_code)]
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
    let (_toast, set_toast) = signal(None::<components::ToastMessage>);

    // Store non-Copy props so they can be accessed from multiple closures
    let source_eid = StoredValue::new(source_event_id);
    let target_eid = StoredValue::new(target_event_id);
    let aid_stored = StoredValue::new(attendee_id);

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

    view! {
        <div class="ticket-action-card ticket-action-card--rollover">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Refresh class="icon-sm" />
            </div>
            <div>
                {move || match state.get() {
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
                                    <p class="ticket-action-desc ticket-action-alt-text">
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
                                            class="btn btn-primary btn-sm ticket-action-wallet-btn"
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
                                class="btn btn-outline btn-xs ticket-action-cancel"
                                on:click=move |_| set_state.set(RolloverState::Ready)
                            >
                                "Cancel"
                            </button>
                        }.into_any()
                    },

                    RolloverState::WalletConnected(wn, _pk) => {
                        let wn_display = wn.clone();
                        let sv_source = source_eid;
                        let sv_target = target_eid;
                        let sv_aid = aid_stored;
                        let ss = set_state;
                        view! {
                            <div class="ticket-action-title">
                                {format!("Connected via {}", wn_display)}
                            </div>
                            <div class="ticket-action-desc">
                                "Click below to sign and send the rollover transaction."
                            </div>
                            <button
                                class="btn btn-success btn-sm ticket-action-btn"
                                on:click=move |_| {
                                    let wn_c = wn.clone();
                                    let pk_c = _pk.clone();
                                    let s_eid = sv_source.get_value();
                                    let t_eid = sv_target.get_value();
                                    let a_id = sv_aid.get_value();
                                    ss.set(RolloverState::Signing(wn_c.clone(), pk_c.clone()));
                                    leptos::task::spawn_local(async move {
                                        let body = RolloverDepositRequest {
                                            source_event_id: s_eid,
                                            target_event_id: t_eid,
                                            attendee_id: a_id,
                                            wallet_address: pk_c.clone(),
                                        };
                                        let resp = match api::rollover_deposit(&body).await {
                                            Ok(r) => r,
                                            Err(e) => {
                                                log::error!("[rollover] TX build failed: {e}");
                                                ss.set(RolloverState::Error(format!("Failed to build transaction: {e}")));
                                                return;
                                            }
                                        };
                                        let tx_b64 = resp.transaction;
                                        if tx_b64.is_empty() {
                                            ss.set(RolloverState::Error("Transaction was empty.".to_string()));
                                            return;
                                        }
                                        let expected_cluster = crate::utils::get_cluster();
                                        if let Err(cluster_err) =
                                            crate::pages::escrow_init::check_wallet_cluster(&wn_c, &expected_cluster).await
                                        {
                                            log::error!("[rollover] cluster mismatch: {cluster_err}");
                                            ss.set(RolloverState::Error(cluster_err));
                                            return;
                                        }
                                        match crate::pages::escrow_init::simulate_transaction_js(&wn_c, &tx_b64).await {
                                            Ok(sim) if sim.ok => {}
                                            Ok(sim) => {
                                                let err_msg = sim.error.unwrap_or_else(|| "Simulation failed".to_string());
                                                log::error!("[rollover] simulation failed: {err_msg}");
                                                ss.set(RolloverState::Error(format!("Transaction would fail: {err_msg}")));
                                                return;
                                            }
                                            Err(e) => {
                                                log::warn!("[rollover] simulate error (not blocking): {e}");
                                            }
                                        }
                                        match crate::pages::escrow_init::sign_and_send_tx_js(&wn_c, &tx_b64).await {
                                            wallet_error::WalletResult::Success(signature) => {
                                                log::info!("[rollover] TX confirmed: {}", signature);
                                                ss.set(RolloverState::Confirmed(signature));
                                            }
                                            wallet_error::WalletResult::Error(e) => {
                                                log::error!("[rollover] sign+send error: {:?}", e.code);
                                                ss.set(RolloverState::Error(
                                                    wallet_error::user_friendly_message(&e),
                                                ));
                                            }
                                            wallet_error::WalletResult::UnknownFailure => {
                                                ss.set(RolloverState::Error(
                                                    "Transaction failed. Please try again.".to_string(),
                                                ));
                                            }
                                        }
                                    });
                                }
                            >
                                <Icon icon=IconName::Refresh class="icon-sm" />
                                " Sign & Send Rollover"
                            </button>
                            <button
                                class="btn btn-outline btn-xs ticket-action-cancel-xs"
                                on:click=move |_| set_state.set(RolloverState::Ready)
                            >
                                "Cancel"
                            </button>
                        }.into_any()
                    },

                    RolloverState::Signing(_, _) => view! {
                        <div class="ticket-action-title">"Processing Rollover..."</div>
                        <div class="ticket-action-desc ticket-action-signing-row">
                            <span class="spinner spinner-sm"></span>
                            "Please approve the transaction in your wallet..."
                        </div>
                    }.into_any(),

                    RolloverState::Confirmed(sig) => {
                        let solscan = utils::solscan_tx_url(&sig, &utils::get_cluster());
                        let sig_short = if sig.len() > 20 {
                            format!("{}...{}", &sig[..8], &sig[sig.len()-8..])
                        } else {
                            sig.clone()
                        };
                        view! {
                            <div class="ticket-action-title ticket-action-title-success">
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
                        <div class="ticket-action-title ticket-action-title-danger">
                            "Rollover Failed"
                        </div>
                        <div class="ticket-action-desc">{msg.clone()}</div>
                        <button
                            class="btn btn-outline btn-xs ticket-action-cancel-xs"
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

// ===== Hold Deposit (THB rolling credit) =====

/// State machine for the THB hold-deposit-as-credit flow.
/// Simpler than rollover: no wallet connection, just an authenticated POST.
#[derive(Clone)]
enum HoldDepositState {
    /// Initial CTA.
    Ready,
    /// Confirmation step explaining the commitment.
    Confirm,
    /// POST in flight.
    Holding,
    /// Success — shows the new credit balance.
    Confirmed { credit_thb: u64, credit_usdc: u64 },
    /// Loaded from server: deposit already converted to credit on a prior call.
    /// Distinct from `Confirmed` (no in-session balance to display) and used as
    /// the initial state when `already_held` prop is true (Issue #061 idempotency).
    AlreadyHeld,
    /// Error.
    Error(String),
}

/// Hold Deposit action card — attendee keeps their THB deposit as rolling credit
/// instead of claiming a refund. The held credit auto-covers their next event
/// registration. THB-only counterpart to the USDC `RolloverActionCard`.
///
/// Backend: `POST /api/deposit/hold` (validates ownership + requires verified deposit).
#[component]
pub fn HoldDepositCard(
    /// Event the deposit belongs to.
    #[prop(into)]
    event_id: String,
    /// Attendee API ID.
    #[prop(into)]
    attendee_id: String,
    /// THB amount being held (for the confirm copy).
    deposit_amount_thb: u64,
    /// Whether the server reports this deposit was already converted to credit
    /// on a prior call. Mounts the card in `AlreadyHeld` so the attendee sees a
    /// held-confirmation (not the CTA) on reload — the backend idempotency guard
    /// is the safety net; this is the UX (Issue #061 idempotency).
    #[prop(default = false)]
    already_held: bool,
) -> impl IntoView {
    let initial = if already_held {
        HoldDepositState::AlreadyHeld
    } else {
        HoldDepositState::Ready
    };
    let (state, set_state) = signal(initial);

    // Store non-Copy props so they can be accessed from the async closure.
    let eid = StoredValue::new(event_id);
    let aid = StoredValue::new(attendee_id);
    let amount = deposit_amount_thb;

    view! {
        <div class="ticket-action-card ticket-action-card--hold">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Save class="icon-sm" />
            </div>
            <div>
                {move || match state.get() {
                    HoldDepositState::Ready => view! {
                        <div class="ticket-action-title">"Hold Deposit for Next Event"</div>
                        <div class="ticket-action-desc">
                            "Keep your deposit as credit and we'll auto-apply it to your next event. \
                             No need to pay again — just RSVP."
                        </div>
                        <button
                            class="btn btn-outline btn-sm ticket-action-btn"
                            on:click=move |_| set_state.set(HoldDepositState::Confirm)
                        >
                            <Icon icon=IconName::Save class="icon-sm" />
                            " Hold Deposit"
                        </button>
                    }.into_any(),

                    HoldDepositState::Confirm => view! {
                        <div class="ticket-action-title">"Confirm: Hold "{amount}" THB"</div>
                        <div class="ticket-action-desc">
                            {format!(
                                "We'll keep your {} THB deposit on file. It will be applied \
                                 automatically when you register for your next event. \
                                 You can request its return at any time.",
                                amount
                            )}
                        </div>
                        <button
                            class="btn btn-success btn-sm ticket-action-btn"
                            on:click=move |_| {
                                let sv_eid = eid;
                                let sv_aid = aid;
                                let ss = set_state;
                                ss.set(HoldDepositState::Holding);
                                leptos::task::spawn_local(async move {
                                    let body = HoldDepositRequest {
                                        event_id: sv_eid.get_value(),
                                        attendee_id: sv_aid.get_value(),
                                    };
                                    match api::hold_deposit(&body).await {
                                        Ok(resp) => {
                                            log::info!(
                                                "[hold] deposit held: thb={} usdc={}",
                                                resp.credit_thb, resp.credit_usdc
                                            );
                                            ss.set(HoldDepositState::Confirmed {
                                                credit_thb: resp.credit_thb,
                                                credit_usdc: resp.credit_usdc,
                                            });
                                        }
                                        Err(e) => {
                                            log::error!("[hold] failed: {}", e.message);
                                            ss.set(HoldDepositState::Error(e.message));
                                        }
                                    }
                                });
                            }
                        >
                            <Icon icon=IconName::Check class="icon-sm" />
                            " Confirm & Hold"
                        </button>
                        <button
                            class="btn btn-outline btn-xs ticket-action-cancel"
                            on:click=move |_| set_state.set(HoldDepositState::Ready)
                        >
                            "Cancel"
                        </button>
                    }.into_any(),

                    HoldDepositState::Holding => view! {
                        <div class="ticket-action-title">"Holding Deposit..."</div>
                        <div class="ticket-action-desc ticket-action-signing-row">
                            <span class="spinner spinner-sm"></span>
                            "Processing your request..."
                        </div>
                    }.into_any(),

                    HoldDepositState::Confirmed { credit_thb, credit_usdc } => {
                        let balance_str = if credit_thb > 0 && credit_usdc > 0 {
                            format!("{} THB + {} USDC", credit_thb, credit_usdc)
                        } else if credit_thb > 0 {
                            format!("{} THB", credit_thb)
                        } else {
                            format!("{} USDC", credit_usdc)
                        };
                        view! {
                            <div class="ticket-action-title ticket-action-title-success">
                                "Deposit Held as Credit ✓"
                            </div>
                            <div class="ticket-action-desc">
                                {format!(
                                    "Your {} THB is now rolling credit. Total credit: {}. \
                                     We'll auto-apply it to your next registration.",
                                    amount, balance_str
                                )}
                            </div>
                        }.into_any()
                    },

                    HoldDepositState::AlreadyHeld => view! {
                        <div class="ticket-action-title ticket-action-title-success">
                            "Deposit Held as Credit ✓"
                        </div>
                        <div class="ticket-action-desc">
                            {format!(
                                "Your {} THB deposit is held as rolling credit and will be \
                                 auto-applied to your next event registration.",
                                amount
                            )}
                        </div>
                    }.into_any(),

                    HoldDepositState::Error(msg) => view! {
                        <div class="ticket-action-title ticket-action-title-danger">
                            "Hold Failed"
                        </div>
                        <div class="ticket-action-desc">{msg.clone()}</div>
                        <button
                            class="btn btn-outline btn-xs ticket-action-cancel-xs"
                            on:click=move |_| set_state.set(HoldDepositState::Ready)
                        >
                            "Try Again"
                        </button>
                    }.into_any(),
                }}
            </div>
        </div>
    }
}
