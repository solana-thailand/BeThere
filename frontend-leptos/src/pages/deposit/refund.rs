//! Refund flow views (RefundChooseWallet, RefundWalletConnected, RefundSigning, RefundConfirmed).

use leptos::prelude::*;

use crate::api::DepositStatusResponse;
use crate::icons::{wallet_icon_name, Icon, IconName};

use super::components;
use super::types::*;

/// Refund: Choose wallet view.
pub fn refund_choose_wallet_view(
    data: &DepositStatusResponse,
    detected_wallets: &[String],
    set_state: WriteSignal<DepositPageState>,
    handle_refund_connect_wallet: impl Fn(String) + Clone + 'static,
) -> AnyView {
    let wallets = detected_wallets.to_vec();
    let data_for_back = data.clone();
    let usdc_fmt = format_usdc(data.deposit_amount_usdc);
    let is_refundable = data.status.as_ref().map(|s| s.refundable).unwrap_or(true);

    let set_state = set_state;
    let handle_connect = handle_refund_connect_wallet.clone();

    view! {
        <div class="card dep-card">
            <div class="card-header">
                <h2 class="card-title">"Claim Refund"</h2>
                <span class="badge badge-info">
                    {format!("{usdc_fmt} USDC")}
                </span>
            </div>
            <p class="hint-desc">
                "Connect the wallet you used to deposit. Your refund will be sent to the same wallet."
            </p>
            // Non-refundable check
            {if is_refundable {
                view! { <div></div> }.into_any()
            } else {
                view! {
                    <div class="badge badge-warning dep-refund-badge-mb">
                        "Non-refundable deposit — no refund available"
                    </div>
                }.into_any()
            }}
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
                                    <span>{format!("Connect {}", &w_clone)}</span>
                                </button>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any()
            }}
            <button
                class="btn btn-outline btn-sm"
                on:click=move |_| {
                    set_state.set(DepositPageState::AlreadyDeposited(data_for_back.clone()));
                }
            >
                "← Go Back"
            </button>
        </div>
    }
        .into_any()
}

/// Refund: Wallet connected — ready to claim.
pub fn refund_wallet_connected_view(
    data: &DepositStatusResponse,
    wallet_name: &str,
    public_key: &str,
    set_state: WriteSignal<DepositPageState>,
    handle_claim_refund: impl Fn(String, String) + Clone + 'static,
) -> AnyView {
    let wallet_name_send = wallet_name.to_string();
    let pk_send = public_key.to_string();
    let usdc_fmt = format_usdc(data.deposit_amount_usdc);
    let data_for_back = data.clone();
    let handle_claim_refund = handle_claim_refund.clone();

    let set_state = set_state;
    view! {
        <div class="card dep-card">
            <div class="card-header">
                <h2 class="card-title">"Claim Refund"</h2>
                <span class="badge badge-info">
                    {format!("{usdc_fmt} USDC")}
                </span>
            </div>
            {components::wallet_connected_bar(wallet_name, public_key)}
            <p class="hint-desc">
                "Your deposit is waiting to be returned. Click below to claim it."
            </p>
            <button
                class="btn btn-success btn-block btn-action-lg"
                on:click=move |_| handle_claim_refund(wallet_name_send.clone(), pk_send.clone())
            >
                <Icon icon=IconName::Recycle class="icon-sm" />" Claim "{format!("{usdc_fmt} USDC")}" — Don't lose it"
            </button>
            <button
                class="btn btn-outline btn-sm btn-action-secondary"
                on:click=move |_| {
                    set_state.set(DepositPageState::RefundChooseWallet(data_for_back.clone()));
                }
            >
                "← Go Back"
            </button>
        </div>
    }
        .into_any()
}

/// Refund: Signing TX view.
pub fn refund_signing_view(data: &DepositStatusResponse) -> AnyView {
    let usdc_fmt = format_usdc(data.deposit_amount_usdc);
    view! {
        <div class="card dep-card">
            <div class="card-header">
                <h2 class="card-title"><Icon icon=IconName::Hourglass class="icon-sm icon-warning" />" Processing Refund..."</h2>
                <span class="badge badge-info">
                    {format!("{usdc_fmt} USDC")}
                </span>
            </div>
            {components::spinner_loading("Please approve the transaction in your wallet...")}
        </div>
    }
        .into_any()
}

/// Refund: Confirmed view.
pub fn refund_confirmed_view(data: &DepositStatusResponse, tx_sig: &str) -> AnyView {
    let sig_display = truncate_sig(tx_sig);
    let usdc_fmt = format_usdc(data.deposit_amount_usdc);
    let data_slug = data.event_slug.clone();

    view! {
        <div class="card dep-card">
            <div class="card-header">
                <h2 class="card-title"><Icon icon=IconName::Party class="icon-sm" />" Refund Recovered & Rent Reclaimed!"</h2>
                <span class="badge badge-success">"On-chain verified"</span>
            </div>
            {components::celebration_icon(IconName::Recycle)}
            <p class="success-title">
                {format!("{usdc_fmt} USDC + ~0.002 SOL returned to your wallet")}
            </p>
            <p class="hint-desc">
                "Your refund has been confirmed on Solana and your deposit account has been closed. Both the USDC refund and rent lamports should appear in your wallet shortly."
            </p>
            {components::tx_hash_box(&sig_display)}
            {components::solscan_link(tx_sig)}
            <div class="action-row-top-lg">
                <a href=if data_slug.is_empty() { "/".to_string() } else { format!("/e/{data_slug}") } class="btn btn-primary">"← Back to event"</a>
            </div>
        </div>
    }
        .into_any()
}
