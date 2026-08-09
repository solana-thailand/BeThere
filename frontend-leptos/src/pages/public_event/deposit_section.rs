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
                        <div class="pe-method-row" style="background: rgba(0, 59, 114, 0.25); border: 1px solid rgba(56, 189, 248, 0.35); border-radius: 12px; padding: 14px 16px; margin-bottom: 10px;">
                            <div style="display: flex; align-items: center; justify-content: space-between; width: 100%;">
                                <span style="display: flex; align-items: center; gap: 8px; font-weight: 700; color: #38bdf8;">
                                    <span style="background: #003b72; color: #fff; padding: 3px 8px; border-radius: 6px; font-size: 0.75rem; font-weight: 800; letter-spacing: 0.02em;">"PromptPay"</span>
                                    "THB Deposit"
                                </span>
                                <span class="pe-method-amount" style="color: #fff; font-weight: 700; font-size: 1.05rem;">{thb}" ฿"</span>
                            </div>
                            <div style="font-size: 0.78rem; color: #94a3b8; margin-top: 6px; display: flex; align-items: center; gap: 6px;">
                                "📲 สแกนชำระผ่านแอปธนาคารไทยได้ทุกธนาคาร (KPlus, SCB, Krungthai ฯลฯ)"
                            </div>
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
            // 4-step deposit journey infographic (bilingual TH/EN)
            <div style="display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px; margin-top: 18px; padding: 14px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; text-align: center;">
                <div>
                    <div style="width: 28px; height: 28px; border-radius: 50%; background: rgba(153,69,255,0.2); border: 1px solid rgba(153,69,255,0.5); color: #fff; font-size: 0.8rem; font-weight: 700; display: flex; align-items: center; justify-content: center; margin: 0 auto 6px;">"1"</div>
                    <div style="font-size: 0.78rem; font-weight: 600; color: #fff;">"Reserve"</div>
                    <div style="font-size: 0.7rem; color: #94a3b8;">"สำรองที่นั่ง"</div>
                </div>
                <div>
                    <div style="width: 28px; height: 28px; border-radius: 50%; background: rgba(20,241,149,0.2); border: 1px solid rgba(20,241,149,0.5); color: #14F195; font-size: 0.8rem; font-weight: 700; display: flex; align-items: center; justify-content: center; margin: 0 auto 6px;">"2"</div>
                    <div style="font-size: 0.78rem; font-weight: 600; color: #fff;">"Show Up"</div>
                    <div style="font-size: 0.7rem; color: #94a3b8;">"เข้าร่วมงาน"</div>
                </div>
                <div>
                    <div style="width: 28px; height: 28px; border-radius: 50%; background: rgba(153,69,255,0.2); border: 1px solid rgba(153,69,255,0.5); color: #fff; font-size: 0.8rem; font-weight: 700; display: flex; align-items: center; justify-content: center; margin: 0 auto 6px;">"3"</div>
                    <div style="font-size: 0.78rem; font-weight: 600; color: #fff;">"Scan QR"</div>
                    <div style="font-size: 0.7rem; color: #94a3b8;">"สแกนเช็คอิน"</div>
                </div>
                <div>
                    <div style="width: 28px; height: 28px; border-radius: 50%; background: rgba(20,241,149,0.25); border: 1px solid #14F195; color: #14F195; font-size: 0.8rem; font-weight: 700; display: flex; align-items: center; justify-content: center; margin: 0 auto 6px;">"4"</div>
                    <div style="font-size: 0.78rem; font-weight: 600; color: #14F195;">"100% Refund"</div>
                    <div style="font-size: 0.7rem; color: #94a3b8;">"รับมัดจำคืน"</div>
                </div>
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
