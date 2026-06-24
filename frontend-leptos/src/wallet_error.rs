//! Wallet error types and user-friendly error message translation.
//!
//! The JS wallet bridge (`solana_wallet.js`) returns structured error objects
//! as JSON strings when wallet operations fail. This module parses those errors
//! and maps them to actionable user-facing messages.

use serde::Deserialize;

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

/// Generate a user-friendly error message with actionable guidance.
pub fn user_friendly_message(error: &WalletError) -> String {
    let code = error.code;
    let msg = &error.raw_message;
    if code == Some(-32603) || msg.contains("Internal error") {
        return "Transaction failed (Internal error). Please check if your wallet extension network settings match the app's network (e.g., Devnet) and that you have enough SOL for fees.".to_string();
    }

    if error.is_user_rejected() {
        return "You cancelled the transaction. Tap the button to try again when ready."
            .to_string();
    }

    if error.is_insufficient_balance() {
        return "Insufficient balance. Make sure you have enough SOL for fees and USDC for the deposit.".to_string();
    }

    if error.is_simulation_failure() {
        let base = "Transaction simulation failed.".to_string();
        if let Some(logs) = &error.logs {
            if logs.iter().any(|l| l.contains("insufficient")) {
                return format!("{base} You may not have enough tokens for this transaction.");
            }
        }
        return format!("{base} This may be temporary — please try again in a few seconds.");
    }

    if error.is_network_error() {
        return "Network issue. Your transaction may still be processing. Check your wallet history before retrying.".to_string();
    }

    // Generic fallback with the raw message if it's useful
    let msg = &error.raw_message;
    if msg.is_empty() || msg == "Unknown transaction error" || msg == "Unknown wallet error" {
        "Something went wrong. Please try again.".to_string()
    } else {
        format!("Transaction failed: {msg}")
    }
}

/// Convenience: extract a user-friendly message from a WalletResult.
/// Returns `None` for success, `Some(msg)` for errors.
pub fn wallet_error_message(result: &WalletResult) -> Option<String> {
    match result {
        WalletResult::Success(_) => None,
        WalletResult::Error(e) => Some(user_friendly_message(e)),
        WalletResult::UnknownFailure => Some("Something went wrong. Please try again.".to_string()),
    }
}

/// Translate an API error into a user-friendly message.
/// Maps known server error patterns to human-readable guidance.
pub fn translate_api_error(error: &crate::api::ApiError) -> String {
    let msg = &error.message;

    // Deposit-specific errors
    if msg.contains("deposit not enabled") {
        return "Deposits are no longer being accepted for this event.".to_string();
    }
    if msg.contains("event has ended") {
        return "This event has ended. Deposits are no longer accepted.".to_string();
    }
    if msg.contains("already has a deposit") {
        return "You have already made a deposit for this event.".to_string();
    }
    if msg.contains("invalid wallet address") {
        return "Invalid wallet address. Please check and try again.".to_string();
    }
    if msg.contains("not checked in") {
        return "You need to be checked in at the event first.".to_string();
    }
    if msg.contains("already been claimed") || msg.contains("already claimed") {
        return "This NFT badge has already been claimed.".to_string();
    }

    // Helius/RPC errors
    if msg.contains("helius") || msg.contains("rpc error") {
        return "Network issue. Please try again in a moment.".to_string();
    }

    // Escrow errors
    if msg.contains("escrow") && msg.contains("not initialized") {
        return "Event escrow is not set up yet. Please contact the organizer.".to_string();
    }
    if msg.contains("not refundable") || msg.contains("refundable") {
        return "This deposit is not eligible for a refund.".to_string();
    }

    // Rate limiting
    if error.status == 429 {
        return "Too many requests. Please wait a moment and try again.".to_string();
    }

    // Auth errors
    if error.status == 401 {
        return "Session expired. Please sign in again.".to_string();
    }

    // Generic by status
    match error.status {
        0 => "Network error. Please check your internet connection.".to_string(),
        400 => format!("Invalid request: {msg}"),
        404 => "The requested resource was not found.".to_string(),
        500..=599 => "Server error. Please try again later.".to_string(),
        _ => format!("{msg}"),
    }
}

/// Convenience: translate an API Result's error side.
pub fn api_error_message<T>(result: &Result<T, crate::api::ApiError>) -> Option<String> {
    match result {
        Ok(_) => None,
        Err(e) => Some(translate_api_error(e)),
    }
}
