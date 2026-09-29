//! Wallet error types and user-friendly error message translation.
//!
//! The JS wallet bridge (`solana_wallet.js`) returns structured error objects
//! as JSON strings when wallet operations fail. This module parses those errors
//! and maps them to actionable user-facing messages in EN or TH.

use serde::Deserialize;

use crate::i18n::{Locale, td_string};
use crate::locale::fill;

/// Raw wallet error returned by the JS bridge.
#[derive(Debug, Clone, Deserialize)]
struct RawWalletError {
    /// Marker field to distinguish error JSON from normal return values.
    #[serde(rename = "__wallet_error__")]
    is_error: bool,
    /// Wallet-specific error code (e.g., 4001 = user rejected in Phantom).
    code: Option<i32>,
    /// Error message from the wallet provider.
    message: Option<String>,
    /// Solana program execution logs (if available).
    logs: Option<Vec<String>>,
}

/// Parsed wallet error with categorized context.
#[derive(Debug, Clone)]
pub struct WalletError {
    /// Wallet-specific error code.
    pub code: Option<i32>,
    /// Raw error message from the wallet.
    pub raw_message: String,
    /// Solana program logs (if available).
    pub logs: Option<Vec<String>>,
}

/// Result of a wallet operation.
#[derive(Debug, Clone)]
pub enum WalletResult {
    /// Operation succeeded, returns the value (e.g., public key or tx signature).
    Success(String),
    /// Operation failed with a parsed error.
    Error(WalletError),
    /// Operation returned null/undefined without structured error info.
    UnknownFailure,
}

impl WalletError {
    /// Check if the user rejected the transaction.
    pub fn is_user_rejected(&self) -> bool {
        // Phantom: 4001, Solflare: 4001, generic: message contains "reject" or "denied"
        if self.code == Some(4001) {
            return true;
        }
        let msg = self.raw_message.to_lowercase();
        msg.contains("reject")
            || msg.contains("denied")
            || msg.contains("cancelled")
            || msg.contains("user rejected")
    }

    /// Check if the error suggests insufficient balance.
    pub fn is_insufficient_balance(&self) -> bool {
        let msg = self.raw_message.to_lowercase();
        msg.contains("insufficient")
            || msg.contains("0x1")
            || msg.contains("not enough")
            || msg.contains("insufficientsol")
    }

    /// Check if the error is a simulation failure.
    pub fn is_simulation_failure(&self) -> bool {
        let msg = self.raw_message.to_lowercase();
        msg.contains("simulation failed")
            || msg.contains("instruction error")
            || msg.contains("custom program error")
    }

    /// Check if the error is a timeout or network issue.
    pub fn is_network_error(&self) -> bool {
        let msg = self.raw_message.to_lowercase();
        msg.contains("timeout")
            || msg.contains("network")
            || msg.contains("rpc")
            || msg.contains("fetch")
    }
}

/// Parse a JsValue returned from the wallet bridge into a WalletResult.
pub fn parse_wallet_js_value(val: &wasm_bindgen::JsValue) -> WalletResult {
    if val.is_null() || val.is_undefined() {
        return WalletResult::UnknownFailure;
    }

    if let Some(s) = val.as_string() {
        // Check if this is a structured error JSON
        if s.starts_with("{\"__wallet_error__") {
            match serde_json::from_str::<RawWalletError>(&s) {
                Ok(raw) if raw.is_error => {
                    return WalletResult::Error(WalletError {
                        code: raw.code,
                        raw_message: raw.message.unwrap_or_default(),
                        logs: raw.logs,
                    });
                }
                _ => {
                    // Not actually an error or failed to parse the error marker
                }
            }
        }
        // Normal success string (public key or tx signature)
        return WalletResult::Success(s);
    }

    WalletResult::UnknownFailure
}

/// What went wrong, as far as the user needs to know. One category per
/// catalog message (`locales/*/wallet.json`, `tx_*`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalletFailure {
    /// JSON-RPC -32603: usually a wallet on the wrong network or no SOL.
    Internal,
    Rejected,
    InsufficientBalance,
    /// Simulation failed and the program logs mention "insufficient".
    SimulationInsufficientTokens,
    SimulationFailed,
    Network,
    /// A raw wallet message worth showing as-is.
    Other(String),
    Unknown,
}

/// Categorise a wallet error. Order matters: the first match wins.
pub fn classify(error: &WalletError) -> WalletFailure {
    let msg = &error.raw_message;
    if error.code == Some(-32603) || msg.contains("Internal error") {
        return WalletFailure::Internal;
    }
    if error.is_user_rejected() {
        return WalletFailure::Rejected;
    }
    if error.is_insufficient_balance() {
        return WalletFailure::InsufficientBalance;
    }
    if error.is_simulation_failure() {
        let tokens_short = error
            .logs
            .as_ref()
            .is_some_and(|logs| logs.iter().any(|l| l.contains("insufficient")));
        return match tokens_short {
            true => WalletFailure::SimulationInsufficientTokens,
            false => WalletFailure::SimulationFailed,
        };
    }
    if error.is_network_error() {
        return WalletFailure::Network;
    }
    match msg.as_str() {
        "" | "Unknown transaction error" | "Unknown wallet error" => WalletFailure::Unknown,
        _ => WalletFailure::Other(msg.clone()),
    }
}

/// A user-friendly message with actionable guidance, in `locale`.
///
/// Staff pages are English-only (`locale.rs`) and pass `Locale::en`;
/// attendee pages pass the current language.
pub fn user_friendly_message(error: &WalletError, locale: Locale) -> String {
    let text = match classify(error) {
        WalletFailure::Internal => td_string!(locale, wallet.tx_internal),
        WalletFailure::Rejected => td_string!(locale, wallet.tx_rejected),
        WalletFailure::InsufficientBalance => td_string!(locale, wallet.tx_insufficient),
        WalletFailure::SimulationInsufficientTokens => td_string!(locale, wallet.tx_sim_tokens),
        WalletFailure::SimulationFailed => td_string!(locale, wallet.tx_sim_retry),
        WalletFailure::Network => td_string!(locale, wallet.tx_network),
        WalletFailure::Unknown => td_string!(locale, wallet.tx_unknown),
        WalletFailure::Other(msg) => {
            return fill(td_string!(locale, wallet.tx_failed), &[("msg", &msg)]);
        }
    };
    text.to_string()
}
