//! Pure helpers for the held-credit payout (`.issues/190`): the attendee's
//! refund-account form and one-tap preview, and the organizer's payout queue.
//!
//! Validation is the domain's (`credit_payout::RefundAccount::normalized`) —
//! the same rule the worker applies and the THB deposit refund account uses —
//! so the form can never accept what the server refuses, or the reverse.

use event_checkin_domain::models::credit_payout::{
    AccountSource, PaidAmounts, RefundAccount, RefundAccountError, RefundMethod,
    SavedAccountPreview, is_overdue,
};

/// Build and validate the account from the form's raw inputs. Only the fields
/// of the chosen method are read.
pub fn account_from_form(
    method: RefundMethod,
    promptpay_id: &str,
    bank_name: &str,
    bank_account: &str,
    account_name: &str,
) -> Result<RefundAccount, RefundAccountError> {
    let account = match method {
        RefundMethod::PromptPay => RefundAccount::PromptPay {
            promptpay_id: promptpay_id.to_string(),
        },
        RefundMethod::Bank => RefundAccount::Bank {
            bank_name: bank_name.to_string(),
            bank_account: bank_account.to_string(),
            account_name: account_name.to_string(),
        },
    };
    account.normalized()
}

/// One amount field of the organizer's confirmation: blank is 0, thousands
/// separators are dropped, anything else must be a whole non-negative number.
pub fn parse_amount(raw: &str) -> Result<i64, &'static str> {
    let cleaned: String = raw.trim().chars().filter(|c| *c != ',').collect();
    match cleaned.as_str() {
        "" => Ok(0),
        digits if digits.chars().all(|c| c.is_ascii_digit()) => {
            digits.parse::<i64>().map_err(|_| "the amount is too large")
        }
        _ => Err("enter the amount as a whole number"),
    }
}

/// The organizer's confirmed payout from the THB and USDC fields.
pub fn parse_paid(thb: &str, usdc: &str) -> Result<PaidAmounts, &'static str> {
    Ok(PaidAmounts {
        thb: parse_amount(thb)?,
        usdc: parse_amount(usdc)?,
    })
}

/// How long a request has been open, for the queue: "5 hours", "3 days".
pub fn age_label(age_hours: i64) -> String {
    match age_hours {
        h if h < 1 => "under an hour".to_string(),
        1 => "1 hour".to_string(),
        h if h < 24 => format!("{h} hours"),
        h if h < 48 => "1 day".to_string(),
        h => format!("{} days", h / 24),
    }
}

/// The queue's age line, flagged past the 7-day promise (D3).
pub fn age_line(age_hours: i64) -> String {
    match is_overdue(age_hours) {
        true => format!("Open {} — past the 7-day promise", age_label(age_hours)),
        false => format!("Open {}", age_label(age_hours)),
    }
}

/// The account as the organizer reads it: `(label, value)` lines.
pub fn account_lines(account: &RefundAccount) -> Vec<(&'static str, String)> {
    match account {
        RefundAccount::PromptPay { promptpay_id } => {
            vec![("PromptPay", promptpay_id.clone())]
        }
        RefundAccount::Bank {
            bank_name,
            bank_account,
            account_name,
        } => vec![
            ("Bank", bank_name.clone()),
            ("Account", bank_account.clone()),
            ("Name", account_name.clone()),
        ],
    }
}

/// What "Copy account" puts on the clipboard: the digits a bank app's
/// transfer field takes.
pub fn account_copy_value(account: &RefundAccount) -> String {
    let raw = match account {
        RefundAccount::PromptPay { promptpay_id } => promptpay_id.as_str(),
        RefundAccount::Bank { bank_account, .. } => bank_account.as_str(),
    };
    raw.chars().filter(char::is_ascii_digit).collect()
}

/// The saved account as the attendee sees it on the one-tap card:
/// `Kasikornbank •••• 7890 · Somchai J.` or `PromptPay •••• 5678`. Built from
/// the worker's masked preview only — the attendee API never sends more.
pub fn saved_account_label(preview: &SavedAccountPreview, promptpay_label: &str) -> String {
    let name = match preview.method {
        RefundMethod::PromptPay => promptpay_label.to_string(),
        RefundMethod::Bank => preview.bank_name.clone().unwrap_or_default(),
    };
    let number = match preview.last4.is_empty() {
        true => format!("{name} ••••"),
        false => format!("{name} •••• {}", preview.last4),
    };
    match &preview.holder {
        Some(holder) => format!("{number} · {holder}"),
        None => number,
    }
}

/// "We'll send it to {account} — the account from your deposit on {date}."
/// The template follows where the account came from; `date` is already
/// formatted for the reader's language.
pub fn saved_account_sentence(
    preview: &SavedAccountPreview,
    from_deposit: &str,
    from_attendee: &str,
    account_label: &str,
    date: &str,
) -> String {
    let template = match preview.source {
        AccountSource::Deposit => from_deposit,
        AccountSource::Attendee => from_attendee,
    };
    crate::locale::fill(template, &[("account", account_label), ("date", date)])
}

/// The organizer queue's provenance badge: `(css class, text)`. A replaced
/// deposit account is a warning — someone with the attendee's session changed
/// where the money goes, so the organizer confirms before paying.
pub fn account_source_badge(
    source: Option<AccountSource>,
    replaced_deposit: bool,
    captured_date: &str,
) -> Option<(&'static str, String)> {
    match (source, replaced_deposit) {
        (None, _) => None,
        (Some(_), true) => Some((
            "badge badge-danger",
            "Changed from the deposit account — confirm with the attendee before paying"
                .to_string(),
        )),
        (Some(AccountSource::Deposit), false) => Some((
            "badge badge-info",
            match captured_date.is_empty() {
                true => "From deposit".to_string(),
                false => format!("From deposit on {captured_date}"),
            },
        )),
        (Some(AccountSource::Attendee), false) => {
            Some(("badge badge-neutral", "Entered by attendee".to_string()))
        }
    }
}
