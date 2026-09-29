//! Deposit Details section, escrow fields and the create-mode wallet connect.

use event_checkin_domain::money::{UsdcDepositCheck, check_usdc_deposit};
use leptos::prelude::*;

use super::ctx::FormCtx;
use super::section::{FormSection, SectionBadge};
use super::types::{format_date_display, parse_date_to_ms};
use crate::components;

/// Deposit config, escrow fields and wallet connect.
#[component]
pub(super) fn DepositSection(ctx: FormCtx) -> impl IntoView {
    let FormCtx {
        form,
        set_form,
        set_toast,
        editing_id,
        is_create,
        create_wallet_name,
        set_create_wallet_name,
        create_wallet_pk,
        set_create_wallet_pk,
        detected_wallets,
        ..
    } = ctx;
    view! {
        <FormSection icon_class="form-section-icon-deposit" title="Deposit Details" badge=SectionBadge::Optional>
                <div class="quiz-settings-grid">
                    <div class="quiz-setting-item">
                        <label class="quiz-field-label">"USDC Amount"<span class="field-optional-badge">"Required for escrow"</span></label>
                    <input
                        type="number"
                        class="quiz-number-input"
                        placeholder="e.g. 10 (whole USDC)"
                        step="0.01"
                        min="0.01"
                        prop:value=move || form.get().deposit_amount_usdc
                        on:input=move |ev| set_form.update(|f| f.deposit_amount_usdc = event_target_value(&ev))
                    />
                    {move || {
                        let current = form.get();
                        current
                            .deposit_enabled
                            .then(|| check_usdc_deposit(&current.deposit_amount_usdc).error_message())
                            .flatten()
                            .map(|message| view! { <div class="hint-warning-xs">{message}</div> })
                    }}
                    <Show
                        when=move || {
                            let current = form.get();
                            let usdc_empty = check_usdc_deposit(&current.deposit_amount_usdc) == UsdcDepositCheck::Empty;
                            let thb = current.deposit_amount_thb.parse::<u64>().unwrap_or(0);
                            current.deposit_enabled && usdc_empty && thb == 0
                        }
                        fallback=|| view! { <div></div> }
                    >
                        <div class="hint-warning-xs">
                            "At least one deposit amount (USDC or THB) is required"
                        </div>
                    </Show>
                    <span class="quiz-setting-hint">"Amount in whole USDC (e.g. 10 = 10 USDC). Max: 1,000 USDC"</span>
                </div>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"THB Amount"</label>
                    <input
                        type="number"
                        class="quiz-number-input"
                        placeholder="e.g. 500"
                        step="1"
                        min="0"
                        prop:value=move || form.get().deposit_amount_thb
                        on:input=move |ev| set_form.update(|f| f.deposit_amount_thb = event_target_value(&ev))
                    />
                    <span class="quiz-setting-hint">"Amount in Thai Baht"</span>
                </div>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"PromptPay ID"</label>
                    <input
                        type="text"
                        class="quiz-number-input"
                        placeholder="e.g. 0812345678 or 1-1001-00000-00-0"
                        prop:value=move || form.get().promptpay_id
                        on:input=move |ev| set_form.update(|f| f.promptpay_id = event_target_value(&ev))
                    />
                    <span class="quiz-setting-hint">"Thai phone number or national ID for PromptPay QR generation"</span>
                    <Show
                        when=move || {
                            let thb = form.get().deposit_amount_thb.parse::<u64>().unwrap_or(0);
                            let pp = form.get().promptpay_id.trim().to_string();
                            thb > 0 && pp.is_empty()
                        }
                        fallback=|| view! { <div></div> }
                    >
                        <div class="hint-warning-xs">
                            "PromptPay ID is required when THB amount is set"
                        </div>
                    </Show>
                </div>
                // ── On-chain escrow fields: always read-only ──
                <Show when=move || !form.get().escrow_address.is_empty() fallback=|| view! { <div></div> }>
                    <div class="quiz-setting-item">
                        <label class="quiz-field-label">
                            "Escrow Address"
                            <a
                                href=move || crate::utils::solscan_address_url(&form.get().escrow_address, &crate::utils::get_cluster())
                                target="_blank"
                                rel="noopener"
                                class="escrow-solscan-link"
                            >
                                "Solscan"
                            </a>
                        </label>
                        <div class="readonly-field">
                            <span class="readonly-value-mono">{move || form.get().escrow_address}</span>
                            <span class="readonly-badge">"Locked"</span>
                        </div>
                        <span class="quiz-setting-hint">"On-chain escrow PDA — auto-filled after on-chain init"</span>
                    </div>
                </Show>

                <Show when=move || !form.get().organizer_wallet.is_empty() fallback=|| view! { <div></div> }>
                    <div class="quiz-setting-item">
                        <label class="quiz-field-label">"Organizer Wallet"</label>
                        <div class="readonly-field">
                            <span class="readonly-value-mono">{move || form.get().organizer_wallet}</span>
                            <span class="readonly-badge">"Locked"</span>
                        </div>
                        <span class="quiz-setting-hint event-form-hint-success">"Wallet locked — set by escrow panel"</span>
                    </div>
                </Show>

                <Show when=move || !form.get().on_chain_event_id.is_empty() && form.get().on_chain_event_id != "0" fallback=|| view! { <div></div> }>
                    <div class="quiz-setting-item">
                        <label class="quiz-field-label">"On-Chain Event ID"<span class="field-optional-badge">"Auto"</span></label>
                        <div class="readonly-field">
                            <span class="readonly-value">{move || form.get().on_chain_event_id}</span>
                            <span class="readonly-badge">"Locked"</span>
                        </div>
                        <span class="quiz-setting-hint">"Numeric ID for PDA seeds — auto-derived from slug"</span>
                    </div>
                </Show>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Refund Deadline (hours)"</label>
                    <input
                        type="number"
                        class="quiz-number-input"
                        placeholder="e.g. 168 (= 7 days)"
                        min="1"
                        step="1"
                        prop:value=move || form.get().refund_deadline_hours
                        on:input=move |ev| set_form.update(|f| f.refund_deadline_hours = event_target_value(&ev))
                    />
                    <Show
                        when=move || {
                            let val = form.get().refund_deadline_hours.parse::<u32>().unwrap_or(0);
                            form.get().deposit_enabled && val == 0
                        }
                        fallback=|| view! { <div></div> }
                    >
                        <div class="hint-warning-xs">
                            "Refund deadline must be at least 1 hour"
                        </div>
                    </Show>
                    <span class="quiz-setting-hint">"Hours after event end for refund deadline (default: 168 = 7 days)"</span>
                    // Visual timeline: show computed deadline date
                    <Show
                        when=move || {
                            let end_ms = parse_date_to_ms(&form.get().event_end).unwrap_or(0);
                            let hrs = form.get().refund_deadline_hours.parse::<u32>().unwrap_or(0);
                            end_ms > 0 && hrs > 0
                        }
                        fallback=|| view! { <div></div> }
                    >
                        <div class="hint-success-sm">
                            {move || {
                                let end_ms = parse_date_to_ms(&form.get().event_end).unwrap_or(0);
                                let hrs = form.get().refund_deadline_hours.parse::<u32>().unwrap_or(0);
                                let deadline_ms = end_ms + (hrs as i64 * 3_600_000);
                                let days = hrs / 24;
                                let day_label = if days >= 7 {
                                    format!("{} days", days)
                                } else if days > 0 {
                                    format!("{}d {}h", days, hrs % 24)
                                } else {
                                    format!("{}h", hrs)
                                };
                                format!("Refund deadline: {} ({day_label} after event end)", format_date_display(deadline_ms))
                            }}
                        </div>
                    </Show>
                </div>
                <div class="quiz-setting-item">
                    <label class="quiz-field-label">"Max Refundable Deposits"</label>
                    <input
                        type="number"
                        class="quiz-number-input"
                        placeholder="e.g., 18"
                        min="0"
                        step="1"
                        prop:value=move || form.get().max_refundable_deposits
                        on:input=move |ev| set_form.update(|f| f.max_refundable_deposits = event_target_value(&ev))
                    />
                    <span class="quiz-setting-hint">"First N deposits get refund on check-in. Leave 0 or empty for unlimited. Deposits beyond this count are non-refundable."</span>
                </div>
            </div>

            // ── Create Mode: Wallet Connect for combined Create + Escrow Init ──
            <Show when=move || {
                let f = form.get();
                f.deposit_enabled && is_create
            }>
                <div class="panel-box-dashed event-form-escrow-panel">
                    <div class="panel-label">
                        "Escrow Setup"
                    </div>
                    <div class="panel-hint u-mb-sm">
                        "Connect your Solana wallet to initialize escrow when the event is created."
                    </div>

                    // Wallet not yet connected — show connect buttons
                    <Show when=move || create_wallet_pk.get().is_empty() fallback=|| view! { <div></div> }>
                        <Show when=move || !detected_wallets.get().is_empty() fallback=|| view! {
                            <div class="panel-hint">
                                "No Solana wallets detected. Install Phantom or another wallet extension."
                            </div>
                        }>
                            <div class="flex-wrap-row">
                                {move || detected_wallets.get().iter().map(|wn| {
                                    let wn_c = wn.clone();
                                    let set_wn = set_create_wallet_name;
                                    let set_wp = set_create_wallet_pk;
                                    let set_t = set_toast;
                                    view! {
                                        <button
                                            class="btn btn-outline btn-sm"
                                            on:click=move |_| {
                                                let wn = wn_c.clone();
                                                let set_wn = set_wn;
                                                let set_wp = set_wp;
                                                let set_t = set_t;
                                                leptos::task::spawn_local(async move {
                                                    match crate::pages::escrow_init::connect_wallet_js(&wn).await {
                                                        crate::wallet_error::WalletResult::Success(pk) => {
                                                            log::info!("[event-form] wallet connected: {} ({})", wn, &pk[..8.min(pk.len())]);
                                                            set_wn.set(wn);
                                                            set_wp.set(pk);
                                                        }
                                                        crate::wallet_error::WalletResult::Error(e) => {
                                                            components::show_toast(&set_t, &crate::wallet_error::user_friendly_message(&e, crate::i18n::Locale::en), components::ToastType::Error);
                                                        }
                                                        crate::wallet_error::WalletResult::UnknownFailure => {
                                                            components::show_toast(&set_t, "Wallet connection failed", components::ToastType::Error);
                                                        }
                                                    }
                                                });
                                            }
                                        >
                                            {format!("Connect {}", wn)}
                                        </button>
                                    }
                                }).collect_view()}
                            </div>
                        </Show>
                    </Show>

                    // Wallet connected — show confirmation
                    <Show when=move || !create_wallet_pk.get().is_empty() fallback=|| view! { <div></div> }>
                        <div class="wallet-connected-bar event-form-wallet-bar-no-gap">
                            <div class="wallet-info-left">
                                <div class="wallet-label">
                                    {move || format!("{} connected", create_wallet_name.get())}
                                </div>
                                <div class="wallet-address-bold">
                                    {move || create_wallet_pk.get()}
                                </div>
                            </div>
                            <button
                                class="btn btn-outline btn-xs"
                                on:click=move |_| {
                                    set_create_wallet_name.set(String::new());
                                    set_create_wallet_pk.set(String::new());
                                }
                            >
                                "Disconnect"
                            </button>
                        </div>
                        <div class="hint-success-sm event-form-hint-after-wallet">
                            "Creating event will also initialize escrow on-chain (wallet signature required)."
                        </div>
                    </Show>
                </div>
            </Show>

            // ── Escrow Management (Edit Mode Only) ──
            <Show when=move || {
                let f = form.get();
                f.deposit_enabled && !editing_id.get().unwrap_or_default().is_empty()
            }>
                <crate::pages::escrow_init::EscrowInitPanel
                    event_id=editing_id.get().unwrap_or_default()
                    form=form
                    set_form=set_form
                    set_toast=set_toast
                />
            </Show>
        </FormSection>

    }
}
