//! AlreadyDeposited + NotEnabled + Error + Loading views.

use leptos::prelude::*;

use crate::api::{DepositMethod, DepositStatusResponse};
use crate::icons::{Icon, IconName};
use crate::utils::format_timestamp;

use super::types::*;

/// Loading view.
pub fn loading_view() -> AnyView {
    view! {
        <div class="dep2-card">
            <div class="dep2-confirming">
                <div class="dep2-confirming-dots">
                    <span class="dep2-confirming-dot"></span>
                    <span class="dep2-confirming-dot"></span>
                    <span class="dep2-confirming-dot"></span>
                </div>
                <p>"Loading deposit info..."</p>
            </div>
        </div>
    }
    .into_any()
}

/// Error view.
pub fn error_view(msg: &str) -> AnyView {
    let msg = msg.to_string();
    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <Icon icon=IconName::Warning class="icon-sm icon-danger" />
                <span class="dep2-card-title">"Something went wrong"</span>
            </div>
            <p>{msg}</p>
            <a href="/" class="btn btn-primary">"Go Home"</a>
        </div>
    }
    .into_any()
}

/// Not enabled view.
pub fn not_enabled_view() -> AnyView {
    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <Icon icon=IconName::CreditCard class="icon-sm" />
                <span class="dep2-card-title">"Deposits Not Available"</span>
            </div>
            <p>"Deposits are not enabled for this event."</p>
            <a href="/" class="btn btn-primary">"Go Home"</a>
        </div>
    }
    .into_any()
}

/// Already deposited view.
pub fn already_deposited_view(
    data: &DepositStatusResponse,
    set_state: &WriteSignal<DepositPageState>,
) -> AnyView {
    let info = data.status.as_ref().unwrap();
    let (method_icon, method_label) = deposit_method_display(&info.method);
    let (verified_icon, verified_text) = if info.verified {
        (IconName::Check, "Verified")
    } else {
        (IconName::Hourglass, "Pending Verification")
    };
    let verified_class = if info.verified {
        "badge badge-success"
    } else {
        "badge badge-warning"
    };
    let usdc_fmt = format_usdc(data.deposit_amount_usdc);
    let refund_info = compute_refund_info(&data);

    let data_clone_for_refund = data.clone();
    let refund_info_clone = refund_info.clone();
    let data_clone_for_event_link = data.clone();
    let info_clone = data.status.clone();

    let set_state = *set_state;

    view! {
        <div class="dep2-card">
            // Header: title + badge
            <div class="dep2-card-header">
                <Icon icon=IconName::Ticket class="icon-sm" />
                <span class="dep2-card-title">"Spot Reserved"</span>
                <span class=verified_class>
                    <Icon icon=verified_icon class="icon-sm" />" "{verified_text}
                </span>
            </div>

            // Success icon (only if verified)
            {if info.verified {
                view! {
                    <div class="dep2-success-icon">
                        <Icon icon=IconName::Check />
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }}

            // Amount hero
            <div class="dep2-amount-hero">
                {format!("{} {}", info_clone.as_ref().unwrap().amount, info_clone.as_ref().unwrap().currency)}
                " deposited"
            </div>

            // Receipt block
            <div class="dep2-receipt">
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">"Method"</span>
                    <span class="dep2-receipt-value">
                        <Icon icon=method_icon class="icon-sm" />" "{method_label}
                    </span>
                </div>
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">"Amount"</span>
                    <span class="dep2-receipt-value">
                        {format!("{} {}", info_clone.as_ref().unwrap().amount, info_clone.as_ref().unwrap().currency)}
                    </span>
                </div>
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">"Status"</span>
                    <span class="dep2-receipt-value">
                        <span class=verified_class>
                            <Icon icon=verified_icon class="icon-sm" />" "{verified_text}
                        </span>
                    </span>
                </div>
                <div class="dep2-receipt-row">
                    <span class="dep2-receipt-label">"Date"</span>
                    <span class="dep2-receipt-value">
                        {format_timestamp(&info_clone.as_ref().unwrap().deposited_at)}
                    </span>
                </div>
            </div>

            // Refund info section
            {if info.verified && info.method == DepositMethod::Usdc {
                let data_clone_for_refund = data_clone_for_refund.clone();
                let refund_info_clone = refund_info_clone.clone();
                view! {
                    <div class="dep2-info-note">
                        <p class="hint-note">
                            <Icon icon=IconName::Coin class="icon-sm" />" "
                            {format!("Your {usdc_fmt} USDC is secured on-chain. Show up → get it all back.")}
                        </p>
                    </div>
                    // Refund deadline
                    {match refund_info_clone {
                        Some((deadline_date, duration_label)) => view! {
                            <div class="dep2-deadline dep2-deadline--warning">
                                <span class="dep2-deadline-text">
                                    {format!("Refund window: {duration_label} after event ends ({deadline_date}).")}
                                </span>
                            </div>
                        }.into_any(),
                        None => ().into_any(),
                    }}
                    // Refund CTA (only if refundable)
                    {if info.refundable {
                        let data_clone_for_refund = data_clone_for_refund.clone();
                        view! {
                            <div class="dep2-refund-cta">
                                <span class="dep2-refund-cta-text">
                                    {format!("Don't lose your {usdc_fmt} USDC — claim it now")}
                                </span>
                                <button
                                    class="btn btn-success btn-block"
                                    on:click=move |_| {
                                        set_state.set(DepositPageState::RefundChooseWallet(data_clone_for_refund.clone()));
                                    }
                                >
                                    "Claim Refund"
                                </button>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <span class="badge badge-muted">"Non-refundable deposit"</span>
                        }.into_any()
                    }}
                }.into_any()
            } else if !info.verified {
                view! {
                    <div class="dep2-info-note">
                        <p class="hint-note">
                            "Refund will be available after verification."
                        </p>
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="dep2-info-note">
                        <p class="hint-note">
                            <Icon icon=IconName::Coin class="icon-sm" />" "
                            {format!("Your {usdc_fmt} USDC deposit is secured. Refund will be available after the event.")}
                        </p>
                    </div>
                }.into_any()
            }}

            // Back to event link
            <a
                href=if data_clone_for_event_link.event_slug.is_empty() { "/".to_string() } else { format!("/e/{}", data_clone_for_event_link.event_slug) }
                class="dep2-back"
            >
                "← Back to event"
            </a>
        </div>
    }
        .into_any()
}
