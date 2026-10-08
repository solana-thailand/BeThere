//! Paying held rolling credit back out (`.issues/190`).
//!
//! Shared by the worker (validation, the guarded ledger write, the admin
//! queue) and the frontend (the attendee's request form, the organizer's
//! queue). Pure: no I/O, compiles for wasm32.
//!
//! - [`RefundAccount`] is where the attendee wants the money. The bank shape
//!   is the one the THB deposit refund account has always used (same field
//!   names, same rules, same messages — [`validate_bank_refund_fields`] is
//!   what the slip upload paths call too). PromptPay is the second option.
//! - [`PaidAmounts`] is what the organizer says they actually transferred.
//!   The server compares it against the payable balance inside the ledger
//!   write, so a balance that moved after the organizer looked is a 409, not
//!   a reversal of a number nobody saw.
//! - [`PayoutScope`] is which organizations' credit a caller may pay out.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

/// THB comes back within 7 days (owner decision D3, 2026-09-28). A credit
/// refund request older than this is overdue.
pub const REFUND_WINDOW_DAYS: i64 = 7;

/// Where an attendee wants a refund sent.
///
/// Wire shape: `{"method":"promptpay","promptpay_id":"0812345678"}` or
/// `{"method":"bank","bank_name":"KBank","bank_account":"123-4-56789-0",
/// "account_name":"Somchai"}`. The bank field names match the deposit slip
/// upload body (`ThbSlipUploadRequest`), so both forms speak one shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum RefundAccount {
    #[serde(rename = "promptpay")]
    PromptPay { promptpay_id: String },
    Bank {
        bank_name: String,
        bank_account: String,
        account_name: String,
    },
}

/// The stored discriminant of a [`RefundAccount`] (`credit_refund_accounts.method`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefundMethod {
    PromptPay,
    Bank,
}

impl RefundMethod {
    /// The value the D1 `CHECK (method IN ('promptpay','bank'))` accepts.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PromptPay => "promptpay",
            Self::Bank => "bank",
        }
    }

    /// Parse a stored value. An unknown value is `None` — never a default, so
    /// a corrupt row shows "no account on file" rather than the wrong one.
    pub fn parse(stored: &str) -> Option<Self> {
        match stored {
            "promptpay" => Some(Self::PromptPay),
            "bank" => Some(Self::Bank),
            _ => None,
        }
    }
}

/// Why a refund account was refused. `Display` is the API message; none of
/// them contain `://` (the error redactor would eat it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefundAccountError {
    BankAccountMissing,
    BankNameMissing,
    AccountNameMissing,
    PromptPayIdInvalid,
}

impl fmt::Display for RefundAccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The three bank messages are the ones the deposit slip upload has
        // always returned; changing them changes that endpoint too.
        f.write_str(match self {
            Self::BankAccountMissing => "bank_account is required",
            Self::BankNameMissing => "bank_name is required",
            Self::AccountNameMissing => "account_name is required",
            Self::PromptPayIdInvalid => {
                "PromptPay ID must be a 10-digit phone number starting with 0 \
                 or a 13-digit ID number"
            }
        })
    }
}

impl std::error::Error for RefundAccountError {}

fn present(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.trim().is_empty())
}

/// The THB deposit refund account rule: account number, bank name and account
/// holder are all required (non-blank). Checked in that order, so the first
/// missing field is the one reported — the order the slip upload always used.
pub fn validate_bank_refund_fields(
    bank_account: Option<&str>,
    bank_name: Option<&str>,
    account_name: Option<&str>,
) -> Result<(), RefundAccountError> {
    match (
        present(bank_account),
        present(bank_name),
        present(account_name),
    ) {
        (false, _, _) => Err(RefundAccountError::BankAccountMissing),
        (_, false, _) => Err(RefundAccountError::BankNameMissing),
        (_, _, false) => Err(RefundAccountError::AccountNameMissing),
        (true, true, true) => Ok(()),
    }
}

/// A PromptPay ID as digits: a mobile number (10 digits, leading 0) or a
/// national / tax ID (13 digits). Spaces and dashes are dropped first, the way
/// people type them; anything else is refused.
pub fn normalize_promptpay_id(raw: &str) -> Result<String, RefundAccountError> {
    let digits: String = raw.chars().filter(|c| !matches!(c, ' ' | '-')).collect();
    let all_digits = !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit());
    match (all_digits, digits.len(), digits.starts_with('0')) {
        (true, 10, true) | (true, 13, _) => Ok(digits),
        _ => Err(RefundAccountError::PromptPayIdInvalid),
    }
}

impl RefundAccount {
    pub fn method(&self) -> RefundMethod {
        match self {
            Self::PromptPay { .. } => RefundMethod::PromptPay,
            Self::Bank { .. } => RefundMethod::Bank,
        }
    }

    /// Validate and return the stored form: fields trimmed, the PromptPay ID
    /// reduced to digits. The bank half is [`validate_bank_refund_fields`].
    pub fn normalized(self) -> Result<Self, RefundAccountError> {
        match self {
            Self::PromptPay { promptpay_id } => Ok(Self::PromptPay {
                promptpay_id: normalize_promptpay_id(&promptpay_id)?,
            }),
            Self::Bank {
                bank_name,
                bank_account,
                account_name,
            } => {
                validate_bank_refund_fields(
                    Some(&bank_account),
                    Some(&bank_name),
                    Some(&account_name),
                )?;
                Ok(Self::Bank {
                    bank_name: bank_name.trim().to_string(),
                    bank_account: bank_account.trim().to_string(),
                    account_name: account_name.trim().to_string(),
                })
            }
        }
    }

    /// Rebuild from the stored columns. `None` when the method is unknown or
    /// its fields are missing — the caller shows "no account on file".
    pub fn from_columns(
        method: &str,
        promptpay_id: Option<String>,
        bank_name: Option<String>,
        bank_account: Option<String>,
        account_name: Option<String>,
    ) -> Option<Self> {
        match RefundMethod::parse(method)? {
            RefundMethod::PromptPay => Some(Self::PromptPay {
                promptpay_id: promptpay_id?,
            }),
            RefundMethod::Bank => Some(Self::Bank {
                bank_name: bank_name?,
                bank_account: bank_account?,
                account_name: account_name?,
            }),
        }
    }
}

/// What the organizer actually transferred, per currency, in the ledger's own
/// units (whole baht for THB — the unit of `amount_thb` and every THB ledger
/// row; USDC as the ledger stores it). Absent currencies are 0.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaidAmounts {
    #[serde(default)]
    pub thb: i64,
    #[serde(default)]
    pub usdc: i64,
}

impl PaidAmounts {
    pub fn is_zero(&self) -> bool {
        self.thb == 0 && self.usdc == 0
    }

    /// Negative amounts are not a payout.
    pub fn validate(&self) -> Result<(), &'static str> {
        match (self.thb < 0, self.usdc < 0) {
            (false, false) => Ok(()),
            _ => Err("paid amounts cannot be negative"),
        }
    }

    /// Add one ledger bucket's balance. Unknown currencies are ignored: they
    /// cannot be confirmed, so the comparison will not match them either.
    pub fn add(&mut self, currency: &str, amount: i64) {
        match currency {
            "thb" => self.thb += amount,
            "usdc" => self.usdc += amount,
            _ => {}
        }
    }
}

impl fmt::Display for PaidAmounts {
    /// THB first (owner decision D5): `500 THB`, `500 THB + 2 USDC`, `0`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.thb, self.usdc) {
            (0, 0) => f.write_str("0"),
            (thb, 0) => write!(f, "{thb} THB"),
            (0, usdc) => write!(f, "{usdc} USDC"),
            (thb, usdc) => write!(f, "{thb} THB + {usdc} USDC"),
        }
    }
}

/// The 409 text when the confirmed payout no longer matches the payable
/// balance. Nothing was written when this is returned.
pub fn payout_mismatch_message(confirmed: PaidAmounts, payable: PaidAmounts) -> String {
    format!(
        "the payable balance is now {payable}, not the {confirmed} you confirmed. \
         Nothing was recorded — reload the request and check before clearing it."
    )
}

/// Which organizations' credit a caller may see and pay out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayoutScope {
    /// Super-admin: every organization.
    All,
    /// These organization ids (`""` is the default organization).
    Orgs(BTreeSet<String>),
}

impl PayoutScope {
    /// True when the caller may act on credit spread over `orgs`: every one of
    /// them has to be in scope, or the payout would move another
    /// organization's money.
    pub fn covers<'a>(&self, orgs: impl IntoIterator<Item = &'a str>) -> bool {
        match self {
            Self::All => true,
            Self::Orgs(allowed) => orgs.into_iter().all(|org| allowed.contains(org)),
        }
    }

    /// No organization at all — the caller is not a payout operator.
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Orgs(allowed) if allowed.is_empty())
    }
}

/// A request open longer than the D3 window.
pub fn is_overdue(age_hours: i64) -> bool {
    age_hours > REFUND_WINDOW_DAYS * 24
}
