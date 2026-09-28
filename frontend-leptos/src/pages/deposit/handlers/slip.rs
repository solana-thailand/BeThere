//! THB slip upload and pay-URL copy.

use leptos::prelude::*;

use crate::api::{self, ThbSlipUploadRequest};
use crate::components::{self as app_components, ToastType};
use crate::i18n::{td_string, use_i18n};

use crate::pages::deposit::js_interop;
use crate::pages::deposit::types::*;

// ---------------------------------------------------------------------------
// Upload THB slip
// ---------------------------------------------------------------------------

/// Create handler: upload THB PromptPay slip with bank account info.
#[allow(clippy::too_many_arguments)]
pub fn make_upload_slip(
    state: ReadSignal<DepositPageState>,
    set_state: WriteSignal<DepositPageState>,
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
    slip_url_input: ReadSignal<String>,
    bank_account_input: ReadSignal<String>,
    bank_name_input: ReadSignal<String>,
    account_name_input: ReadSignal<String>,
    file_input_ref: NodeRef<leptos::html::Input>,
    params: DepositParamsSignal,
) -> impl Fn() + Clone + Send + Sync + 'static {
    let i18n = use_i18n();
    move || {
        let locale = i18n.get_locale_untracked();
        let current_state = state.get();
        let (deposit_data, attendee_id, event_id) = match &current_state {
            DepositPageState::ChoosePayment(d) => {
                let aid = match params.get() {
                    Ok(p) => p.attendee_id.unwrap_or_default(),
                    Err(_) => return,
                };
                let eid = extract_event_id_from_url();
                (d.clone(), aid, eid)
            }
            _ => return,
        };

        let deposit_data_for_err = deposit_data.clone();
        let deposit_data_slug = deposit_data.event_slug.clone();
        set_state.set(DepositPageState::ThbUploading(deposit_data));

        let file_ref = file_input_ref;
        let text_slip_url = slip_url_input.get();
        let bank_account_input_for_upload = bank_account_input.get();
        let bank_name_input_for_upload = bank_name_input.get();
        let account_name_input_for_upload = account_name_input.get();
        let params = params;

        leptos::task::spawn_local(async move {
            let slip_url = match file_ref.get() {
                Some(el) => {
                    let js_val: wasm_bindgen::JsValue = el.into();
                    match js_interop::read_file_as_data_url(&js_val).await {
                        Some(data_url) => data_url,
                        None => {
                            if text_slip_url.trim().is_empty() {
                                set_state
                                    .set(DepositPageState::ChoosePayment(deposit_data_for_err));
                                app_components::show_toast(
                                    &set_toast,
                                    td_string!(locale, deposit.toast.select_slip),
                                    ToastType::Warning,
                                );
                                return;
                            }
                            text_slip_url
                        }
                    }
                }
                None => {
                    if text_slip_url.trim().is_empty() {
                        set_state.set(DepositPageState::ChoosePayment(deposit_data_for_err));
                        app_components::show_toast(
                            &set_toast,
                            td_string!(locale, deposit.toast.select_slip),
                            ToastType::Warning,
                        );
                        return;
                    }
                    text_slip_url
                }
            };

            // Only an uploaded image can carry a QR we can read; a pasted URL
            // is someone else's host and is not fetched.
            let slip_qr = match slip_url.starts_with("data:image/") {
                true => js_interop::decode_slip_qr(&slip_url).await,
                false => None,
            };

            let body = ThbSlipUploadRequest {
                event_id: event_id.unwrap_or_default(),
                attendee_id,
                slip_url,
                bank_account: {
                    let v = bank_account_input_for_upload.trim().to_string();
                    if v.is_empty() { None } else { Some(v) }
                },
                bank_name: {
                    let v = bank_name_input_for_upload.trim().to_string();
                    if v.is_empty() { None } else { Some(v) }
                },
                account_name: {
                    let v = account_name_input_for_upload.trim().to_string();
                    if v.is_empty() { None } else { Some(v) }
                },
                slip_qr,
            };
            match api::upload_thb_slip(&body).await {
                Ok(_resp) => {
                    log::info!("[deposit] THB slip uploaded successfully");
                    let aid = match params.get() {
                        Ok(p) => p.attendee_id.unwrap_or_default(),
                        Err(_) => String::new(),
                    };
                    let eid = extract_event_id_from_url().unwrap_or_default();
                    set_state.set(DepositPageState::ThbUploaded(aid, eid, deposit_data_slug));
                }
                Err(e) => {
                    log::error!("[deposit] THB slip upload failed: {e}");
                    // 401 = JWT missing/expired. The deposit page itself is
                    // public (loads deposit status without auth), but
                    // `/api/deposit/thb/upload` is gated by `require_identity`.
                    // Without this branch, the user sees a generic "Failed to
                    // upload slip" toast and has no path forward — able to
                    // view but unable to act. Route to `ThbAuthRequired`,
                    // which renders a clear "session expired, sign in" CTA
                    // and unblocks the user.
                    if e.status == 401 {
                        app_components::show_toast(
                            &set_toast,
                            td_string!(locale, deposit.toast.session_expired),
                            ToastType::Warning,
                        );
                        let aid = match params.get() {
                            Ok(p) => p.attendee_id.unwrap_or_default(),
                            Err(_) => return,
                        };
                        let eid = extract_event_id_from_url();
                        match api::get_deposit_status(&aid, eid.as_deref()).await {
                            Ok(data) => {
                                set_state.set(DepositPageState::ThbAuthRequired(data));
                            }
                            Err(_) => {
                                set_state.set(DepositPageState::Error(DepositError::ReloadFailed));
                            }
                        }
                        return;
                    }
                    let error_msg =
                        if e.to_string().contains("413") || e.to_string().contains("too large") {
                            td_string!(locale, deposit.toast.too_large).to_string()
                        } else if e.to_string().contains("File size exceeds") {
                            e.to_string()
                        } else {
                            format!("{} {e}", td_string!(locale, deposit.toast.upload_failed))
                        };
                    app_components::show_toast(&set_toast, &error_msg, ToastType::Error);
                    let aid = match params.get() {
                        Ok(p) => p.attendee_id.unwrap_or_default(),
                        Err(_) => return,
                    };
                    let eid = extract_event_id_from_url();
                    match api::get_deposit_status(&aid, eid.as_deref()).await {
                        Ok(data) => {
                            set_state.set(DepositPageState::ChoosePayment(data));
                        }
                        Err(_) => {
                            set_state.set(DepositPageState::Error(DepositError::ReloadFailed));
                        }
                    }
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Copy payment URL
// ---------------------------------------------------------------------------

/// Create handler: copy payment URL to clipboard.
pub fn make_copy_url(
    set_toast: WriteSignal<Option<app_components::ToastMessage>>,
    set_pay_url_copied: WriteSignal<bool>,
) -> impl Fn(String) + Clone + Send + Sync + 'static {
    let i18n = use_i18n();
    move |url: String| {
        let locale = i18n.get_locale_untracked();
        if js_interop::copy_to_clipboard(&url) {
            set_pay_url_copied.set(true);
            app_components::show_toast(
                &set_toast,
                td_string!(locale, deposit.toast.link_copied),
                ToastType::Success,
            );
            set_timeout(
                move || set_pay_url_copied.set(false),
                std::time::Duration::from_secs(3),
            );
        } else {
            app_components::show_toast(
                &set_toast,
                td_string!(locale, deposit.toast.copy_failed),
                ToastType::Error,
            );
        }
    }
}
