//! THB payment flow views — form, uploading spinner, and success redirect.
//!
//! Extracted from `choose_payment.rs` to keep each file focused.
//! The THB form is the main interaction surface for PromptPay slip upload.

use leptos::prelude::*;

use crate::api::DepositStatusResponse;
use crate::icons::{Icon, IconName};

use super::js_interop;
use super::types::*;

// ---------------------------------------------------------------------------
// THB payment form (ChoosePayment → Thb)
// ---------------------------------------------------------------------------

/// Renders the full THB payment form: instructions, PromptPay QR, slip upload,
/// bank account info, and submit button.
#[allow(clippy::too_many_arguments)]
pub fn thb_payment_form_view(
    data: &DepositStatusResponse,
    file_input_ref: NodeRef<leptos::html::Input>,
    slip_url_input: ReadSignal<String>,
    set_slip_url_input: WriteSignal<String>,
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
    handle_upload_slip: impl Fn() + Clone + Send + Sync + 'static,
) -> AnyView {
    let deposit_amount_thb = data.deposit_amount_thb;
    let promptpay_id = data.promptpay_id.clone();
    let pp_amount = data.deposit_amount_thb as f64;
    let pp_reference = data.event_name.clone();
    let has_promptpay = !data.promptpay_id.is_empty() && data.deposit_amount_thb > 0;

    let handle_upload_slip = handle_upload_slip.clone();

    view! {
        <div class="card">
            // Card header
            <div class="card-header">
                <h2 class="card-title">"Pay with THB"</h2>
                <span class="badge badge-warning">
                    {format!("{deposit_amount_thb} THB")}
                </span>
            </div>

            // How-to-pay instructions
            <div class="thb-how-to-pay">
                <p class="thb-how-to-pay-title">"How to pay:"</p>
                <ol>
                    <li>"Scan the QR code below with your banking app"</li>
                    <li>"Transfer "{format!("{deposit_amount_thb} THB")}" via PromptPay"</li>
                    <li>"Take a screenshot of the payment confirmation"</li>
                    <li>"Upload the screenshot below and submit"</li>
                </ol>
            </div>

            <p class="hint-desc">"Transfer via PromptPay and upload your payment slip."</p>

            // PromptPay QR code
            {if has_promptpay {
                view! {
                    <div class="layout-col-center u-mb-1rem">
                        <p class="text-amount">
                            {format!("Scan to pay {deposit_amount_thb} THB")}
                        </p>
                        {move || {
                            let pp_qr_string = js_interop::generate_promptpay_qr(&promptpay_id, pp_amount, &pp_reference);
                            let pp_qr_image = pp_qr_string.and_then(|s| js_interop::generate_qr_data_url(&s, 256));
                            match pp_qr_image {
                                Some(url) => {
                                    let url_for_save = url.clone();
                                    view! {
                                        <div class="qr-wrapper">
                                            <img src=url alt="PromptPay QR" class="qr-img-md" />
                                        </div>
                                        <button
                                            class="btn btn-outline btn-sm u-mt-xs"
                                            on:click=move |_| {
                                                js_interop::download_data_url(
                                                    &url_for_save,
                                                    &format!("promptpay-{deposit_amount_thb}THB-qr.png"),
                                                );
                                            }
                                        >
                                            <Icon icon=IconName::Save class="icon-sm" />
                                            " Save QR Code"
                                        </button>
                                    }.into_any()
                                },
                                None => view! {
                                    <p class="hint-2xs">"QR generation failed — please pay manually."</p>
                                }.into_any(),
                            }
                        }}
                        <p class="qr-hint-text">"Open your banking app → Scan QR → Pay"</p>
                    </div>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}

            // Slip upload section
            <div class="dep-divider-section">
                <label class="upload-label">
                    <Icon icon=IconName::Clip class="icon-sm" />" Upload payment slip"
                </label>
                <p class="thb-slip-hint">
                    "Take a screenshot or photo of your transfer confirmation. Max 3MB (JPEG, PNG, WebP)."
                </p>
                <input
                    type="file"
                    accept="image/jpeg,image/png,image/webp"
                    node_ref=file_input_ref
                    class="file-input-styled"
                    on:change=move |_| {
                        let file_ref = file_input_ref.clone();
                        leptos::task::spawn_local(async move {
                            if let Some(el) = file_ref.get() {
                                let js_val: wasm_bindgen::JsValue = el.into();
                                let preview = js_interop::read_file_as_data_url(&js_val).await;
                                set_slip_preview.set(preview);
                            }
                        });
                    }
                />

                // Slip image preview
                {move || match slip_preview.get() {
                    Some(url) => view! {
                        <div class="slip-preview-container">
                            <img src=&url class="slip-preview-img" />
                            <button
                                class="slip-preview-remove"
                                on:click=move |_| {
                                    set_slip_preview.set(None);
                                    if let Some(el) = file_input_ref.get() {
                                        el.set_value("");
                                    }
                                }
                            >
                                "✕"
                            </button>
                        </div>
                    }.into_any(),
                    None => view! { <div></div> }.into_any(),
                }}

                // Manual URL fallback
                <details class="u-mt-xs">
                    <summary class="details-summary-text">"Or paste slip URL manually"</summary>
                    <input
                        type="text"
                        class="form-input dep-input u-mt-xs"
                        placeholder="Paste slip image URL"
                        prop:value=move || slip_url_input.get()
                        on:input=move |ev| {
                            let val = event_target_value(&ev);
                            set_slip_url_input.set(val);
                        }
                    />
                </details>

                // Bank account for refund
                <div class="thb-bank-section">
                    <p class="thb-bank-label">"Bank account for refund"</p>
                    <input
                        type="text"
                        class="form-input dep-input"
                        placeholder="Bank account number"
                        prop:value=move || bank_account_input.get()
                        on:input=move |ev| {
                            let val = event_target_value(&ev);
                            set_bank_account_input.set(val);
                        }
                    />
                    // Bank name with autocomplete dropdown
                    <div class="bank-dropdown u-mt-xs">
                        <input
                            type="text"
                            class="form-input dep-input"
                            placeholder="Bank name (e.g. KBank, SCB)"
                            prop:value=move || bank_name_input.get()
                            on:focus=move |_| set_show_bank_dropdown.set(true)
                            on:input=move |ev| {
                                let val = event_target_value(&ev);
                                set_bank_name_input.set(val);
                                set_show_bank_dropdown.set(true);
                            }
                            on:blur=move |_| {
                                set_timeout(
                                    move || set_show_bank_dropdown.set(false),
                                    std::time::Duration::from_millis(200),
                                );
                            }
                        />
                        {move || {
                            if !show_bank_dropdown.get() {
                                return view! { <div></div> }.into_any();
                            }
                            let query = bank_name_input.get().to_lowercase();
                            let matches: Vec<&(&str, &str)> = THAI_BANKS
                                .iter()
                                .filter(|(code, name)| {
                                    if query.is_empty() { return true; }
                                    code.to_lowercase().contains(&query)
                                        || name.to_lowercase().contains(&query)
                                })
                                .collect();
                            if matches.is_empty() {
                                return view! { <div></div> }.into_any();
                            }
                            let items: Vec<_> = matches.into_iter().map(|bank| {
                                let bank_val = bank.1.to_string();
                                view! {
                                    <div
                                        class="bank-dropdown-item"
                                        on:mousedown=move |ev| {
                                            ev.prevent_default();
                                            set_bank_name_input.set(bank_val.clone());
                                            set_show_bank_dropdown.set(false);
                                        }
                                    >
                                        <span class="bank-dropdown-name">{bank_val.clone()}</span>
                                    </div>
                                }
                            }).collect();
                            view! {
                                <div class="bank-dropdown-list">
                                    {items}
                                </div>
                            }.into_any()
                        }}
                    </div>
                    <input
                        type="text"
                        class="form-input dep-input u-mt-xs"
                        placeholder="Account holder name"
                        prop:value=move || account_name_input.get()
                        on:input=move |ev| {
                            let val = event_target_value(&ev);
                            set_account_name_input.set(val);
                        }
                    />
                </div>

                // Submit button
                <button
                    class="btn btn-success btn-block btn-action-lg u-mt-1rem"
                    disabled=move || {
                        bank_account_input.get().trim().is_empty()
                        || bank_name_input.get().trim().is_empty()
                        || account_name_input.get().trim().is_empty()
                    }
                    on:click={
                        let hus = handle_upload_slip.clone();
                        move |_| hus()
                    }
                >
                    "Upload Slip"
                </button>
                <p class="thb-upload-disclaimer">
                    "Bank account, bank name, and account holder name are required for refund."
                </p>
            </div>
        </div>
    }
        .into_any()
}

// ---------------------------------------------------------------------------
// Uploading spinner
// ---------------------------------------------------------------------------

/// THB uploading spinner view.
pub fn thb_uploading_view() -> AnyView {
    view! {
        <div class="card dep-card">
            <div class="card-header">
                <h2 class="card-title">
                    <span class="spinner spinner-lg"></span>
                    " Uploading slip..."
                </h2>
            </div>
            <p class="hint-desc">"Please wait while we upload your payment slip."</p>
        </div>
    }
        .into_any()
}

// ---------------------------------------------------------------------------
// Upload success + auto-redirect
// ---------------------------------------------------------------------------

/// THB uploaded successfully view — auto-redirects to ticket page.
pub fn thb_uploaded_view(
    attendee_id: &str,
    event_id: &str,
) -> AnyView {
    let aid = attendee_id.to_string();
    let eid = event_id.to_string();
    leptos::task::spawn_local(async move {
        gloo_timers::future::TimeoutFuture::new(1500).await;
        js_interop::navigate_to(&format!("/ticket/{aid}?event_id={eid}"));
    });
    view! {
        <div class="card thb-success-card">
            <div class="card-header">
                <h2 class="card-title">
                    <Icon icon=IconName::Check class="icon-sm icon-success" />
                    " Slip Uploaded"
                </h2>
            </div>
            <p class="hint-desc">
                "Your payment slip has been submitted for verification. You'll be notified once it's confirmed."
            </p>
            <span class="badge badge-warning">
                <Icon icon=IconName::Hourglass class="icon-sm icon-warning" />
                " Pending Verification"
            </span>
            <p class="thb-success-redirect">"Redirecting to your ticket..."</p>
        </div>
    }
        .into_any()
}

// ---------------------------------------------------------------------------
// THB slip rejected — re-upload prompt
// ---------------------------------------------------------------------------

/// THB slip was rejected by admin. Shows rejection notice with re-upload CTA.
pub fn thb_rejected_view(
    data: &DepositStatusResponse,
    set_state: WriteSignal<DepositPageState>,
    set_payment_choice: WriteSignal<Option<PaymentChoice>>,
) -> AnyView {
    let amount_thb = data.deposit_amount_thb;
    let data_clone = data.clone();

    view! {
        <div class="card dep-card-error">
            <div class="card-header">
                <h2 class="card-title">
                    <Icon icon=IconName::Warning class="icon-sm icon-danger" />
                    " Slip Rejected"
                </h2>
                <span class="badge badge-danger">
                    {format!("{amount_thb} THB")}
                </span>
            </div>
            <p class="hint-desc">
                "Your payment slip was reviewed and could not be verified. This can happen if the slip is unreadable, the amount doesn't match, or the transfer wasn't completed."
            </p>
            <div class="dep-info-note">
                <p class="hint-note">
                    "Please make a new transfer and upload the correct payment slip."
                </p>
            </div>
            <button
                class="btn btn-primary btn-block btn-action-lg"
                on:click=move |_| {
                    set_payment_choice.set(Some(PaymentChoice::Thb));
                    set_state.set(DepositPageState::ChoosePayment(data_clone.clone()));
                }
            >
                "Re-upload Payment Slip"
            </button>
        </div>
    }
        .into_any()
}
