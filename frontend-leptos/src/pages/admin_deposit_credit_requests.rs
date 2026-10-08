//! Held-as-Credit tab: the "Credit Refund Requested" payout queue (Issue #061
//! Phase 3, hardened in `.issues/190`). Contacts who asked for their held
//! credit back, with where they want it sent and how long they have waited.
//!
//! The organizer transfers the money out-of-band, then records it here: they
//! type the amount they actually sent (and may attach the transfer slip). The
//! worker writes the payout only if that still equals the payable balance; if
//! a registration spent some of it meanwhile, the clear is refused with a 409
//! and nothing is recorded.

use leptos::prelude::*;
use wasm_bindgen::JsValue;

use crate::api::{self, ClearCreditRefundRequest, CreditRefundRequest};
use crate::components::{self, ToastMessage, ToastType};
use crate::icons::{Icon, IconName};
use crate::utils;
use crate::utils::credit_payout::{account_copy_value, account_lines, age_line, parse_paid};
use event_checkin_domain::models::credit_payout::is_overdue;

/// Same cap as the deposit refund proof (`admin_deposit.rs`): the worker's
/// slip validator refuses a data URL over 5 MB.
const MAX_PROOF_DATA_URL_LEN: usize = 5 * 1024 * 1024;

#[component]
pub fn CreditRefundRequests(
    requests: ReadSignal<Vec<CreditRefundRequest>>,
    set_toast: WriteSignal<Option<ToastMessage>>,
    set_refresh_counter: WriteSignal<u32>,
) -> impl IntoView {
    view! {
        <div class="admin-dep-credit-refund-requests">
            <div class="admin-dep-flow-hint">
                <Icon icon=IconName::Warning class="icon-sm"/>
                {move || {
                    let count = requests.get().len();
                    format!(
                        "{} contact{} asked for held credit back. Transfer it to the account shown, then record the amount you sent.",
                        count,
                        if count != 1 { "s" } else { "" }
                    )
                }}
            </div>
            {move || {
                requests
                    .get()
                    .into_iter()
                    .map(|req| view! {
                        <CreditRefundRow req=req set_toast=set_toast set_refresh_counter=set_refresh_counter/>
                    })
                    .collect_view()
            }}
        </div>
    }
}

/// One open request: the account, the age, and the record-payout form.
#[component]
fn CreditRefundRow(
    req: CreditRefundRequest,
    set_toast: WriteSignal<Option<ToastMessage>>,
    set_refresh_counter: WriteSignal<u32>,
) -> impl IntoView {
    let CreditRefundRequest {
        email,
        name,
        credit_thb,
        credit_usdc,
        locked_thb,
        locked_until,
        requested_at,
        age_hours,
        account,
    } = req;
    let (paid_thb, set_paid_thb) = signal(String::new());
    let (paid_usdc, set_paid_usdc) = signal(String::new());
    let (proof, set_proof) = signal(None::<String>);
    let (pending, set_pending) = signal(false);

    // Issue #120 §3. Nothing payable and credit still covering an event that
    // has not ended: clearing would reverse nothing and drop the request. The
    // server refuses this with a 409 — this only saves the round trip.
    let locked_only = credit_thb == 0 && credit_usdc == 0 && locked_thb > 0;
    let display_name = match name.is_empty() {
        true => email.clone(),
        false => name,
    };
    let payable = api::PaidAmounts {
        thb: credit_thb,
        usdc: credit_usdc,
    };
    let requested_display = match requested_at.is_empty() {
        true => "N/A".to_string(),
        false => utils::format_timestamp(&requested_at),
    };
    let overdue = is_overdue(age_hours);
    let copy_value = account.as_ref().map(account_copy_value);

    let on_proof = move |ev: leptos::ev::Event| {
        let target: JsValue = event_target::<web_sys::HtmlInputElement>(&ev).into();
        leptos::task::spawn_local(async move {
            match crate::pages::deposit::js_interop::read_file_as_data_url(&target).await {
                Some(data_url) if data_url.len() <= MAX_PROOF_DATA_URL_LEN => {
                    set_proof.set(Some(data_url))
                }
                Some(_) => components::show_toast(
                    &set_toast,
                    "Slip image is over 3MB — take a screenshot of it instead",
                    ToastType::Error,
                ),
                None => components::show_toast(
                    &set_toast,
                    "Could not read that image",
                    ToastType::Error,
                ),
            }
        });
    };

    let record_payout = move |_| {
        let paid = match parse_paid(&paid_thb.get_untracked(), &paid_usdc.get_untracked()) {
            Ok(paid) => paid,
            Err(msg) => {
                components::show_toast(&set_toast, msg, ToastType::Error);
                return;
            }
        };
        let body = ClearCreditRefundRequest {
            email: email.clone(),
            paid,
            proof: proof.get_untracked(),
        };
        set_pending.set(true);
        leptos::task::spawn_local(async move {
            match api::clear_credit_refund_request(&body).await {
                Ok(_) => {
                    components::show_toast(
                        &set_toast,
                        &format!("Recorded the {paid} payout."),
                        ToastType::Success,
                    );
                    set_refresh_counter.update(|c| *c += 1);
                }
                Err(e) => {
                    log::warn!("[admin-deposit] failed to record credit payout: {e}");
                    components::show_toast(
                        &set_toast,
                        &format!("Not recorded: {e}"),
                        ToastType::Error,
                    );
                }
            }
            set_pending.set(false);
        });
    };

    let account_view = match &account {
        Some(account) => account_lines(account)
            .into_iter()
            .map(|(label, value)| {
                view! { <div class="panel-hint">{format!("{label}: {value}")}</div> }
            })
            .collect_view()
            .into_any(),
        None => view! {
            <span class="badge badge-warning">"No account on file — ask the attendee"</span>
        }
        .into_any(),
    };

    view! {
        <div class="card admin-dep-credit-refund-row">
            <div class="flex-row-wrap">
                <div>
                    <div class="admin-attendee-name">{utils::escape_html(&display_name)}</div>
                    <div class="admin-amount-line">{format!("Payable now: {payable}")}</div>
                    {(locked_thb > 0).then(|| {
                        let event = match locked_until.is_empty() {
                            true => "an event that has not ended".to_string(),
                            false => locked_until.clone(),
                        };
                        view! {
                            <div class="panel-hint">
                                {format!("{locked_thb} THB is covering {event} — it returns when that event ends")}
                            </div>
                        }
                    })}
                    <div class="panel-hint">{format!("Requested: {requested_display}")}</div>
                    <div class="panel-hint">
                        {match overdue {
                            true => view! { <span class="badge badge-danger">{age_line(age_hours)}</span> }.into_any(),
                            false => view! { <span>{age_line(age_hours)}</span> }.into_any(),
                        }}
                    </div>
                    <div class="admin-dep-bank-section">
                        <div class="panel-hint admin-dep-bank-label">"Pay to"</div>
                        {account_view}
                    </div>
                </div>
                <div>
                    <span class="badge badge-warning">"Refund Requested"</span>
                    {copy_value.map(|value| view! {
                        <button
                            class="btn btn-outline btn-sm"
                            on:click=move |_| {
                                let (msg, kind) = match crate::pages::deposit::js_interop::copy_to_clipboard(&value) {
                                    true => (format!("Copied account: {value}"), ToastType::Success),
                                    false => ("Could not copy the account".to_string(), ToastType::Error),
                                };
                                components::show_toast(&set_toast, &msg, kind);
                            }
                        >
                            "Copy account"
                        </button>
                    })}
                </div>
            </div>
            {match locked_only {
                true => view! {
                    <div class="panel-hint">
                        "Nothing to pay out yet — the credit is covering an event that has not ended. The request stays open until then."
                    </div>
                }.into_any(),
                false => view! {
                    <div class="admin-dep-confirm-row">
                        <label class="form-label">"THB you transferred"</label>
                        <input
                            type="text"
                            inputmode="numeric"
                            class="form-input dep-input"
                            placeholder=credit_thb.to_string()
                            prop:value=move || paid_thb.get()
                            on:input=move |ev| set_paid_thb.set(event_target_value(&ev))
                        />
                        {(credit_usdc > 0).then(|| view! {
                            <label class="form-label">"USDC you transferred"</label>
                            <input
                                type="text"
                                inputmode="numeric"
                                class="form-input dep-input"
                                placeholder=credit_usdc.to_string()
                                prop:value=move || paid_usdc.get()
                                on:input=move |ev| set_paid_usdc.set(event_target_value(&ev))
                            />
                        })}
                        <label class="form-label">"Transfer slip (optional — JPEG, PNG, WebP, max 3MB)"</label>
                        <input
                            type="file"
                            accept="image/jpeg,image/png,image/webp"
                            class="file-input-styled"
                            on:change=on_proof
                        />
                        {move || proof.get().is_some().then(|| view! {
                            <span class="badge badge-success">"Slip attached"</span>
                        })}
                        <button
                            class="btn btn-success btn-xs admin-dep-clear-btn"
                            disabled=move || {
                                pending.get()
                                    || (paid_thb.get().trim().is_empty() && paid_usdc.get().trim().is_empty())
                            }
                            on:click=record_payout
                        >
                            {move || match pending.get() {
                                true => view! { <span>"Recording..."</span> }.into_any(),
                                false => view! {
                                    <Icon icon=IconName::Check class="icon-sm"/>
                                    " Record payout"
                                }.into_any(),
                            }}
                        </button>
                    </div>
                }.into_any(),
            }}
        </div>
    }
}
