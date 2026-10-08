//! "Request Return of Held Credit" (Issue #061 Phase 3, `.issues/190`).
//!
//! The attendee asks for their held credit back. When they already gave a
//! refund account with their deposit (or on an earlier request), the card
//! names it, masked — "We'll send it to KBank •••• 7890" — and one tap
//! requests the refund to it. "Use a different account", or no account on
//! file, opens the form: a PromptPay ID or a bank account, validated by the
//! same rule the THB deposit refund account and the worker use
//! (`utils::credit_payout`). The organizer sees the account in the payout
//! queue, transfers the money, and records it.
//!
//! Once a payout has settled everything held, the card says so ("Credit Paid
//! Back", `.issues/192`) instead of offering a refund of money already sent:
//! an organizer payout leaves no request behind to show.

use event_checkin_domain::models::credit_payout::{
    CreditPayoutReceipt, RefundAccountError, RefundMethod, SavedAccountPreview,
};
use leptos::prelude::*;

use crate::api;
use crate::icons::{Icon, IconName};
use crate::pages::deposit::types::THAI_BANKS;
use crate::pages::ticket::credit_chip::credit_balance_label;
use crate::utils::credit_payout::{account_from_form, saved_account_label, saved_account_sentence};

/// State machine for the "Request Return of Held Credit" flow. `Loading`
/// fetches the attendee's own flag state on mount, so a reload mounts in
/// `AlreadyRequested` if a request is open.
#[derive(Clone)]
enum RequestCreditRefundState {
    /// Initial fetch of the flag state is in flight.
    Loading,
    /// No open request — show CTA.
    Ready,
    /// The refund-account form and the confirm button.
    Confirm,
    /// POST in flight.
    Requesting,
    /// Server reports an open request from a prior call (page reload).
    AlreadyRequested,
    /// The last payout settled everything held; nothing left to request.
    PaidBack(CreditPayoutReceipt),
    /// Success — this call set the flag.
    Requested { message: String },
    /// Error.
    Error(String),
}

/// "We'll send it to KBank •••• 7890 · Somchai J. — the account from your
/// deposit on 30 Aug 2026." Follows a language switch.
fn saved_account_text(preview: SavedAccountPreview) -> impl Fn() -> String {
    let i18n = crate::i18n::use_i18n();
    move || {
        let locale = i18n.get_locale();
        let label = saved_account_label(
            &preview,
            crate::i18n::td_string!(locale, ticket.action.return_promptpay),
        );
        saved_account_sentence(
            &preview,
            crate::i18n::td_string!(locale, ticket.action.return_saved_from_deposit),
            crate::i18n::td_string!(locale, ticket.action.return_saved_from_attendee),
            &label,
            &crate::utils::format_iso_day(&preview.captured_at),
        )
    }
}

/// "The organizer sent you 500 THB on 9 Oct 2026." Follows a language switch.
fn paid_back_text(receipt: CreditPayoutReceipt) -> impl Fn() -> String {
    let i18n = crate::i18n::use_i18n();
    let amount = credit_balance_label(receipt.thb.max(0) as u64, receipt.usdc.max(0) as u64)
        .unwrap_or_default();
    move || {
        let date = crate::utils::format_iso_day(&receipt.paid_at);
        crate::locale::fill(
            crate::i18n::td_string!(i18n.get_locale(), ticket.action.return_paid_desc),
            &[("amount", amount.as_str()), ("date", date.as_str())],
        )
    }
}

/// The attendee-facing text for a refused account (the domain's messages are
/// the API's, in English).
fn account_error_text(error: RefundAccountError) -> Signal<&'static str> {
    match error {
        RefundAccountError::PromptPayIdInvalid => {
            crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_err_promptpay))
        }
        RefundAccountError::BankAccountMissing
        | RefundAccountError::BankNameMissing
        | RefundAccountError::AccountNameMissing => {
            crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_err_bank))
        }
    }
}

/// Request Return of Held Credit action card.
///
/// Rendered on the ticket page / My Registrations once the attendee holds
/// credit. Backend:
/// - `POST /api/deposit/request-credit-refund` with `{account}` (JWT email;
///   VULN-012 pattern).
/// - `GET /api/deposit/credit-refund-request` reads own flag state.
#[component]
pub fn RequestCreditRefundCard() -> impl IntoView {
    let (state, set_state) = signal(RequestCreditRefundState::Loading);
    let (method, set_method) = signal(RefundMethod::PromptPay);
    let (promptpay_id, set_promptpay_id) = signal(String::new());
    let (bank_name, set_bank_name) = signal(String::new());
    let (bank_account, set_bank_account) = signal(String::new());
    let (account_name, set_account_name) = signal(String::new());
    let (form_error, set_form_error) = signal(None::<Signal<&'static str>>);
    // The masked account already on file; `None` → the form, as before.
    let (saved, set_saved) = signal(None::<SavedAccountPreview>);

    // On mount: fetch the attendee's own flag state. A failed read (or signed
    // out) degrades to `Ready`: the read never redirects, because this card
    // mounts on the public ticket page (`.issues/142`).
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            let status = api::get_credit_refund_request_status().await;
            let status = status.unwrap_or_default();
            set_saved.set(status.saved_account);
            set_state.set(match (status.requested, status.paid_back) {
                (true, _) => RequestCreditRefundState::AlreadyRequested,
                (false, Some(receipt)) => RequestCreditRefundState::PaidBack(receipt),
                (false, None) => RequestCreditRefundState::Ready,
            });
        });
    });

    let submit = move |_| {
        let account = account_from_form(
            method.get_untracked(),
            &promptpay_id.get_untracked(),
            &bank_name.get_untracked(),
            &bank_account.get_untracked(),
            &account_name.get_untracked(),
        );
        let account = match account {
            Ok(account) => account,
            Err(e) => {
                set_form_error.set(Some(account_error_text(e)));
                return;
            }
        };
        set_form_error.set(None);
        set_state.set(RequestCreditRefundState::Requesting);
        leptos::task::spawn_local(async move {
            match api::request_credit_refund(&account).await {
                Ok(resp) => {
                    log::info!("[credit-refund] request submitted");
                    set_state.set(RequestCreditRefundState::Requested {
                        message: resp.message,
                    });
                }
                Err(e) => {
                    log::error!("[credit-refund] failed: {}", e.message);
                    set_state.set(RequestCreditRefundState::Error(e.message));
                }
            }
        });
    };

    // One tap: the account on file. A 400 (it was paid out or purged in the
    // meantime) shows the error with "try again", which opens the form.
    let submit_saved = move |_| {
        set_state.set(RequestCreditRefundState::Requesting);
        leptos::task::spawn_local(async move {
            match api::request_credit_refund_saved().await {
                Ok(resp) => {
                    log::info!("[credit-refund] request to the saved account submitted");
                    set_state.set(RequestCreditRefundState::Requested {
                        message: resp.message,
                    });
                }
                Err(e) => {
                    log::error!(
                        "[credit-refund] saved-account request failed: {}",
                        e.message
                    );
                    set_saved.set(None);
                    set_state.set(RequestCreditRefundState::Error(e.message));
                }
            }
        });
    };

    let method_button = move |choice: RefundMethod, label: Signal<&'static str>| {
        view! {
            <button
                class=move || match method.get() == choice {
                    true => "btn btn-primary btn-xs",
                    false => "btn btn-outline btn-xs",
                }
                on:click=move |_| {
                    set_method.set(choice);
                    set_form_error.set(None);
                }
            >
                {label}
            </button>
        }
    };

    let account_form = move || {
        view! {
            <div class="ticket-action-desc">
                <div class="flex-row-wrap u-mt-xs">
                    {method_button(
                        RefundMethod::PromptPay,
                        crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_promptpay)),
                    )}
                    {method_button(
                        RefundMethod::Bank,
                        crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_bank)),
                    )}
                </div>
                {move || match method.get() {
                    RefundMethod::PromptPay => view! {
                        <input
                            type="text"
                            inputmode="numeric"
                            autocomplete="off"
                            class="form-input dep-input u-mt-xs"
                            placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_promptpay_placeholder))
                            prop:value=move || promptpay_id.get()
                            on:input=move |ev| set_promptpay_id.set(event_target_value(&ev))
                        />
                    }.into_any(),
                    RefundMethod::Bank => view! {
                        <input
                            type="text"
                            list="credit-refund-banks"
                            class="form-input dep-input u-mt-xs"
                            placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.bank_name))
                            prop:value=move || bank_name.get()
                            on:input=move |ev| set_bank_name.set(event_target_value(&ev))
                        />
                        <datalist id="credit-refund-banks">
                            {THAI_BANKS.iter().map(|(_, name)| view! { <option value=*name></option> }).collect_view()}
                        </datalist>
                        <input
                            type="text"
                            inputmode="numeric"
                            autocomplete="off"
                            class="form-input dep-input u-mt-xs"
                            placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.bank_account))
                            prop:value=move || bank_account.get()
                            on:input=move |ev| set_bank_account.set(event_target_value(&ev))
                        />
                        <input
                            type="text"
                            class="form-input dep-input u-mt-xs"
                            placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, deposit.thb.account_name))
                            prop:value=move || account_name.get()
                            on:input=move |ev| set_account_name.set(event_target_value(&ev))
                        />
                    }.into_any(),
                }}
                {move || form_error.get().map(|msg| view! {
                    <div class="ticket-action-desc ticket-action-title-danger">{msg}</div>
                })}
            </div>
        }
    };

    view! {
        <div class="ticket-action-card ticket-action-card--credit-refund">
            <div class="ticket-action-icon">
                <Icon icon=IconName::MoneyWings class="icon-sm" />
            </div>
            <div>
                {move || match state.get() {
                    RequestCreditRefundState::Loading => view! {
                        <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_checking))}</div>
                        <div class="ticket-action-desc ticket-action-signing-row">
                            <span class="spinner spinner-sm"></span>
                        </div>
                    }.into_any(),

                    RequestCreditRefundState::Ready => match saved.get() {
                        Some(preview) => view! {
                            <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_title))}</div>
                            <div class="ticket-action-desc">
                                {saved_account_text(preview)}
                            </div>
                            <button
                                class="btn btn-primary btn-sm ticket-action-btn"
                                on:click=submit_saved
                            >
                                <Icon icon=IconName::Check class="icon-sm" />
                                " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_saved_cta))}
                            </button>
                            <button
                                class="btn btn-outline btn-xs ticket-action-cancel"
                                on:click=move |_| set_state.set(RequestCreditRefundState::Confirm)
                            >
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_saved_other))}
                            </button>
                        }.into_any(),
                        None => view! {
                            <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_title))}</div>
                            <div class="ticket-action-desc">
                                {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_desc))}
                            </div>
                            <button
                                class="btn btn-outline btn-sm ticket-action-btn"
                                on:click=move |_| set_state.set(RequestCreditRefundState::Confirm)
                            >
                                <Icon icon=IconName::Check class="icon-sm" />
                                " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_cta))}
                            </button>
                        }.into_any(),
                    },

                    RequestCreditRefundState::Confirm => view! {
                        <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_confirm_title))}</div>
                        <div class="ticket-action-desc">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_confirm_desc))}
                        </div>
                        {account_form()}
                        <button
                            class="btn btn-primary btn-sm ticket-action-btn"
                            on:click=submit
                        >
                            <Icon icon=IconName::Check class="icon-sm" />
                            " "{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_confirm_cta))}
                        </button>
                        <button
                            class="btn btn-outline btn-xs ticket-action-cancel"
                            on:click=move |_| set_state.set(RequestCreditRefundState::Ready)
                        >
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.cancel))}
                        </button>
                    }.into_any(),

                    RequestCreditRefundState::Requesting => view! {
                        <div class="ticket-action-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_submitting))}</div>
                        <div class="ticket-action-desc ticket-action-signing-row">
                            <span class="spinner spinner-sm"></span>
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.processing_request))}
                        </div>
                    }.into_any(),

                    RequestCreditRefundState::AlreadyRequested => view! {
                        <div class="ticket-action-title ticket-action-title-success">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_requested))}
                        </div>
                        <div class="ticket-action-desc">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_requested_desc))}
                        </div>
                    }.into_any(),

                    RequestCreditRefundState::PaidBack(receipt) => view! {
                        <div class="ticket-action-title ticket-action-title-success">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_paid_title))}
                        </div>
                        <div class="ticket-action-desc">{paid_back_text(receipt)}</div>
                    }.into_any(),

                    RequestCreditRefundState::Requested { message } => view! {
                        <div class="ticket-action-title ticket-action-title-success">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_submitted))}
                        </div>
                        <div class="ticket-action-desc">{message.clone()}</div>
                    }.into_any(),

                    RequestCreditRefundState::Error(msg) => view! {
                        <div class="ticket-action-title ticket-action-title-danger">
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.return_failed))}
                        </div>
                        <div class="ticket-action-desc">{msg.clone()}</div>
                        <button
                            class="btn btn-outline btn-xs ticket-action-cancel-xs"
                            on:click=move |_| set_state.set(RequestCreditRefundState::Confirm)
                        >
                            {crate::locale::tr(|l| crate::i18n::td_string!(l, ticket.action.try_again))}
                        </button>
                    }.into_any(),
                }}
            </div>
        </div>
    }
}
