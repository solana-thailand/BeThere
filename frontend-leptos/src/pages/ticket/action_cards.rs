//! Action card components for deposit, refund, claim, and reclaim flows.

use crate::api::{self, DepositMethod, HoldDepositRequest, RolloverDepositRequest};
use crate::components::{self, ToastType};
use crate::i18n::{t, t_string, td_string, use_i18n};
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
    let i18n = use_i18n();

    // Build primary label
    let amount = move || {
        if show_thb {
            format!("{amount_thb} THB")
        } else if show_usdc {
            format!("${:.2} USDC", amount_usdc as f64 / 1_000_000.0)
        } else {
            t_string!(i18n, ticket.action.deposit_required).to_string()
        }
    };

    view! {
        <div class="ticket-action-card ticket-action-card--deposit">
            <div class="ticket-action-icon">
                <Icon icon=IconName::CreditCard class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">
                    {t!(i18n, ticket.action.deposit_required_amount, amount)}
                </div>
                // Show secondary payment method when both are available
                {if show_thb && show_usdc {
                    let usdc = format!("${:.2} USDC", amount_usdc as f64 / 1_000_000.0);
                    view! {
                        <div class="ticket-action-desc ticket-action-alt-desc">
                            <span class="ticket-action-alt-text">
                                {t!(i18n, ticket.action.also_payable, usdc)}
                            </span>
                        </div>
                    }.into_any()
                } else if amount_usdc > 0 && escrow_closed {
                    view! {
                        <div class="ticket-action-desc ticket-action-alt-desc">
                            <span class="ticket-action-alt-text">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.usdc_closed))}
                            </span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
                <div class="ticket-action-desc">
                    {match deadline_hours {
                        Some(hours) => t!(i18n, ticket.action.deadline_hours, hours).into_any(),
                        None => crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.deadline_none)).into_any(),
                    }}
                </div>
                <a href=deposit_href class="btn btn-primary btn-sm ticket-action-btn">
                    <Icon icon=IconName::CreditCard class="icon-sm" />
                    " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.pay_now))}
                </a>
            </div>
        </div>
    }
}

/// Deposit verified notice — shown when deposit has been confirmed.
#[component]
pub fn DepositVerifiedCard() -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="ticket-action-card ticket-action-card--verified">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Check class="icon-sm" />
            </div>
            <div>
                {move || view! {
                    <crate::components::StatusBadge
                        tone=crate::components::StatusTone::Confirmed
                        label=t_string!(i18n, ticket.action.deposit_verified)
                    />
                }}
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
    let i18n = use_i18n();
    let label = move || match method {
        DepositMethod::Thb => t_string!(i18n, ticket.action.pending_thb_title),
        DepositMethod::Usdc => t_string!(i18n, ticket.action.pending_usdc_title),
        DepositMethod::CreditThb | DepositMethod::CreditUsdc => {
            t_string!(i18n, ticket.action.pending_credit_title)
        }
    };
    let desc = move || match method {
        DepositMethod::Thb => t_string!(i18n, ticket.action.pending_thb_desc),
        DepositMethod::Usdc => t_string!(i18n, ticket.action.pending_usdc_desc),
        DepositMethod::CreditThb | DepositMethod::CreditUsdc => {
            t_string!(i18n, ticket.action.pending_credit_desc)
        }
    };

    view! {
        <div class="ticket-action-card ticket-action-card--pending">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Hourglass class="icon-sm" />
            </div>
            <div>
                {move || view! {
                    <crate::components::StatusBadge
                        tone=crate::components::StatusTone::Pending
                        label=t_string!(i18n, ticket.action.pending)
                    />
                }}
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
    // Staff-typed and clicked by the attendee: render only a safe link (.issues/145).
    let url = event_checkin_domain::validation::safe_document_link(&refund_proof_url)
        .unwrap_or_default()
        .to_string();
    view! {
        <div class="ticket-action-card ticket-action-card--refund">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Recycle class="icon-sm" />
            </div>
            <div>
                <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.refund_returned))}</div>
                {if !url.is_empty() {
                    view! {
                        <a
                            href=url
                            target="_blank"
                            rel="noopener noreferrer"
                            class="ticket-action-link"
                        >
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.refund_receipt))}
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
                <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.claim_title))}</div>
                <a href=claim_href class="btn btn-primary btn-sm ticket-action-btn">
                    <Icon icon=IconName::Gift class="icon-sm" />
                    " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.claim_cta))}
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
                <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.reclaim_title))}</div>
                <div class="ticket-action-desc">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.reclaim_desc))}
                </div>
                <a href=reclaim_href class="btn btn-success btn-sm ticket-action-btn">
                    <Icon icon=IconName::CreditCard class="icon-sm" />
                    " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.reclaim_cta))}
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
                <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.moved_online_title))}</div>
                <div class="ticket-action-desc">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.moved_online_desc))}
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
    Error(RolloverError),
}

/// Why a rollover failed. Our own wording is rendered in the reader's
/// language; text from the server or the wallet is passed through.
#[derive(Clone)]
enum RolloverError {
    /// Building the transaction failed (server error text).
    Build(String),
    /// The server returned an empty transaction.
    Empty,
    /// Simulation says the transaction would fail (simulator text, if any).
    WouldFail(Option<String>),
    /// The wallet failed without a usable reason.
    Failed,
    /// A message that is already written for the reader (wallet / cluster check).
    Message(String),
}

/// Rollover deposit card — self-contained wallet signing flow.
///
/// Shown when attendee has a verified USDC deposit on a past event
/// and a new event from the same organizer is available.
#[component]
pub fn RolloverActionCard(
    /// Source deposit amount in USDC smallest units.
    deposit_amount_usdc: u64,
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
    let i18n = use_i18n();

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
                        &wallet_error::user_friendly_message(&e, i18n.get_locale_untracked()),
                        ToastType::Error,
                    );
                }
                wallet_error::WalletResult::UnknownFailure => {
                    components::show_toast(
                        &set_toast,
                        td_string!(
                            i18n.get_locale_untracked(),
                            ticket.action.wallet_connect_failed
                        ),
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
                        <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.rollover_title))}</div>
                        <div class="ticket-action-desc">
                            {
                                let target = target_event_name.clone();
                                t!(i18n, ticket.action.rollover_desc, target)
                            }
                        </div>
                        <button
                            class="btn btn-primary btn-sm ticket-action-btn"
                            on:click=move |_| set_state.set(RolloverState::ChooseWallet)
                        >
                            <Icon icon=IconName::Refresh class="icon-sm" />
                            " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.rollover_cta))}
                        </button>
                    }.into_any(),

                    RolloverState::ChooseWallet => {
                        let wallets = detected_wallets.get();
                        view! {
                            <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.rollover_connect_title))}</div>
                            <div class="ticket-action-desc">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.rollover_connect_desc))}
                            </div>
                            {if wallets.is_empty() {
                                view! {
                                    <p class="ticket-action-desc ticket-action-alt-text">
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.no_wallet))}
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
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.cancel))}
                            </button>
                        }.into_any()
                    },

                    RolloverState::WalletConnected(wn, _pk) => {
                        let wn_display = wn.clone();
                        let amount = crate::pages::deposit::types::format_usdc(deposit_amount_usdc);
                        let cluster = utils::get_cluster();
                        let network = format!(
                            "Solana {}",
                            crate::pages::deposit::components::cluster_display_label(&cluster)
                        );
                        let sv_source = source_eid;
                        let sv_target = target_eid;
                        let sv_aid = aid_stored;
                        let ss = set_state;
                        view! {
                            <div class="ticket-action-title">
                                {t!(i18n, ticket.action.connected_via, wallet = wn_display)}
                            </div>
                            {
                                let move_word = t_string!(i18n, ticket.action.review_move);
                                let to_word = t_string!(i18n, ticket.action.review_to);
                                crate::pages::deposit::components::transaction_review(vec![
                                    (
                                        t_string!(i18n, ticket.action.review_authorize),
                                        format!("{move_word} {amount} USDC {to_word} {target_event_name}"),
                                    ),
                                    (t_string!(i18n, ticket.action.review_network), network),
                                    (
                                        t_string!(i18n, ticket.action.review_extra_payment),
                                        t_string!(i18n, ticket.action.review_none).to_string(),
                                    ),
                                    (
                                        t_string!(i18n, ticket.action.review_fee),
                                        t_string!(i18n, ticket.action.review_fee_value).to_string(),
                                    ),
                                ])
                            }
                            <div class="ticket-action-desc">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.review_hint))}
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
                                                ss.set(RolloverState::Error(RolloverError::Build(e.to_string())));
                                                return;
                                            }
                                        };
                                        let tx_b64 = resp.transaction;
                                        if tx_b64.is_empty() {
                                            ss.set(RolloverState::Error(RolloverError::Empty));
                                            return;
                                        }
                                        let expected_cluster = crate::utils::get_cluster();
                                        if let Err(cluster_err) =
                                            crate::pages::escrow_init::check_wallet_cluster(&wn_c, &expected_cluster).await
                                        {
                                            let cluster_err = cluster_err.message(i18n.get_locale_untracked());
                                            log::error!("[rollover] cluster mismatch: {cluster_err}");
                                            ss.set(RolloverState::Error(RolloverError::Message(cluster_err)));
                                            return;
                                        }
                                        match crate::pages::escrow_init::simulate_transaction_js(&wn_c, &tx_b64).await {
                                            Ok(sim) if sim.ok => {}
                                            Ok(sim) => {
                                                log::error!("[rollover] simulation failed: {:?}", sim.error);
                                                ss.set(RolloverState::Error(RolloverError::WouldFail(sim.error)));
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
                                                ss.set(RolloverState::Error(RolloverError::Message(
                                                    wallet_error::user_friendly_message(&e, i18n.get_locale_untracked()),
                                                )));
                                            }
                                            wallet_error::WalletResult::UnknownFailure => {
                                                ss.set(RolloverState::Error(RolloverError::Failed));
                                            }
                                        }
                                    });
                                }
                            >
                                <Icon icon=IconName::Refresh class="icon-sm" />
                                " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.rollover_sign))}
                            </button>
                            <button
                                class="btn btn-outline btn-xs ticket-action-cancel-xs"
                                on:click=move |_| set_state.set(RolloverState::Ready)
                            >
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.cancel))}
                            </button>
                        }.into_any()
                    },

                    RolloverState::Signing(_, _) => view! {
                        <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.rollover_processing))}</div>
                        <div class="ticket-action-desc ticket-action-signing-row">
                            <span class="spinner spinner-sm"></span>
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.approve_in_wallet))}
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
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.rollover_done))}
                            </div>
                            <div class="ticket-action-desc">
                                {
                                    let target = target_event_name.clone();
                                    t!(i18n, ticket.action.rollover_done_desc, target, tx = sig_short)
                                }
                            </div>
                            <a
                                href=solscan
                                target="_blank"
                                rel="noopener noreferrer"
                                class="ticket-action-link"
                            >
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.view_solscan))}
                            </a>
                        }.into_any()
                    },

                    RolloverState::Error(err) => view! {
                        <div class="ticket-action-title ticket-action-title-danger">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.rollover_failed))}
                        </div>
                        <div class="ticket-action-desc">{match err {
                            RolloverError::Build(error) => {
                                t!(i18n, ticket.action.err_build, error).into_any()
                            }
                            RolloverError::Empty => crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.err_empty)).into_any(),
                            RolloverError::WouldFail(Some(error)) => {
                                t!(i18n, ticket.action.err_would_fail, error).into_any()
                            }
                            RolloverError::WouldFail(None) => {
                                let error = crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.err_simulation));
                                t!(i18n, ticket.action.err_would_fail, error).into_any()
                            }
                            RolloverError::Failed => crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.err_failed)).into_any(),
                            RolloverError::Message(msg) => msg.into_any(),
                        }}</div>
                        <button
                            class="btn btn-outline btn-xs ticket-action-cancel-xs"
                            on:click=move |_| set_state.set(RolloverState::Ready)
                        >
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.try_again))}
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
    let i18n = use_i18n();

    view! {
        <div class="ticket-action-card ticket-action-card--hold">
            <div class="ticket-action-icon">
                <Icon icon=IconName::Save class="icon-sm" />
            </div>
            <div>
                {move || match state.get() {
                    HoldDepositState::Ready => view! {
                        <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.hold_title))}</div>
                        <div class="ticket-action-desc">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.hold_desc))}
                        </div>
                        <button
                            class="btn btn-outline btn-sm ticket-action-btn"
                            on:click=move |_| set_state.set(HoldDepositState::Confirm)
                        >
                            <Icon icon=IconName::Save class="icon-sm" />
                            " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.hold_cta))}
                        </button>
                    }.into_any(),

                    HoldDepositState::Confirm => view! {
                        <div class="ticket-action-title">{t!(i18n, ticket.action.hold_confirm_title, amount)}</div>
                        <div class="ticket-action-desc">
                            {t!(i18n, ticket.action.hold_confirm_desc, amount)}
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
                            " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.hold_confirm_cta))}
                        </button>
                        <button
                            class="btn btn-outline btn-xs ticket-action-cancel"
                            on:click=move |_| set_state.set(HoldDepositState::Ready)
                        >
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.cancel))}
                        </button>
                    }.into_any(),

                    HoldDepositState::Holding => view! {
                        <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.holding))}</div>
                        <div class="ticket-action-desc ticket-action-signing-row">
                            <span class="spinner spinner-sm"></span>
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.processing_request))}
                        </div>
                    }.into_any(),

                    HoldDepositState::Confirmed { credit_thb, credit_usdc } => {
                        // Shared formatter: USDC is the 6-decimal smallest unit and
                        // must not be printed raw (15_000_000 is $15, not $15M).
                        // Falls back to the amount just held if the server reports a
                        // zero balance — impossible right after a hold, but it keeps
                        // the sentence well-formed rather than emitting "Total credit: .".
                        let balance =
                            super::credit_chip::credit_balance_label(credit_thb, credit_usdc)
                                .unwrap_or_else(|| format!("{amount} THB"));
                        view! {
                            <div class="ticket-action-title ticket-action-title-success">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.held_title))}
                            </div>
                            <div class="ticket-action-desc">
                                {t!(i18n, ticket.action.held_desc, amount, balance)}
                            </div>
                        }.into_any()
                    },

                    HoldDepositState::AlreadyHeld => view! {
                        <div class="ticket-action-title ticket-action-title-success">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.held_title))}
                        </div>
                        <div class="ticket-action-desc">
                            {t!(i18n, ticket.action.already_held_desc, amount)}
                        </div>
                    }.into_any(),

                    HoldDepositState::Error(msg) => view! {
                        <div class="ticket-action-title ticket-action-title-danger">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.hold_failed))}
                        </div>
                        <div class="ticket-action-desc">{msg.clone()}</div>
                        <button
                            class="btn btn-outline btn-xs ticket-action-cancel-xs"
                            on:click=move |_| set_state.set(HoldDepositState::Ready)
                        >
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.try_again))}
                        </button>
                    }.into_any(),
                }}
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// RequestCreditRefundCard (Issue #061 Phase 3 — exit path)
// ---------------------------------------------------------------------------

/// State machine for the "Request Return of Held Credit" flow.
/// Mirrors `HoldDepositState` in shape — minimal, no wallet flow, just an
/// authenticated POST + a confirm step. Adds an initial `Loading` arm because
/// this card fetches the attendee's own flag state on mount (so a reload mounts
/// in `AlreadyRequested` if a prior request is open).
#[derive(Clone)]
enum RequestCreditRefundState {
    /// Initial fetch of the flag state is in flight.
    Loading,
    /// No open request — show CTA.
    Ready,
    /// Confirmation step explaining what the request does (and does NOT) do.
    Confirm,
    /// POST in flight.
    Requesting,
    /// Server reports an open request from a prior call (page reload).
    /// Distinct from `Requested` (no in-session success message to display).
    AlreadyRequested,
    /// Success — this call set the flag.
    Requested { message: String },
    /// Error.
    Error(String),
}

/// Request Return of Held Credit action card — Phase 3 exit path so "hold
/// forever" doesn't feel like a trap (Issue #061 §D3). Sets a visibility-only
/// flag on the attendee's contact row; the organizer processes the actual
/// payout through the existing refund tooling.
///
/// Rendered on the ticket page only when the attendee has already held their
/// deposit as credit (`dep.held_as_credit == true`) — the exit path is
/// meaningful only when there is held credit to exit from.
///
/// Backend:
/// - `POST /api/deposit/request-credit-refund` (JWT-gated, no body — email
///   comes from claims; VULN-012 pattern).
/// - `GET /api/deposit/credit-refund-request` reads own flag state so a reload
///   mounts in `AlreadyRequested` (mirrors the `held_as_credit` UX pattern).
#[component]
pub fn RequestCreditRefundCard() -> impl IntoView {
    let (state, set_state) = signal(RequestCreditRefundState::Loading);

    // On mount: fetch the attendee's own flag state. If already requested,
    // mount in `AlreadyRequested` (mirrors the `held_as_credit` UX pattern —
    // the backend idempotency is the safety net; this is the UX). A failed
    // read degrades to `Ready` so the attendee can still trigger the request
    // (the write path is idempotent — a duplicate just re-stamps the timestamp).
    // Signed out also reads as `Ready`: the read never redirects, because this
    // card mounts on the public ticket page (`.issues/142`); pressing the button
    // is what asks a signed-out attendee to sign in.
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            let requested = api::get_credit_refund_request_status()
                .await
                .is_some_and(|status| status.requested);
            set_state.set(match requested {
                true => RequestCreditRefundState::AlreadyRequested,
                false => RequestCreditRefundState::Ready,
            });
        });
    });

    view! {
        <div class="ticket-action-card ticket-action-card--credit-refund">
            <div class="ticket-action-icon">
                <Icon icon=IconName::MoneyWings class="icon-sm" />
            </div>
            <div>
                {move || match state.get() {
                    RequestCreditRefundState::Loading => view! {
                        <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_checking))}</div>
                        <div class="ticket-action-desc ticket-action-signing-row">
                            <span class="spinner spinner-sm"></span>
                        </div>
                    }.into_any(),

                    RequestCreditRefundState::Ready => view! {
                        <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_title))}</div>
                        <div class="ticket-action-desc">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_desc))}
                        </div>
                        <button
                            class="btn btn-outline btn-sm ticket-action-btn"
                            on:click=move |_| set_state.set(RequestCreditRefundState::Confirm)
                        >
                            <Icon icon=IconName::Check class="icon-sm" />
                            " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_cta))}
                        </button>
                    }.into_any(),

                    RequestCreditRefundState::Confirm => view! {
                        <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_confirm_title))}</div>
                        <div class="ticket-action-desc">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_confirm_desc))}
                        </div>
                        <button
                            class="btn btn-primary btn-sm ticket-action-btn"
                            on:click=move |_| {
                                set_state.set(RequestCreditRefundState::Requesting);
                                leptos::task::spawn_local(async move {
                                    match api::request_credit_refund().await {
                                        Ok(resp) => {
                                            log::info!("[credit-refund] request submitted");
                                            set_state.set(RequestCreditRefundState::Requested {
                                                message: resp.message,
                                            });
                                        }
                                        Err(e) => {
                                            log::error!("[credit-refund] failed: {}", e.message);
                                            set_state.set(RequestCreditRefundState::Error(e.message));
                                        }
                                    }
                                });
                            }
                        >
                            <Icon icon=IconName::Check class="icon-sm" />
                            " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_confirm_cta))}
                        </button>
                        <button
                            class="btn btn-outline btn-xs ticket-action-cancel"
                            on:click=move |_| set_state.set(RequestCreditRefundState::Ready)
                        >
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.cancel))}
                        </button>
                    }.into_any(),

                    RequestCreditRefundState::Requesting => view! {
                        <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_submitting))}</div>
                        <div class="ticket-action-desc ticket-action-signing-row">
                            <span class="spinner spinner-sm"></span>
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.processing_request))}
                        </div>
                    }.into_any(),

                    RequestCreditRefundState::AlreadyRequested => view! {
                        <div class="ticket-action-title ticket-action-title-success">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_requested))}
                        </div>
                        <div class="ticket-action-desc">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_requested_desc))}
                        </div>
                    }.into_any(),

                    RequestCreditRefundState::Requested { message } => view! {
                        <div class="ticket-action-title ticket-action-title-success">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_submitted))}
                        </div>
                        <div class="ticket-action-desc">{message.clone()}</div>
                    }.into_any(),

                    RequestCreditRefundState::Error(msg) => view! {
                        <div class="ticket-action-title ticket-action-title-danger">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_failed))}
                        </div>
                        <div class="ticket-action-desc">{msg.clone()}</div>
                        <button
                            class="btn btn-outline btn-xs ticket-action-cancel-xs"
                            on:click=move |_| set_state.set(RequestCreditRefundState::Ready)
                        >
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.try_again))}
                        </button>
                    }.into_any(),
                }}
            </div>
        </div>
    }
}
