use super::types::*;
use crate::i18n::{t, t_string, use_i18n};
use crate::icons::{Icon, IconName};
use crate::utils::deposit_copy::{NEVER_FORFEITED, THB_REFUND_WINDOW};
use leptos::prelude::*;

pub fn deposit_section(data: &PublicEventData) -> AnyView {
    let i18n = use_i18n();
    let is_online_only = data.event_format == crate::api::EventFormat::Online;
    let is_hybrid = data.event_format == crate::api::EventFormat::Hybrid;
    let has_deposit =
        data.deposit_enabled && (data.deposit_amount_usdc > 0 || data.deposit_amount_thb > 0);

    if !has_deposit || is_online_only {
        return ().into_any();
    }

    let escrow_status = data.escrow_status.as_deref().unwrap_or("");
    let escrow_closed =
        escrow_status == "closed" || escrow_status == "cancelled" || escrow_status == "deactivated";

    let usdc_display = format_usdc(data.deposit_amount_usdc);
    let thb_display = if data.deposit_amount_thb > 0 {
        Some(data.deposit_amount_thb.to_string())
    } else {
        None
    };
    let refund_hours = data.refund_deadline_hours;

    let show_usdc = data.deposit_amount_usdc > 0 && !escrow_closed;
    let show_thb = data.deposit_amount_thb > 0;

    // The journey reads as a sum: pay, show up, check in, get it back = FREE.
    // "Free" is the net cost of attending, so every line must also say that the
    // deposit is paid first, or people reach the payment step surprised.
    let pay_label = match (show_thb, show_usdc) {
        (true, _) => {
            let amount = thb_display.clone().unwrap_or_default();
            view! { {t!(i18n, event.pay_thb, amount)} }.into_any()
        }
        (false, true) => {
            let amount = usdc_display.clone();
            view! { {t!(i18n, event.pay_usdc, amount)} }.into_any()
        }
        (false, false) => view! { {t!(i18n, event.pay_deposit)} }.into_any(),
    };
    let back_label = move || match show_thb {
        true => t_string!(i18n, event.back_refund_or_credit),
        false => t_string!(i18n, event.back_claim_to_wallet),
    };
    let free_detail = move || match (show_thb, show_usdc) {
        (true, true) => t_string!(i18n, event.free_detail_both),
        (true, false) => t_string!(i18n, event.free_detail_thb),
        (false, _) => t_string!(i18n, event.free_detail_usdc),
    };

    view! {
        <div class="pe-card pe-deposit-card">
            <h2 class="pe-section-title">
                <Icon icon=IconName::Coin class="icon-md" />" "{t!(i18n, event.deposit_title)}
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
                                {t!(i18n, event.promptpay_subtext)}
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
                                {t!(i18n, event.usdc_subtext)}
                            </div>
                        </div>
                    }.into_any()
                } else if data.deposit_amount_usdc > 0 && escrow_closed {
                    let usdc = usdc_display.clone();
                    view! {
                        <div class="pe-method-card pe-method-card--solana" style="opacity: 0.6;">
                            <div class="pe-method-header">
                                <span class="pe-method-badge pe-method-badge--solana">{t!(i18n, event.solana_closed)}</span>
                                <span class="pe-method-amount-val">{usdc}</span>
                            </div>
                            <div class="pe-method-subtext">
                                {t!(i18n, event.escrow_closed)}
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
                    <span class="pe-refund-text">{t!(i18n, event.refund_attend)}</span>
                </div>
                // Deposit model D1 (owner, 2026-09-28): nothing is forfeited.
                // Ask for notice, threaten nothing (docs/deposit-commitment-model.md §5).
                // The promise itself stays in `utils::deposit_copy` (one home,
                // English only for now); the catalog carries the words around it.
                {if show_thb {
                    let promise = NEVER_FORFEITED;
                    view! {
                        <div class="pe-refund-item">
                            <span class="pe-check">"✓"</span>
                            <span class="pe-refund-text">
                                {t!(i18n, event.refund_cant_make_it, promise)}
                            </span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
                {if show_usdc {
                    let deadline = move || format_refund_deadline(refund_hours, i18n.get_locale());
                    view! {
                        <div class="pe-refund-item">
                            <span class="pe-check">"✓"</span>
                            <span class="pe-refund-text">
                                {t!(i18n, event.refund_usdc, deadline)}
                            </span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
                {if show_thb {
                    let window = THB_REFUND_WINDOW;
                    view! {
                        <div class="pe-refund-item">
                            <span class="pe-check">"✓"</span>
                            <span class="pe-refund-text">
                                {t!(i18n, event.refund_thb, window)}
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
                    <div style="font-size: 0.8rem; font-weight: 700; color: #fff;">{t!(i18n, event.step_reserve)}</div>
                    <div style="font-size: 0.72rem; color: #94a3b8; margin-top: 2px;">{pay_label}</div>
                </div>
                <div>
                    <div class="pe-journey-step-num" style="background: rgba(20,241,149,0.2); border: 1px solid rgba(20,241,149,0.5); color: #14F195;">"2"</div>
                    <div style="font-size: 0.8rem; font-weight: 700; color: #fff;">{t!(i18n, event.step_show_up)}</div>
                    <div style="font-size: 0.72rem; color: #94a3b8; margin-top: 2px;">{t!(i18n, event.step_at_venue)}</div>
                </div>
                <div>
                    <div class="pe-journey-step-num" style="background: rgba(153,69,255,0.2); border: 1px solid rgba(153,69,255,0.5); color: #fff;">"3"</div>
                    <div style="font-size: 0.8rem; font-weight: 700; color: #fff;">{t!(i18n, event.step_scan_qr)}</div>
                    <div style="font-size: 0.72rem; color: #94a3b8; margin-top: 2px;">{t!(i18n, event.step_check_in)}</div>
                </div>
                <div>
                    <div class="pe-journey-step-num" style="background: rgba(20,241,149,0.25); border: 1px solid #14F195; color: #14F195;">"4"</div>
                    <div style="font-size: 0.8rem; font-weight: 700; color: #14F195;">{t!(i18n, event.step_back)}</div>
                    <div style="font-size: 0.72rem; color: #94a3b8; margin-top: 2px;">{back_label}</div>
                </div>
            </div>
            <div class="pe-journey-total" role="note">
                <span class="pe-journey-total-sum">
                    <span aria-hidden="true">"= "</span>{t!(i18n, event.free)}
                </span>
                <span class="pe-journey-total-detail">{free_detail}</span>
            </div>

            // Hybrid note
            {if is_hybrid {
                view! {
                    <div class="pe-hybrid-note" style="margin-top: 16px;">
                        <span>"💡"</span>
                        <span>{t!(i18n, event.hybrid_deposit_note)}</span>
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }}
        </div>
    }.into_any()
}
