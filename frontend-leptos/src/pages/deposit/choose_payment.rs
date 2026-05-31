//! Choose payment method view for the deposit page.
//!
//! Extracted from `mod.rs` to keep the main deposit component under 1024 lines.
//! Renders the THB (PromptPay slip upload) and USDC (wallet/QR) payment forms.

use leptos::prelude::*;

use crate::api::DepositStatusResponse;
use crate::icons::{wallet_icon_name, Icon, IconName};

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

    view! {
        // Deadline expired banner
        {if deadline_expired && !can_reclaim {
            view! {
                <div class="dep-info-note dep-choose-note-mb">
                    <div class="badge badge-warning dep-choose-badge-mb">
                        "Deadline Expired"
                    </div>
                    <p class="hint-note">
                        <Icon icon=IconName::Clock class="icon-sm" />
                        " Your deposit deadline has passed and in-person spots are now full. You have been moved to the online track."
                    </p>
                    <p class="hint-note dep-choose-hint-mt">
                        "You will be able to claim your NFT after the event ends."
                    </p>
                </div>
            }.into_any()
        } else {
            view! { <div></div> }.into_any()
        }}

        {if deadline_expired && can_reclaim {
            view! {
                <div class="dep-info-note dep-choose-note-mb">
                    <div class="badge badge-success dep-choose-badge-mb">
                        "Spot Still Available!"
                    </div>
                    <p class="hint-note">
                        <Icon icon=IconName::Clock class="icon-sm" />
                        " Your deposit deadline has passed, but in-person spots are still available! Complete your deposit now to reclaim your spot."
                    </p>
                </div>
                <p class="subtitle subtitle-lg">
                    "Choose your preferred payment method to secure your spot."
                </p>
            }.into_any()
        } else if !deadline_expired {
            view! {
                <p class="subtitle subtitle-lg">
                    "Choose your preferred payment method to secure your spot."
                </p>

                {if let Some(hours) = deposit_deadline {
                    let label = format_duration_label(hours);
                    view! {
                        <div class="dep-info-note dep-choose-note-mb">
                            <p class="hint-note">
                                <Icon icon=IconName::Clock class="icon-sm" />
                                " You have "{label}" to complete your deposit. After that, your in-person spot may be released."
                            </p>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
            }.into_any()
        } else {
            view! { <div></div> }.into_any()
        }}

        {if !deadline_expired || can_reclaim {
            let hcw = handle_connect_wallet.clone();
            let hqr = handle_pay_usdc_qr.clone();
            let hus = handle_upload_slip.clone();
            view! {
                <div class="dep-methods">

            {move || match payment_choice.get() {
                None => view! {
                    <div class="deposit-method-cards">
                        {if show_usdc {
                            view! {
                                <div class="deposit-method-card"
                                    on:click=move |_| set_payment_choice.set(Some(PaymentChoice::Usdc))>
                                    <div class="deposit-method-header">
                                        <h3 class="deposit-method-title">"Pay with USDC"</h3>
                                        <span class="badge badge-info">
                                            {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                                        </span>
                                    </div>
                                    <p class="deposit-method-desc">
                                        "Pay via Solana wallet or QR code."
                                    </p>
                                    <span class="badge badge-muted">"🧪 Dev Mode"</span>
                                </div>
                            }.into_any()
                        } else {
                            view! { <div></div> }.into_any()
                        }}
                        <div class="deposit-method-card"
                            on:click=move |_| set_payment_choice.set(Some(PaymentChoice::Thb))>
                            <div class="deposit-method-header">
                                <h3 class="deposit-method-title">"Pay with THB"</h3>
                                <span class="badge badge-warning">
                                    {format!("{} THB", data_clone.deposit_amount_thb)}
                                </span>
                            </div>
                            <p class="deposit-method-desc">
                                "Transfer via PromptPay and upload your payment slip."
                            </p>
                        </div>
                    </div>
                }.into_any(),

                Some(PaymentChoice::Usdc) => view! {
                    <button class="btn btn-outline btn-sm dep-choose-back-btn"
                        on:click=move |_| set_payment_choice.set(None)>
                        "← Change method"
                    </button>

            {if show_usdc {
                view! {
                    <div class="card">
                        <div class="card-header">
                            <h2 class="card-title">"Pay with USDC"</h2>
                            <span class="badge badge-info">
                                {format!("{:.2} USDC", data.deposit_amount_usdc as f64 / 1_000_000.0)}
                            </span>
                        </div>
                        <p class="hint-desc">
                            "Pay via Solana. Connect your wallet to send the deposit directly, or use a QR code."
                        </p>
                        <span class="badge badge-muted dep-choose-badge-mb">"🧪 Dev Mode"</span>

                        {if has_wallets() {
                            let wallets_for_click = wallets.clone();
                            view! {
                                <div class="wallet-list">
                                    <p class="wallet-prompt">
                                        <Icon icon=IconName::Link class="icon-sm" />" Connect your Solana wallet:"
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

                        <div class="dep-divider-section">
                            <p class="hint-sm">
                                <Icon icon=IconName::Phone class="icon-sm" />" No wallet? Use QR code instead:"
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
                    <button class="btn btn-outline btn-sm dep-choose-back-btn"
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

        </div>
            }.into_any()
        } else {
            view! { <div></div> }.into_any()
        }}

        // Back to event
        {
            if !event_slug.is_empty() {
                view! {
                    <a href=format!("/e/{event_slug}") class="link-back-home">
                        "← Back to event"
                    </a>
                }.into_any()
            } else {
                view! {
                    <a href="/" class="link-back-home">
                        "← Back to home"
                    </a>
                }.into_any()
            }
        }
    }
}
