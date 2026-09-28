//! THB payment flow views — form, uploading spinner, and success redirect.
//!
//! Extracted from `choose_payment.rs` to keep each file focused.
//! The THB form is the main interaction surface for PromptPay slip upload.

use leptos::prelude::*;

use super::js_interop;
use super::types::*;
use crate::api::DepositStatusResponse;
use crate::components::LightboxImage;
use crate::i18n::{t, t_string, use_i18n};

// ---------------------------------------------------------------------------
// THB payment form (ChoosePayment → Thb)
// ---------------------------------------------------------------------------

/// Where a slip without a readable QR goes. A product name, so it is not
/// translated; the sentence around it lives in `deposit.thb.vision_privacy`.
const SLIP_VISION_VENDOR: &str = "Anthropic's Claude API";

/// Shown above the file picker only while the worker's vision fallback is on
/// (`.plans/033` §4 Q3): the attendee learns before submitting that a slip
/// without a readable QR leaves our storage for the Claude API. A render
/// function rather than a string so the line follows the language switch.
const SLIP_VISION_PRIVACY_LINE: fn() -> AnyView = slip_vision_privacy_line;

fn slip_vision_privacy_line() -> AnyView {
    let i18n = use_i18n();
    view! { {t!(i18n, deposit.thb.vision_privacy, vendor = SLIP_VISION_VENDOR)} }.into_any()
}

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
    let i18n = use_i18n();
    let deposit_amount_thb = data.deposit_amount_thb;
    let promptpay_id = data.promptpay_id.clone();
    let pp_amount = data.deposit_amount_thb as f64;
    let pp_reference = data.event_name.clone();
    let has_promptpay = !data.promptpay_id.is_empty() && data.deposit_amount_thb > 0;
    let slip_vision_enabled = data.slip_vision_enabled;

    log::trace!(
        "[thb_payment] promptpay_id='{}' amount={} has_promptpay={}",
        promptpay_id,
        deposit_amount_thb,
        has_promptpay
    );

    let handle_upload_slip = handle_upload_slip.clone();

    // Eagerly compute QR — no signals involved, so no need for reactive closure.
    let pp_qr_image = if has_promptpay {
        js_interop::generate_promptpay_qr(&promptpay_id, pp_amount, &pp_reference)
            .and_then(|s| js_interop::generate_qr_data_url(&s, 256))
    } else {
        None
    };
    log::trace!(
        "[thb_payment] pp_qr_image generated: {}",
        pp_qr_image.is_some()
    );

    view! {
        <div class="dep2-card">
            // Card header
            <div class="dep2-card-header">
                <h2 class="dep2-card-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.title))}</h2>
                <span class="badge badge-warning">
                    {format!("฿{deposit_amount_thb}")}
                </span>
            </div>

            // ── Section A: Scan QR ──────────────────────────────────────
            {if has_promptpay {
                view! {
                    <div class="dep2-section">
                        <div class="dep2-section-title">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.scan_pay))}
                        </div>

                        {match pp_qr_image {
                            Some(url) => {
                                let url_for_save = url.clone();
                                view! {
                                    <div class="layout-col-center">
                                        <div class="qr-wrapper">
                                            <img src=url alt="PromptPay QR" class="qr-img-md" />
                                        </div>
                                        <button
                                            class="btn btn-primary btn-sm u-mt-xs dep2-save-qr-btn"
                                            on:click=move |_| {
                                                js_interop::download_data_url(
                                                    &url_for_save,
                                                    &format!("promptpay-{deposit_amount_thb}-qr.png")
                                                );
                                            }
                                        >
                                            {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.save_qr))}
                                        </button>
                                    </div>
                                }.into_any()
                            },
                            None => view! {
                                <p class="hint-2xs">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.qr_failed))}</p>
                            }.into_any(),
                        }}

                        <p class="hint-desc u-mt-xs">
                            {t!(i18n, deposit.thb.scan_hint, amount = deposit_amount_thb)}
                        </p>
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="dep2-section">
                        <div class="dep2-section-title">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.scan_pay))}
                        </div>
                        <p class="hint-desc">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.no_promptpay))}</p>
                    </div>
                }.into_any()
            }}

            // ── Section B: Upload Slip ──────────────────────────────────
            <div class="dep2-section">
                <div class="dep2-section-title">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.upload_title))}
                </div>
                <p class="thb-slip-hint">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.upload_hint))}
                </p>
                {slip_vision_enabled.then(|| view! {
                    <p class="thb-slip-hint">
                        {SLIP_VISION_PRIVACY_LINE}
                    </p>
                })}
                <input
                    type="file"
                    accept="image/jpeg,image/png,image/webp"
                    node_ref=file_input_ref
                    class="file-input-styled"
                    on:change=move |_| {
                        let file_ref = file_input_ref;
                        leptos::task::spawn_local(async move {
                            if let Some(el) = file_ref.get() {
                                let js_val: wasm_bindgen::JsValue = el.into();
                                let preview = js_interop::read_file_as_data_url(&js_val).await;
                                set_slip_preview.set(preview);
                            }
                        });
                    }
                />

                // Slip image preview — click to view fullscreen so the user
                // can verify the slip is legible before submitting.
                {move || match slip_preview.get() {
                    Some(url) => view! {
                        <div class="slip-preview-container">
                            <LightboxImage
                                src=url
                                alt=t_string!(i18n, deposit.thb.preview_alt)
                                thumb_class="slip-preview-img"
                                hint=t_string!(i18n, deposit.thb.preview_close_hint).to_string()
                            />
                            <span class="slip-preview-zoom-hint">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.tap_enlarge))}</span>
                            <button
                                class="slip-preview-remove"
                                on:click=move |_| {
                                    set_slip_preview.set(None);
                                    if let Some(el) = file_input_ref.get() {
                                        el.set_value("");
                                    }
                                }
                            >
                                "\u{2715}"
                            </button>
                        </div>
                    }.into_any(),
                    None => view! { <div></div> }.into_any(),
                }}

                // Manual URL fallback — advanced option
                <details class="u-mt-xs dep2-advanced-toggle">
                    <summary class="details-summary-text hint-muted">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.advanced))}</summary>
                    <input
                        type="text"
                        class="form-input dep-input u-mt-xs"
                        placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.url_placeholder))
                        prop:value=move || slip_url_input.get()
                        on:input=move |ev| {
                            let val = event_target_value(&ev);
                            set_slip_url_input.set(val);
                        }
                    />
                </details>
            </div>

            // ── Section C: Refund Account ───────────────────────────────
            <div class="dep2-section">
                <div class="dep2-section-title">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.refund_title))}
                </div>
                <p class="hint-desc">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.refund_question))}</p>

                <input
                    type="text"
                    class="form-input dep-input u-mt-xs"
                    placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.bank_account))
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
                        placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.bank_name))
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
                    placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.account_name))
                    prop:value=move || account_name_input.get()
                    on:input=move |ev| {
                        let val = event_target_value(&ev);
                        set_account_name_input.set(val);
                    }
                />
            </div>

            // ── Submit ──────────────────────────────────────────────────
            <button
                class="btn btn-success btn-block u-mt-1rem"
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
                {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.submit))}
            </button>
            <p class="thb-upload-disclaimer">
                {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.required))}
            </p>
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
        <div class="dep2-card">
            <div class="dep2-confirming">
                <div class="dep2-confirming-dots">
                    <span class="dep2-confirming-dot"></span>
                    <span class="dep2-confirming-dot"></span>
                    <span class="dep2-confirming-dot"></span>
                </div>
                <p class="hint-desc u-mt-xs">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.uploading))}</p>
            </div>
        </div>
    }
    .into_any()
}

// ---------------------------------------------------------------------------
// Upload success + auto-redirect
// ---------------------------------------------------------------------------

/// THB uploaded successfully view — auto-redirects to ticket page.
pub fn thb_uploaded_view(attendee_id: &str, event_id: &str) -> AnyView {
    let i18n = use_i18n();
    let aid = attendee_id.to_string();
    let eid = event_id.to_string();
    leptos::task::spawn_local(async move {
        gloo_timers::future::TimeoutFuture::new(1500).await;
        js_interop::navigate_to(&format!("/ticket/{aid}?event_id={eid}"));
    });
    view! {
        <div class="dep2-card">
            <div class="dep2-success-icon">"✓"</div>
            <h2 class="dep2-card-title" style="text-align:center;margin-top:0.75rem">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.submitted_title))}</h2>
            <p class="hint-desc" style="text-align:center">
                {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.submitted_body))}
            </p>
            <div style="text-align:center">
                {move || view! {
                    <crate::components::StatusBadge
                        tone=crate::components::StatusTone::Pending
                        label=t_string!(i18n, deposit.thb.pending)
                    />
                }}
            </div>
            <p class="thb-success-redirect">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.redirecting))}</p>
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
    let _amount_thb = data.deposit_amount_thb;
    let data_clone = data.clone();

    view! {
        <div class="dep2-card">
            <div class="dep2-deadline dep2-deadline--danger">
                <p class="dep2-deadline-text">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.rejected))}
                </p>
            </div>

            <button
                class="btn btn-primary btn-block u-mt-1rem"
                on:click=move |_| {
                    set_payment_choice.set(Some(PaymentChoice::Thb));
                    set_state.set(DepositPageState::ChoosePayment(data_clone.clone()));
                }
            >
                {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.reupload))}
            </button>
        </div>
    }
    .into_any()
}

// ---------------------------------------------------------------------------
// THB upload blocked — session expired, sign-in required
// ---------------------------------------------------------------------------

/// THB upload was rejected with 401 because the attendee's JWT is missing or
/// expired. The deposit page is public (loads deposit status without auth),
/// but `/api/deposit/thb/upload` is gated by `require_identity`. Rather than
/// leaving the user stuck with a generic "Failed to upload slip" toast and no
/// path forward, render a clear "session expired" notice with a "Sign In"
/// CTA that routes to `/login`.
///
/// After successful sign-in, the user returns to the deposit page (via the
/// URL they came from) and can retry the upload.
pub fn thb_auth_required_view(data: &DepositStatusResponse) -> AnyView {
    let amount_thb = data.deposit_amount_thb;

    // Capture the current path + query string so we can return the user here
    // after the OAuth roundtrip. Eager computation — no signals involved, so
    // no need for a reactive closure. Falls back to `/` if the browser
    // context is unavailable (e.g. SSR — not currently used but defensive).
    let current_path = web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_default();
    let current_search = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default();
    let return_to = format!("{current_path}{current_search}");
    let login_href = if return_to.is_empty() || return_to == "/" {
        "/login".to_string()
    } else {
        format!("/login?next={}", urlencoding::encode(&return_to))
    };

    view! {
        <div class="dep2-card">
            <div class="dep2-card-header">
                <h2 class="dep2-card-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.auth_title))}</h2>
                {if amount_thb > 0 {
                    view! {
                        <span class="badge badge-warning">
                            {format!("฿{amount_thb}")}
                        </span>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
            </div>

            <div class="dep2-section">
                <div class="dep2-deadline dep2-deadline--danger">
                    <p class="dep2-deadline-text">
                        {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.auth_expired))}
                    </p>
                </div>
                <p class="hint-desc u-mt-xs">
                    {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.auth_explain))}
                </p>
            </div>

            <a
                class="btn btn-primary btn-block u-mt-1rem"
                href=login_href
            >
                {crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.auth_cta))}
            </a>
        </div>
    }
    .into_any()
}
