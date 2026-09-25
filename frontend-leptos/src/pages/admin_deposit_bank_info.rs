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
