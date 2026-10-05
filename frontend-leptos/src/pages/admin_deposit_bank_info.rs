//! "Refund Bank Info" block shared by the admin deposit tabs (refund queue,
//! refunded, held as credit), so the three cannot drift apart.

use leptos::prelude::*;

/// The attendee's refund bank details, or `missing` as a warning badge when
/// any of the three parts is absent. The server maps empty columns to `None`.
pub fn refund_bank_info(
    bank_account: Option<String>,
    bank_name: Option<String>,
    account_name: Option<String>,
    missing: &'static str,
) -> AnyView {
    let body = match (bank_account, bank_name, account_name) {
        (Some(account), Some(bank), Some(name)) => view! {
            <div class="panel-hint">{format!("Account: {account}")}</div>
            <div class="panel-hint">{format!("Bank: {bank}")}</div>
            <div class="panel-hint">{format!("Name: {name}")}</div>
        }
        .into_any(),
        _ => view! {
            <div class="badge badge-warning admin-dep-badge-row">{missing}</div>
        }
        .into_any(),
    };
    view! {
        <div class="admin-dep-bank-section">
            <div class="panel-hint admin-dep-bank-label">"Refund Bank Info"</div>
            {body}
        </div>
    }
    .into_any()
}

/// The account number as a bank app's transfer field takes it: digits only,
/// without the dashes and spaces attendees type.
pub fn account_digits(account: &str) -> String {
    account.chars().filter(char::is_ascii_digit).collect()
}

/// One-tap copy of the refund row's account number and amount, so a refund
/// is copy → paste in the bank app → transfer, without retyping.
pub fn refund_copy_buttons(
    bank_account: Option<String>,
    amount_thb: u64,
    set_toast: WriteSignal<Option<crate::components::ToastMessage>>,
) -> AnyView {
    use crate::components::{ToastType, show_toast};
    use crate::pages::deposit::js_interop::copy_to_clipboard;

    let digits = bank_account
        .as_deref()
        .map(account_digits)
        .unwrap_or_default();
    let copy = move |text: String, what: &'static str| {
        let (msg, kind) = match copy_to_clipboard(&text) {
            true => (format!("Copied {what}: {text}"), ToastType::Success),
            false => (format!("Could not copy {what}"), ToastType::Error),
        };
        show_toast(&set_toast, &msg, kind);
    };
    view! {
        <div class="admin-dep-confirm-row">
            {(!digits.is_empty()).then(|| {
                let digits = digits.clone();
                view! {
                    <button class="btn btn-outline btn-sm" on:click=move |_| copy(digits.clone(), "account")>
                        "Copy account"
                    </button>
                }
            })}
            <button class="btn btn-outline btn-sm" on:click=move |_| copy(amount_thb.to_string(), "amount")>
                {format!("Copy {amount_thb} THB")}
            </button>
        </div>
    }
    .into_any()
}
