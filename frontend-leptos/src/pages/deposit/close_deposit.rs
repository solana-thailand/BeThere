//! Close deposit views (CloseDepositChooseWallet, CloseDepositWalletConnected, CloseDepositSigning, CloseDepositConfirmed).

use leptos::prelude::*;

use crate::api::DepositStatusResponse;
use crate::i18n::{t, use_i18n};
use crate::icons::{Icon, IconName, wallet_icon_name};
use crate::utils::get_cluster;

use super::components;
use super::types::*;

/// Close deposit: Choose wallet view.
pub fn close_deposit_choose_wallet_view(
    data: &DepositStatusResponse,
    detected_wallets: &[String],
    set_state: WriteSignal<DepositPageState>,
    handle_close_deposit_connect_wallet: impl Fn(String) + Clone + 'static,
) -> AnyView {
    let i18n = use_i18n();
    let wallets = detected_wallets.to_vec();
    let data_for_back = data.clone();

    let handle_connect = handle_close_deposit_connect_wallet.clone();

    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <span class="dep2-card-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.close.title))}</span>
                <span class="badge badge-info">"~0.002 SOL"</span>
            </div>
            <p class="hint-desc">
                {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.close.hint))}
            </p>
            {if wallets.is_empty() {
                components::wallet_fallback_view()
            } else {
                let wallets_for_click = wallets.clone();
                view! {
                    <div class="wallet-list">
                        {wallets_for_click.into_iter().map(|w| {
                            let w_clone = w.clone();
                            let wallet_icon = wallet_icon_name(&w);
                            let handle_connect = handle_connect.clone();
                            view! {
                                <button
                                    class="btn btn-primary btn-block wallet-btn-inner"
                                    on:click={
                                        let w = w.clone();
                                        move |_| handle_connect(w.clone())
                                    }
                                >
                                    <Icon icon=wallet_icon class="icon-md wallet-icon-white" />
                                    <span>{t!(i18n, deposit.connect_wallet, wallet = w_clone)}</span>
                                </button>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any()
            }}
            <button
                class="btn btn-outline btn-sm"
                on:click=move |_| {
                    set_state.set(DepositPageState::CloseDepositChooseWallet(data_for_back.clone()));
                }
            >
                {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.go_back))}
            </button>
        </div>
    }
        .into_any()
}

/// Close deposit: Wallet connected — ready to close.
pub fn close_deposit_wallet_connected_view(
    data: &DepositStatusResponse,
    wallet_name: &str,
    public_key: &str,
    set_state: WriteSignal<DepositPageState>,
    handle_close_deposit: impl Fn(String, String) + Clone + 'static,
) -> AnyView {
    let i18n = use_i18n();
    let wallet_name_send = wallet_name.to_string();
    let pk_send = public_key.to_string();
    let data_for_back = data.clone();
    let wallet = wallet_name.to_string();

    let _wallet_icon = wallet_icon_name(wallet_name);
    let pk_display = truncate_pk(public_key);
    let cluster = get_cluster();
    let network = format!("Solana {}", components::cluster_display_label(&cluster));

    let handle_close = handle_close_deposit.clone();

    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <span class="dep2-card-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.close.title))}</span>
                <span class="badge badge-info">"~0.002 SOL"</span>
            </div>
            <div class="dep2-wallet-bar">
                <div class="dep2-wallet-bar-info">
                    <span class="dep2-wallet-bar-name">{t!(i18n, deposit.connected_via, wallet)}</span>
                    <span class="dep2-wallet-bar-pk">{pk_display}</span>
                </div>
                <span class="dep2-wallet-bar-badge">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.connected))}</span>
            </div>
            {components::transaction_review_views(vec![
                (
                    view! { {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.review.authorize))} }.into_any(),
                    view! { {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.close.authorize_value))} }.into_any(),
                ),
                (view! { {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.review.network))} }.into_any(), network.into_any()),
                (
                    view! { {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.review.receive))} }.into_any(),
                    view! { {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.close.receive_value))} }.into_any(),
                ),
                (
                    view! { {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.review.fee))} }.into_any(),
                    view! { {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.review.fee_value))} }.into_any(),
                ),
            ])}
            <p class="hint-desc">
                {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.close.estimate))}
            </p>
            <button
                class="btn btn-success btn-block"
                on:click=move |_| handle_close(wallet_name_send.clone(), pk_send.clone())
            >
                {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.close.cta))}
            </button>
            <button
                class="btn btn-outline btn-sm"
                on:click=move |_| {
                    set_state.set(DepositPageState::CloseDepositChooseWallet(data_for_back.clone()));
                }
            >
                {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.go_back))}
            </button>
        </div>
    }
        .into_any()
}

/// Close deposit: Signing TX view.
pub fn close_deposit_signing_view() -> AnyView {
    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <span class="dep2-card-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.close.closing))}</span>
                <span class="badge badge-info">"~0.002 SOL"</span>
            </div>
            <div class="dep2-confirming">
                <div class="dep2-confirming-dots">
                    <span class="dep2-confirming-dot"></span>
                    <span class="dep2-confirming-dot"></span>
                    <span class="dep2-confirming-dot"></span>
                </div>
                <p class="hint-desc">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.approve_in_wallet))}
                </p>
            </div>
        </div>
    }
    .into_any()
}

/// Close deposit: Confirmed view.
pub fn close_deposit_confirmed_view(data: &DepositStatusResponse, tx_sig: &str) -> AnyView {
    let sig_display = truncate_sig(tx_sig);
    let data_slug = data.event_slug.clone();

    view! {
        <div class="dep2-card">
            <div class="dep2-success-icon"><Icon icon=IconName::Check class="icon-lg" /></div>
            <p class="dep2-amount-hero">
                {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.close.done))}
            </p>
            <div class="dep2-receipt">
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">"TX"</span>
                    <span class="dep2-receipt-value">{sig_display}</span>
                </div>
            </div>
            {components::solscan_link(tx_sig)}
            <div class="dep2-back">
                <a href=if data_slug.is_empty() { "/".to_string() } else { format!("/e/{data_slug}") }>{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.back_event))}</a>
            </div>
        </div>
    }
        .into_any()
}
