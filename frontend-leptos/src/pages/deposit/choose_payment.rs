//! Choose payment method view for the deposit page.
//!
//! Extracted from `mod.rs` to keep the main deposit component under 1024 lines.
//! Renders the THB (PromptPay slip upload) and USDC (wallet/QR) payment forms.

use leptos::prelude::*;

use crate::api::DepositStatusResponse;
use crate::icons::{wallet_icon_name, Icon};

use super::types::*;

// Note: ReadSignal + WriteSignal come from leptos::prelude::* above.

/// Renders the "Choose Payment" view — the first step of the deposit wizard.
///
/// Shows deadline banners, payment method cards, and the selected method's form
/// (USDC wallet connect + QR, or THB PromptPay slip upload).
#[allow(clippy::too_many_arguments)]
pub fn choose_payment_view(
    data: DepositStatusResponse,
    detected_wallets: ReadSignal<Vec<String>>,
    has_wallets: impl Fn() -> bool + Clone + Send + Sync + 'static,
    payment_choice: ReadSignal<Option<PaymentChoice>>,
    set_payment_choice: WriteSignal<Option<PaymentChoice>>,
    wallet_input: ReadSignal<String>,
    set_wallet_input: WriteSignal<String>,
    slip_url_input: ReadSignal<String>,
    set_slip_url_input: WriteSignal<String>,
    file_input_ref: NodeRef<leptos::html::Input>,
    slip_preview: ReadSignal<Option<String>>,
    set_slip_preview: WriteSignal<Option<String>>,
    bank_account_input: ReadSignal<String>,
    set_bank_account_input: WriteSignal<String>,
    bank_name_input: ReadSignal<String>,
    set_bank_name_input: WriteSignal<String>,
    account_name_input: ReadSignal<String>,
    set_account_name_input: WriteSignal<String>,
    show_bank_dropdown: ReadSignal<bool>,
    set_show_bank_dropdown: WriteSignal<bool>,
    handle_connect_wallet: impl Fn(String) + Clone + Send + Sync + 'static,
    handle_pay_usdc_qr: impl Fn() + Clone + Send + Sync + 'static,
    handle_upload_slip: impl Fn() + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let data_clone = data.clone();
    let event_slug = data_clone.event_slug.clone();
    let wallets = detected_wallets.get();
    let usdc_accepted = data.usdc_deposits_accepted;
    let show_usdc = usdc_accepted;
    let deposit_deadline = data_clone.deposit_deadline_hours;
    let deadline_expired = data_clone.deadline_expired;
    let can_reclaim = data_clone.in_person_available.unwrap_or(false);
    let usdc_formatted = format_usdc(data.deposit_amount_usdc);
    let thb_amount = data_clone.deposit_amount_thb;

    // Reactive countdown for the deposit deadline banner.
    // Computes deadline from registration_date + deposit_deadline_hours, then ticks every second.
    let deadline_ms = compute_deadline_ms(&data_clone.registration_date, deposit_deadline);
    let (countdown_text, set_countdown_text) = signal(String::new());
    let (countdown_expired, set_countdown_expired) = signal(false);
    if let Some(dl_ms) = deadline_ms {
        let now_ms = js_sys::Date::now();
        let remaining_ms = dl_ms - now_ms;
        if remaining_ms <= 0.0 {
            set_countdown_expired.set(true);
        } else {
            let remaining_secs = (remaining_ms / 1000.0) as i64;
            set_countdown_text.set(format_countdown(remaining_secs));
            if let Ok(handle) = set_interval_with_handle(
                move || {
                    let now = js_sys::Date::now();
                    let remaining = dl_ms - now;
                    if remaining <= 0.0 {
                        set_countdown_text.set(String::new());
                        set_countdown_expired.set(true);
                    } else {
                        set_countdown_text.set(format_countdown((remaining / 1000.0) as i64));
                    }
                },
                std::time::Duration::from_secs(1),
            ) {
                on_cleanup(move || handle.clear());
            }
        }
    }

    view! {
        // Amount hero
        {if !deadline_expired || can_reclaim {
            view! {
                <div class="dep2-amount-hero">
                    {if show_usdc && thb_amount > 0 {
                        format!("฿{} / {} USDC", thb_amount, usdc_formatted)
                    } else if show_usdc {
                        format!("{} USDC", usdc_formatted)
                    } else {
                        format!("฿{}", thb_amount)
                    }}
                </div>
                <div class="dep2-amount-unit">"Secure your spot with a deposit"</div>
            }.into_any()
        } else {
            view! { <div></div> }.into_any()
        }}

        // Deadline banners
        {if deadline_expired && !can_reclaim {
            view! {
                <div class="dep2-deadline dep2-deadline--danger">
                    <span class="dep2-deadline-text">
                        "Your deposit deadline has passed and in-person spots are now full. You have been moved to the online track. You will be able to claim your NFT after the event ends."
                    </span>
                </div>
            }.into_any()
        } else if deadline_expired && can_reclaim {
            view! {
                <div class="dep2-deadline dep2-deadline--success">
                    <span class="dep2-deadline-text">
                        "Your deadline has passed, but in-person spots are still available! Complete your deposit now to reclaim your spot."
                    </span>
                </div>
            }.into_any()
        } else if let Some(_hours) = deposit_deadline {
            view! {
                <div class="dep2-deadline dep2-deadline--warning">
                    <span class="dep2-deadline-text">
                        {move || {
                            let ct = countdown_text.get();
                            let expired = countdown_expired.get();
                            if expired {
                                view! {
                                    "Your deposit deadline has passed. After that, your in-person spot may be released."
                                }.into_any()
                            } else if ct.is_empty() {
                                // Fallback when no registration_date available
                                let label = format_duration_label(_hours);
                                view! {
                                    "You have "{format!("{label}")}" to complete your deposit. After that, your in-person spot may be released."
                                }.into_any()
                            } else {
                                view! {
                                    "You have "
                                    <span class="dep2-countdown-timer">{ct}</span>
                                    " to complete your deposit. After that, your in-person spot may be released."
                                }.into_any()
                            }
                        }}
                    </span>
                </div>
            }.into_any()
        } else {
            view! { <div></div> }.into_any()
        }}

        {if !deadline_expired || can_reclaim {
            let hcw = handle_connect_wallet.clone();
            let hqr = handle_pay_usdc_qr.clone();
            let hus = handle_upload_slip.clone();
            view! {
                {move || match payment_choice.get() {
                    None => view! {
                        <div class="dep2-method-grid"
                            class:dep2-method-grid--single={!show_usdc}>
                            // THB card — always shown, recommended
                            <div class="dep2-method-card dep2-method-card--recommended"
                                on:click=move |_| set_payment_choice.set(Some(PaymentChoice::Thb))>
                                <div class="dep2-method-name">"THB"</div>
                                <div class="dep2-method-amount">
                                    {format!("฿{} THB", thb_amount)}
                                </div>
                                <div class="dep2-method-label">"via PromptPay"</div>
                                <button class="dep2-method-cta"
                                    on:click=move |ev| {
                                        ev.stop_propagation();
                                        set_payment_choice.set(Some(PaymentChoice::Thb));
                                    }>
                                    "Pay with PromptPay →"
                                </button>
                            </div>

                            // USDC card — only if accepted
                            {if show_usdc {
                                view! {
                                    <div class="dep2-method-card"
                                        on:click=move |_| set_payment_choice.set(Some(PaymentChoice::Usdc))>
                                        <div class="dep2-method-name">"USDC"</div>
                                        <div class="dep2-method-amount">
                                            {format!("{} USDC", usdc_formatted)}
                                        </div>
                                        <div class="dep2-method-label">"via Solana"</div>
                                    </div>
                                }.into_any()
                            } else {
                                view! { <div></div> }.into_any()
                            }}
                        </div>
                    }.into_any(),

                    Some(PaymentChoice::Usdc) => view! {
                        <button class="dep2-back"
                            on:click=move |_| set_payment_choice.set(None)>
                            "← Change method"
                        </button>

                        {if show_usdc {
                            view! {
                                <div class="dep2-card">
                                    <div class="dep2-amount-hero">
                                        {format!("{} USDC", usdc_formatted)}
                                    </div>

                                    {if has_wallets() {
                                        let wallets_for_click = wallets.clone();
                                        view! {
                                            <div class="wallet-list">
                                                <p class="wallet-prompt">
                                                            "Connect your Solana wallet:"
                                                        </p>
                                                {wallets_for_click.into_iter().map(|w| {
                                                    let w_clone = w.clone();
                                                    let wallet_icon = wallet_icon_name(&w);
                                                    let hcw = hcw.clone();
                                                    view! {
                                                        <button
                                                            class="btn btn-primary btn-block wallet-btn-inner"
                                                            on:click={
                                                                let w = w.clone();
                                                                move |_| hcw(w.clone())
                                                            }
                                                        >
                                                            <Icon icon=wallet_icon class="icon-md wallet-icon-white" />
                                                            <span>{format!("Connect {}", &w_clone)}</span>
                                                        </button>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }}

                                    <div class="dep2-qr-secondary">
                                        <p class="dep2-qr-secondary-label">
                                            "No wallet? Use QR code instead:"
                                        </p>
                                        <div class="u-mb-sm">
                                            <input
                                                type="text"
                                                class="form-input dep-input"
                                                placeholder="Enter your Solana wallet address"
                                                prop:value=move || wallet_input.get()
                                                on:input=move |ev| {
                                                    let val = event_target_value(&ev);
                                                    set_wallet_input.set(val);
                                                }
                                            />
                                        </div>
                                        <button
                                            class="btn btn-outline btn-block"
                                            on:click={
                                                let hqr = hqr.clone();
                                                move |_| hqr()
                                            }
                                        >
                                            "Generate QR Code"
                                        </button>
                                    </div>
                                </div>
                            }.into_any()
                        } else {
                            view! { <div></div> }.into_any()
                        }}
                    }.into_any(),

                    Some(PaymentChoice::Thb) => view! {
                        <button class="dep2-back"
                            on:click=move |_| set_payment_choice.set(None)>
                            "← Change method"
                        </button>
                        {super::thb_payment::thb_payment_form_view(
                            &data_clone,
                            file_input_ref.clone(),
                            slip_url_input,
                            set_slip_url_input,
                            slip_preview,
                            set_slip_preview,
                            bank_account_input,
                            set_bank_account_input,
                            bank_name_input,
                            set_bank_name_input,
                            account_name_input,
                            set_account_name_input,
                            show_bank_dropdown,
                            set_show_bank_dropdown,
                            hus.clone(),
                        )}
                    }.into_any(),
                }}
            }.into_any()
        } else {
            view! { <div></div> }.into_any()
        }}

        // Back to event
        {
            if !event_slug.is_empty() {
                view! {
                    <a href=format!("/e/{event_slug}") class="dep2-back">
                        "← Back to event"
                    </a>
                }.into_any()
            } else {
                view! {
                    <a href="/" class="dep2-back">
                        "← Back to home"
                    </a>
                }.into_any()
            }
        }
    }
}
