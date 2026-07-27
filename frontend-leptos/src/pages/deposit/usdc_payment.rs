//! USDC payment flow views (WalletConnected, AwaitingConfirmation, UsdcQrReady, DepositConfirmed).

use leptos::prelude::*;

use crate::api::DepositStatusResponse;
use crate::icons::{wallet_icon_name, Icon, IconName};

use super::components;
use super::js_interop;
use super::types::*;

/// Wallet connected — ready to send deposit TX.
pub fn wallet_connected_view(
    data: &DepositStatusResponse,
    wallet_name: &str,
    public_key: &str,
    handle_send_deposit: impl Fn(String, String) + Clone + 'static,
    set_state: WriteSignal<DepositPageState>,
    set_payment_choice: WriteSignal<Option<PaymentChoice>>,
) -> AnyView {
    let wallet_name_send = wallet_name.to_string();
    let pk_send = public_key.to_string();
    let usdc_fmt = format_usdc(data.deposit_amount_usdc);
    let data_clone = data.clone();
    let handle_send_deposit = handle_send_deposit.clone();
    let wallet_icon = wallet_icon_name(wallet_name);
    let pk_short = truncate_pk(public_key);

    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <span class="dep2-card-title">"USDC Deposit"</span>
                <span class="badge badge-info">{format!("{usdc_fmt} USDC")}</span>
            </div>
            <div class="dep2-amount-hero">
                {usdc_fmt.to_string()}
                <span class="dep2-amount-unit">" USDC"</span>
            </div>
            <div class="dep2-wallet-bar">
                <Icon icon=wallet_icon class="dep2-wallet-bar-icon wallet-icon-white" />
                <div class="dep2-wallet-bar-info">
                    <div class="dep2-wallet-bar-name">{format!("Connected via {wallet_name}")}</div>
                    <div class="dep2-wallet-bar-pk">{pk_short}</div>
                </div>
                <span class="dep2-wallet-bar-badge">"Connected"</span>
            </div>
            <p class="hint-desc">
                "Tap below to approve the transaction in your wallet."
            </p>
            <button
                class="btn btn-success btn-block"
                on:click=move |_| handle_send_deposit(wallet_name_send.clone(), pk_send.clone())
            >
                {format!("Send {usdc_fmt} USDC")}
            </button>
            <button
                class="btn btn-outline btn-sm"
                on:click=move |_| {
                    set_payment_choice.set(None);
                    set_state.set(DepositPageState::ChoosePayment(data_clone.clone()));
                }
            >
                "← Go Back"
            </button>
        </div>
    }
        .into_any()
}

/// Awaiting confirmation — polling for TX.
pub fn awaiting_confirmation_view(
    data: &DepositStatusResponse,
    _wallet_name: &str,
    tx_sig: &str,
    _state: ReadSignal<DepositPageState>,
    _set_state: WriteSignal<DepositPageState>,
    _set_toast: WriteSignal<Option<crate::components::ToastMessage>>,
    params: DepositParamsSignal,
    handle_poll_confirmation: impl Fn(String, String, String) + Clone + 'static,
) -> AnyView {
    let event_id = super::types::extract_event_id_from_url();
    let attendee_id = match params.get() {
        Ok(p) => p.attendee_id.unwrap_or_default(),
        Err(_) => String::new(),
    };
    let sig_display = truncate_sig(tx_sig);
    let usdc_fmt = format_usdc(data.deposit_amount_usdc);

    // Trigger confirmation polling
    let eid = event_id.unwrap_or_default();
    let aid = attendee_id.clone();
    let sig = tx_sig.to_string();
    let handle_poll = handle_poll_confirmation.clone();
    Effect::new(move |_| {
        let eid_c = eid.clone();
        let aid_c = aid.clone();
        let sig_c = sig.clone();
        let handle_poll = handle_poll.clone();
        leptos::task::spawn_local(async move {
            handle_poll(eid_c, aid_c, sig_c);
        });
    });

    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <span class="dep2-card-title">"Confirming Deposit..."</span>
                <span class="badge badge-info">{format!("{usdc_fmt} USDC")}</span>
            </div>
            <div class="dep2-confirming">
                <div class="dep2-confirming-dots">
                    <span class="dep2-confirming-dot"></span>
                    <span class="dep2-confirming-dot"></span>
                    <span class="dep2-confirming-dot"></span>
                </div>
                <p>"Waiting for on-chain confirmation..."</p>
                <p class="hint-xs">"Usually 5-15 seconds. Don't close this page."</p>
            </div>
            <div class="tx-hash-box-top">
                {format!("TX: {}", &sig_display)}
            </div>
        </div>
    }
        .into_any()
}

/// Deposit confirmed on-chain.
pub fn deposit_confirmed_view(
    data: &DepositStatusResponse,
    tx_sig: &str,
    params: DepositParamsSignal,
) -> AnyView {
    let sig_display = truncate_sig(tx_sig);
    let usdc_fmt = format_usdc(data.deposit_amount_usdc);
    let refund_info = compute_refund_info(data);

    let ticket_attendee_id = match params.get() {
        Ok(p) => p.attendee_id.unwrap_or_default(),
        Err(_) => String::new(),
    };
    let ticket_event_id = extract_event_id_from_url().unwrap_or_default();
    let ticket_href = format!("/ticket/{ticket_attendee_id}?event_id={ticket_event_id}");

    let data_clone = data.clone();
    let data_clone_slug = data.clone();

    view! {
        <div class="dep2-card">
            <div class="dep2-success-icon">
                <Icon icon=IconName::Check class="icon-lg" />
            </div>
            <div class="dep2-amount-hero">
                {usdc_fmt.to_string()}
                <span class="dep2-amount-unit">" USDC deposited"</span>
            </div>
            <p class="hint-desc">
                "You're confirmed! Your spot is secured on Solana."
            </p>
            <div class="dep2-receipt">
                {
                    let status = &data_clone.status;
                    
                    match status {
                        Some(s) if !s.refundable => view! {
                            <div class="dep2-receipt-row">
                                <span class="dep2-receipt-label">"Status"</span>
                                <span class="dep2-receipt-value">
                                    <span class="badge badge-warning">
                                        "Non-refundable (#" {s.deposit_order} ")"
                                    </span>
                                </span>
                            </div>
                        }.into_any(),
                        Some(s) => view! {
                            <div class="dep2-receipt-row">
                                <span class="dep2-receipt-label">"Status"</span>
                                <span class="dep2-receipt-value">
                                    <span class="badge badge-success">
                                        "Refundable (#" {s.deposit_order} ")"
                                    </span>
                                </span>
                            </div>
                        }.into_any(),
                        _ => view! { <div></div> }.into_any(),
                    }
                }
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">"TX"</span>
                    <span class="dep2-receipt-value">{sig_display.clone()}</span>
                </div>
            </div>
            {components::solscan_link(tx_sig)}
            // Refund deadline info
            {match refund_info {
                Some((deadline_date, duration_label)) => view! {
                    <div class="dep2-deadline--warning">
                        <p class="hint-note">
                            {format!("Refund window: {duration_label} after the event ends ({deadline_date}). Don't lose your deposit — claim it back.")}
                        </p>
                    </div>
                }.into_any(),
                None => view! {
                    <div class="dep2-deadline--warning">
                        <p class="hint-note">"Refund will be available after the event."</p>
                    </div>
                }.into_any(),
            }}
            <div class="action-row-top-lg">
                <a href=ticket_href class="btn btn-primary">"View Your Ticket →"</a>
                <a href=if data_clone_slug.event_slug.is_empty() { "/".to_string() } else { format!("/e/{}", data_clone_slug.event_slug) } class="btn btn-outline">"← Back to event"</a>
            </div>
        </div>
    }
        .into_any()
}

/// USDC QR ready view — QR code display with polling.
#[allow(clippy::too_many_arguments)]
pub fn usdc_qr_ready_view(
    data: &DepositStatusResponse,
    pay_url: &str,
    pay_url_copied: ReadSignal<bool>,
    handle_copy_url: impl Fn(String) + Clone + 'static,
    handle_qr_poll: impl Fn() + Clone + 'static,
) -> AnyView {
    let _pay_url_display = pay_url.to_string();
    let pay_url_copy = pay_url.to_string();
    let pay_url_qr = pay_url.to_string();
    let usdc_fmt = format_usdc(data.deposit_amount_usdc);

    // Trigger QR confirmation polling on mount
    let poll = handle_qr_poll.clone();
    Effect::new(move |_| {
        let poll = poll.clone();
        leptos::task::spawn_local(async move {
            poll();
        });
    });

    let handle_copy_url = handle_copy_url.clone();
    let data_slug = data.event_slug.clone();

    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <span class="dep2-card-title">"Scan to Pay"</span>
                <span class="badge badge-info">{format!("{usdc_fmt} USDC")}</span>
            </div>
            <div class="dep2-qr-primary">
                {move || {
                    match js_interop::generate_qr_data_url(&pay_url_qr, 256) {
                        Some(url) => view! {
                            <div class="qr-wrapper">
                                <img src=url alt="Solana Pay QR" class="qr-img-lg" />
                            </div>
                        }.into_any(),
                        None => view! { <div></div> }.into_any(),
                    }
                }}
                <button
                    class=move || if pay_url_copied.get() { "btn btn-success btn-sm" } else { "btn btn-outline btn-sm" }
                    on:click=move |_| handle_copy_url(pay_url_copy.clone())
                >
                    {move || view! { <Icon icon=if pay_url_copied.get() { IconName::Check } else { IconName::Copy } class="icon-sm" /> }}
                    " " {move || if pay_url_copied.get() { "Copied!" } else { "Copy Link" }}
                </button>
            </div>
            <div class="dep2-qr-polling">
                <span class="spinner spinner-sm"></span>
                " Checking for payment..."
            </div>
            <p class="hint-2xs u-mt-1rem">
                "After payment, your deposit will be verified automatically."
            </p>
        </div>

        <a href=if data_slug.is_empty() { "/".to_string() } else { format!("/e/{data_slug}") } class="dep2-back">
            "← Back to event"
        </a>
    }
        .into_any()
}
