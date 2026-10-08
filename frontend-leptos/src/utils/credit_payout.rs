//! Pure helpers for the held-credit payout (`.issues/190`): the attendee's
//! refund-account form and the organizer's payout queue.
//!
//! Validation is the domain's (`credit_payout::RefundAccount::normalized`) —
//! the same rule the worker applies and the THB deposit refund account uses —
//! so the form can never accept what the server refuses, or the reverse.

use event_checkin_domain::models::credit_payout::{
    PaidAmounts, RefundAccount, RefundAccountError, RefundMethod, is_overdue,
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
