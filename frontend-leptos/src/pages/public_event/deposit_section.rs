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
        <div class="pe-card pe-deposit-card">
            <h2 class="pe-section-title">
                <Icon icon=IconName::Coin class="icon-md" />" Deposit Commitment"
            </h2>
            // Dual Payment Methods Grid (PromptPay & Solana USDC)
            <div class="pe-deposit-methods-grid">
                {if show_thb {
                    let thb = thb_display.clone().unwrap_or_default();
                    view! {
                        <div class="pe-method-card pe-method-card--promptpay">
                            <div class="pe-method-header">
                                <span class="pe-method-badge pe-method-badge--promptpay">"PromptPay"</span>
                                <span class="pe-method-amount-val">{thb}" ฿"</span>
                            </div>
                            <div class="pe-method-subtext">
                                "Off-Chain deposit via PromptPay QR Code using any Thai banking app."
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
                {if show_usdc {
                    let usdc = usdc_display.clone();
                    view! {
                        <div class="pe-method-card pe-method-card--solana">
                            <div class="pe-method-header">
                                <span class="pe-method-badge pe-method-badge--solana">"Solana USDC"</span>
                                <span class="pe-method-amount-val">{usdc}</span>
                            </div>
                            <div class="pe-method-subtext">
                                "On-Chain automatic escrow deposit via Solana Smart Contract."
                            </div>
                        </div>
                    }.into_any()
                } else if data.deposit_amount_usdc > 0.0 && escrow_closed {
                    let usdc = usdc_display.clone();
                    view! {
                        <div class="pe-method-card pe-method-card--solana" style="opacity: 0.6;">
                            <div class="pe-method-header">
                                <span class="pe-method-badge pe-method-badge--solana">"Solana (Closed)"</span>
                                <span class="pe-method-amount-val">{usdc}</span>
                            </div>
                            <div class="pe-method-subtext">
                                "Escrow pool for this event is closed."
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
            </div>

            // Refund policy checklist
            <div class="pe-refund-list">
                <div class="pe-refund-item">
                    <span class="pe-check">"✓"</span>
                    <span class="pe-refund-text">"100% Fully refundable when you attend the event."</span>
                </div>
                {if show_usdc {
                    let refund = refund_label.clone();
                    view! {
                        <div class="pe-refund-item">
                            <span class="pe-check">"✓"</span>
                            <span class="pe-refund-text">
                                "USDC: Automatic payout via Solana smart contract within "{refund}" after check-in."
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
                                "THB: PromptPay transfer returned by organizer post-event."
                            </span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
            </div>

            // 4-step deposit journey infographic
            <div class="pe-journey-timeline">
                <div>
                    <div class="pe-journey-step-num" style="background: rgba(153,69,255,0.2); border: 1px solid rgba(153,69,255,0.5); color: #fff;">"1"</div>
                    <div style="font-size: 0.8rem; font-weight: 700; color: #fff;">"Reserve"</div>
                    <div style="font-size: 0.72rem; color: #94a3b8; margin-top: 2px;">"Lock spot"</div>
                </div>
                <div>
                    <div class="pe-journey-step-num" style="background: rgba(20,241,149,0.2); border: 1px solid rgba(20,241,149,0.5); color: #14F195;">"2"</div>
                    <div style="font-size: 0.8rem; font-weight: 700; color: #fff;">"Show Up"</div>
                    <div style="font-size: 0.72rem; color: #94a3b8; margin-top: 2px;">"At venue"</div>
                </div>
                <div>
                    <div class="pe-journey-step-num" style="background: rgba(153,69,255,0.2); border: 1px solid rgba(153,69,255,0.5); color: #fff;">"3"</div>
                    <div style="font-size: 0.8rem; font-weight: 700; color: #fff;">"Scan QR"</div>
                    <div style="font-size: 0.72rem; color: #94a3b8; margin-top: 2px;">"Verify entry"</div>
                </div>
                <div>
                    <div class="pe-journey-step-num" style="background: rgba(20,241,149,0.25); border: 1px solid #14F195; color: #14F195;">"4"</div>
                    <div style="font-size: 0.8rem; font-weight: 700; color: #14F195;">"100% Refund"</div>
                    <div style="font-size: 0.72rem; color: #94a3b8; margin-top: 2px;">"Back to wallet"</div>
                </div>
            </div>

            // Hybrid note
            {if is_hybrid {
                view! {
                    <div class="pe-hybrid-note" style="margin-top: 16px;">
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
