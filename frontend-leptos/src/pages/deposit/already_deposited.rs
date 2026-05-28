//! AlreadyDeposited + NotEnabled + Error + Loading views.

use leptos::prelude::*;

use crate::api::{DepositMethod, DepositStatusResponse};
use crate::icons::{Icon, IconName};
use crate::utils::format_timestamp;

use super::types::*;

/// Loading view.
pub fn loading_view() -> AnyView {
    view! {
        <div class="loading visible loading-top">
            <span class="spinner spinner-lg"></span>
            " Loading deposit info..."
        </div>
    }
    .into_any()
}

/// Error view.
pub fn error_view(msg: &str) -> AnyView {
    let msg = msg.to_string();
    view! {
        <div class="card dep-card-error">
            <div class="card-header">
                <h2 class="card-title"><Icon icon=IconName::Warning class="icon-sm icon-danger" />" Error"</h2>
            </div>
            <p class="hint-desc">
                {msg}
            </p>
            <a href="/" class="btn btn-primary">"Go Home"</a>
        </div>
    }
        .into_any()
}

/// Not enabled view.
pub fn not_enabled_view() -> AnyView {
    view! {
        <div class="card dep-card-error">
            <div class="card-header">
                <h2 class="card-title"><Icon icon=IconName::CreditCard class="icon-sm" />" Deposits Not Available"</h2>
            </div>
            <p class="hint-desc">
                "Deposits are not enabled for this event."
            </p>
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
        <div class="card dep-card-error">
            <div class="card-header">
                <h2 class="card-title"><Icon icon=IconName::Ticket class="icon-sm" />" Spot Reserved"</h2>
            </div>
            <div class="dep-details-block">
                <div class="dep-detail-row">
                    <span class="dep-label">"Method"</span>
                    <span><Icon icon=method_icon class="icon-sm" />" "{method_label}</span>
                </div>
                <div class="dep-detail-row">
                    <span class="dep-label">"Amount"</span>
                    <span>
                        {format!("{} {}", info_clone.as_ref().unwrap().amount, info_clone.as_ref().unwrap().currency)}
                    </span>
                </div>
                <div class="dep-detail-row-center">
                    <span class="dep-label">"Status"</span>
                    <span class=verified_class><Icon icon=verified_icon class="icon-sm" />" "{verified_text}</span>
                </div>
                <div class="dep-detail-row-last">
                    <span class="dep-label">"Date"</span>
                    <span>{format_timestamp(&info_clone.as_ref().unwrap().deposited_at)}</span>
                </div>
            </div>
            {if info.verified && info.method == DepositMethod::Usdc {
                let data_clone_for_refund = data_clone_for_refund.clone();
                let refund_info_clone = refund_info_clone.clone();
                view! {
                    <div class="dep-info-note">
                        <p class="hint-note">
                            <Icon icon=IconName::Coin class="icon-sm" />" "{format!("Your {usdc_fmt} USDC is secured on-chain. Show up → get it all back.")}
                        </p>
                    </div>
                    // Refund deadline urgency (loss aversion)
                    {match refund_info_clone {
                        Some((deadline_date, duration_label)) => view! {
                            <div class="dep-info-note">
                                <p class="hint-note">
                                    {format!("Refund window: {duration_label} after event ends ({deadline_date}).")}
                                </p>
                            </div>
                        }.into_any(),
                        None => view! { <div></div> }.into_any(),
                    }}
                    <button
                        class="btn btn-success btn-block btn-action-lg"
                        on:click=move |_| {
                            set_state.set(DepositPageState::RefundChooseWallet(data_clone_for_refund.clone()));
                        }
                    >
                        <Icon icon=IconName::Coin class="icon-sm" />" "{format!("Don't lose your {usdc_fmt} USDC — claim it now")}
                    </button>
                }.into_any()
            } else {
                view! {
                    <div class="dep-info-note">
                        <p class="hint-note">
                            <Icon icon=IconName::Coin class="icon-sm" />" "{format!("Your {usdc_fmt} USDC deposit is secured. Refund will be available after the event.")}
                        </p>
                    </div>
                }.into_any()
            }}
            <a href=if data_clone_for_event_link.event_slug.is_empty() { "/".to_string() } else { format!("/e/{}", data_clone_for_event_link.event_slug) } class="btn btn-primary action-row-top">"← Back to event"</a>
        </div>
    }
        .into_any()
}
