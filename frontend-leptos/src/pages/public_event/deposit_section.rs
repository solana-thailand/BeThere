use super::types::*;
use crate::i18n::{t, use_i18n};
use crate::icons::{Icon, IconName};
use crate::utils::deposit_copy::{never_forfeited, thb_refund_window};
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

    view! {
        <div class="pe-card pe-deposit-card">
            <h2 class="pe-section-title">
                <Icon icon=IconName::Coin class="icon-md" />" "{crate::locale::tr(|l| crate::i18n::td_string!(l, event.deposit_title))}
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
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, event.promptpay_subtext))}
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
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, event.usdc_subtext))}
                            </div>
                        </div>
                    }.into_any()
                } else if data.deposit_amount_usdc > 0 && escrow_closed {
                    let usdc = usdc_display.clone();
                    view! {
                        <div class="pe-method-card pe-method-card--solana" style="opacity: 0.6;">
                            <div class="pe-method-header">
                                <span class="pe-method-badge pe-method-badge--solana">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.solana_closed))}</span>
                                <span class="pe-method-amount-val">{usdc}</span>
                            </div>
                            <div class="pe-method-subtext">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, event.escrow_closed))}
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
                    <span class="pe-refund-text">{crate::locale::tr(|l| crate::i18n::td_string!(l, event.refund_attend))}</span>
                </div>
                // Deposit model D1 (owner, 2026-09-28): nothing is forfeited.
                // Ask for notice, threaten nothing (docs/deposit-commitment-model.md §5).
                // The promise itself lives in `utils::deposit_copy` (one home,
                // EN and TH); the catalog carries the words around it.
                {if show_thb {
                    let promise = move || never_forfeited(i18n.get_locale());
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
                    let window = move || thb_refund_window(i18n.get_locale());
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

            // Hybrid note
            {if is_hybrid {
                view! {
                    <div class="pe-hybrid-note" style="margin-top: 16px;">
                        <span>"💡"</span>
                        <span>{crate::locale::tr(|l| crate::i18n::td_string!(l, event.hybrid_deposit_note))}</span>
                    </div>
                }.into_any()
            } else {
                ().into_any()
            }}
        </div>
    }.into_any()
}
