//! Refund flow views (RefundChooseWallet, RefundWalletConnected, RefundSigning, RefundConfirmed).

use leptos::prelude::*;

use crate::api::DepositStatusResponse;
use crate::icons::{wallet_icon_name, Icon};

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

    let handle_connect = handle_refund_connect_wallet.clone();

    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <span class="dep2-card-title">"Claim Refund"</span>
                <span class="badge badge-info">
                    {format!("{usdc_fmt} USDC")}
                </span>
            </div>
            <p class="hint-desc">
                "Connect the wallet you used to deposit. Your refund will be sent there."
            </p>
            {if is_refundable {
                view! { <div></div> }.into_any()
            } else {
                view! {
                    <div class="dep2-deadline dep2-deadline--warning">
                        <span class="dep2-deadline-text">"Non-refundable deposit — no refund available"</span>
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

    let _wallet_icon = wallet_icon_name(wallet_name);
    let pk_display = truncate_pk(public_key);

    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <span class="dep2-card-title">"Claim Refund"</span>
                <span class="badge badge-info">
                    {format!("{usdc_fmt} USDC")}
                </span>
            </div>
            <div class="dep2-wallet-bar">
                <div class="dep2-wallet-bar-info">
                    <span class="dep2-wallet-bar-name">{format!("Connected via {}", wallet_name)}</span>
                    <span class="dep2-wallet-bar-pk">{pk_display}</span>
                </div>
                <span class="dep2-wallet-bar-badge">"Connected"</span>
            </div>
            <p class="hint-desc">
                "Your deposit is ready to be returned."
            </p>
            <button
                class="btn btn-success btn-block"
                on:click=move |_| handle_claim_refund(wallet_name_send.clone(), pk_send.clone())
            >
                "Claim "{format!("{usdc_fmt} USDC")}" USDC"
            </button>
            <button
                class="btn btn-outline btn-sm"
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
        <div class="dep2-card">
            <div class="dep2-card-header">
                <span class="dep2-card-title">"Processing Refund..."</span>
                <span class="badge badge-info">
                    {format!("{usdc_fmt} USDC")}
                </span>
            </div>
            <div class="dep2-confirming">
                <div class="dep2-confirming-dots">
                    <span class="dep2-confirming-dot"></span>
                    <span class="dep2-confirming-dot"></span>
                    <span class="dep2-confirming-dot"></span>
                </div>
                <p class="hint-desc">
                    "Please approve the transaction in your wallet..."
                </p>
            </div>
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
        <div class="dep2-card">
            <div class="dep2-success-icon">"✓"</div>
            <p class="dep2-amount-hero">
                {format!("{usdc_fmt} USDC + ~0.002 SOL returned")}
            </p>
            <p class="hint-desc">
                "Your refund has been confirmed on Solana."
            </p>
            <div class="dep2-receipt">
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">"TX"</span>
                    <span class="dep2-receipt-value">{sig_display}</span>
                </div>
            </div>
            {components::solscan_link(tx_sig)}
            <div class="dep2-back">
                <a href=if data_slug.is_empty() { "/".to_string() } else { format!("/e/{data_slug}") }>"← Back to event"</a>
            </div>
        </div>
    }
        .into_any()
}
