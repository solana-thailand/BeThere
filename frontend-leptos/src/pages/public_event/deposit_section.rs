use super::types::*;
use crate::icons::{Icon, IconName};
use leptos::prelude::*;

pub fn deposit_section(data: &PublicEventData) -> AnyView {
    let is_online_only = data.event_format == crate::api::EventFormat::Online;
    let is_hybrid = data.event_format == crate::api::EventFormat::Hybrid;
    let has_deposit =
        data.deposit_enabled && (data.deposit_amount_usdc > 0.0 || data.deposit_amount_thb > 0.0);

    if !has_deposit || is_online_only {
        return ().into_any();
    }

    let escrow_status = data.escrow_status.as_deref().unwrap_or("");
    let escrow_closed =
        escrow_status == "closed" || escrow_status == "cancelled" || escrow_status == "deactivated";

    let usdc_display = format_usdc(data.deposit_amount_usdc);
    let thb_display = if data.deposit_amount_thb > 0.0 {
        Some(format_thb(data.deposit_amount_thb))
    } else {
        None
    };
    let refund_label = format_refund_deadline(data.refund_deadline_hours);

    let show_usdc = data.deposit_amount_usdc > 0.0 && !escrow_closed;
    let show_thb = data.deposit_amount_thb > 0.0;

    view! {
        <div class="pe-card">
            <h2 class="pe-section-title">
                <Icon icon=IconName::Coin class="icon-md" />" Deposit Commitment"
            </h2>
            // Payment method details
            <div class="pe-method-list">
                {if show_thb {
                    let thb = thb_display.clone().unwrap_or_default();
                    view! {
                        <div class="pe-method-row">
                            <span class="pe-method-label">"THB"</span>
                            <span class="pe-method-amount">{thb}" Baht"</span>
                            <span class="pe-method-via">"via PromptPay"</span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
                {if show_usdc {
                    let usdc = usdc_display.clone();
                    view! {
                        <div class="pe-method-row">
                            <span class="pe-method-label">"USDC"</span>
                            <span class="pe-method-amount">{usdc}</span>
                            <span class="pe-method-via">"via Solana"</span>
                        </div>
                    }.into_any()
                } else if data.deposit_amount_usdc > 0.0 && escrow_closed {
                    let usdc = usdc_display.clone();
                    view! {
                        <div class="pe-method-row pe-method-row--closed">
                            <span class="pe-method-label pe-method-label--closed">"USDC"</span>
                            <span class="pe-method-amount pe-method-amount--closed">{usdc}</span>
                            <span class="pe-method-via">"(closed)"</span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
            </div>
            // Refund policy
            <div class="pe-refund-list">
                <div class="pe-refund-item">
                    <span class="pe-check">"✓"</span>
                    <span class="pe-refund-text">"Fully refundable when you show up"</span>
                </div>
                {if show_usdc {
                    let refund = refund_label.clone();
                    view! {
                        <div class="pe-refund-item">
                            <span class="pe-check">"✓"</span>
                            <span class="pe-refund-text">
                                "USDC: Automatic refund via Solana smart contract within "{refund}" after event"
                            </span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
                {if show_thb {
                    view! {
                        <div class="pe-refund-item">
                            <span class="pe-check">"✓"</span>
                            <span class="pe-refund-text">
                                "THB: Refund by organizer via PromptPay (may take 1-2 days after event)"
                            </span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
            </div>
            // Hybrid note
            {if is_hybrid {
                view! {
                    <div class="pe-hybrid-note">
                        <span>"💡"</span>
                        <span>"Deposit applies to In-Person track only. Online participants are exempt."</span>
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }}
        </div>
    }.into_any()
}
