//! Shared types, constants, and helper functions for the deposit page.

use leptos::prelude::*;
use leptos_router::params::Params;
use leptos_router::params::ParamsError;

use crate::api::DepositStatusResponse;

// ---------------------------------------------------------------------------
// Thai banks (2c2p payout codes)
// ---------------------------------------------------------------------------

pub const THAI_BANKS: &[(&str, &str)] = &[
    ("002", "Bangkok Bank (BBL)"),
    ("004", "Kasikornbank (KBANK)"),
    ("006", "Krung Thai Bank (KTB)"),
    ("011", "TMB Bank (TTB)"),
    ("014", "Siam Commercial Bank (SCB)"),
    ("022", "CIMB Thai Bank (CIMB)"),
    ("024", "United Overseas Bank Thai (UOBT)"),
    ("025", "Bank of Ayudhya (BAY)"),
    ("065", "Thanachart Bank"),
    ("066", "Islamic Bank of Thailand"),
    ("067", "Tisco Bank"),
    ("069", "Kiatnakin Bank (KK)"),
    ("070", "ICBC Thai"),
    ("071", "Thai Credit Retail Bank (TCRB)"),
    ("073", "Land and Houses Bank (LHBANK)"),
    ("030", "Government Saving Bank (GSB)"),
    ("033", "Government Housing Bank (GHB)"),
    ("034", "Bank for Agriculture (BAAC)"),
];

// ---------------------------------------------------------------------------
// Route params
// ---------------------------------------------------------------------------

/// Route parameters for `/deposit/:attendee_id`.
#[derive(Params, PartialEq, Clone)]
pub struct DepositParams {
    pub attendee_id: Option<String>,
}

/// Type alias for the params signal returned by `use_params::<DepositParams>()`.
pub type DepositParamsSignal = leptos::prelude::Memo<Result<DepositParams, ParamsError>>;

// ---------------------------------------------------------------------------
// Payment choice (wizard step 1)
// ---------------------------------------------------------------------------

/// Payment method chosen in Step 1 of the 2-step wizard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PaymentChoice {
    /// THB via PromptPay slip upload.
    Thb,
    /// USDC via Solana wallet or QR.
    Usdc,
}

// ---------------------------------------------------------------------------
// Page state machine
// ---------------------------------------------------------------------------

/// Top-level state machine for the deposit page flow.
#[derive(Clone, Debug)]
pub enum DepositPageState {
    /// Loading deposit status from backend.
    Loading,
    /// API or param error.
    Error(String),
    /// Deposits not enabled for this event.
    NotEnabled,
    /// Deposit already completed.
    AlreadyDeposited(DepositStatusResponse),
    /// Ready to choose payment method.
    ChoosePayment(DepositStatusResponse),
    /// Wallet connected — ready to send TX.
    WalletConnected(DepositStatusResponse, String, String),
    /// TX sent — polling for on-chain confirmation.
    AwaitingConfirmation(DepositStatusResponse, String, String),
    /// Deposit confirmed on-chain.
    DepositConfirmed(DepositStatusResponse, String),
    /// USDC QR URL generated and ready to display (QR fallback for mobile).
    UsdcQrReady(DepositStatusResponse, String),
    /// THB slip is being uploaded.
    #[allow(dead_code)]
    ThbUploading(DepositStatusResponse),
    /// THB slip uploaded successfully.
    ThbUploaded(String, String, String),
    /// Refund flow — choosing wallet to connect.
    RefundChooseWallet(DepositStatusResponse),
    /// Refund flow — wallet connected, ready to claim.
    RefundWalletConnected(DepositStatusResponse, String, String),
    /// Refund flow — signing and sending refund TX.
    RefundSigning(DepositStatusResponse, String, String),
    /// Refund flow — TX confirmed on-chain.
    RefundConfirmed(DepositStatusResponse, String),
    /// Close deposit — choosing wallet to connect.
    CloseDepositChooseWallet(DepositStatusResponse),
    /// Close deposit — wallet connected, ready to close.
    CloseDepositWalletConnected(DepositStatusResponse, String, String),
    /// Close deposit — signing TX.
    CloseDepositSigning(DepositStatusResponse, String, String),
    /// Close deposit — confirmed.
    CloseDepositConfirmed(DepositStatusResponse, String),
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Format epoch ms to a short readable date for the refund deadline.
pub fn format_refund_deadline(ms: i64) -> String {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    let month = date.get_month() + 1;
    let day = date.get_date();
    let hours = date.get_hours();
    let minutes = date.get_minutes();
    format!("{:02}/{:02} {:02}:{:02}", month, day, hours, minutes)
}

/// Format hours into a human-friendly duration label (e.g. "7 days", "3d 12h").
pub fn format_duration_label(hours: u32) -> String {
    if hours >= 24 {
        let days = hours / 24;
        let remaining = hours % 24;
        if remaining == 0 {
            if days == 1 {
                "1 day".to_string()
            } else {
                format!("{days} days")
            }
        } else {
            format!("{days}d {remaining}h")
        }
    } else {
        format!("{hours}h")
    }
}

/// Extract event context (name, tagline) from the current page state.
pub fn extract_event_context(state: &DepositPageState) -> Option<(String, String)> {
    let data: &DepositStatusResponse = match state {
        DepositPageState::AlreadyDeposited(d)
        | DepositPageState::ChoosePayment(d)
        | DepositPageState::WalletConnected(d, _, _)
        | DepositPageState::AwaitingConfirmation(d, _, _)
        | DepositPageState::DepositConfirmed(d, _)
        | DepositPageState::UsdcQrReady(d, _)
        | DepositPageState::ThbUploading(d)
        | DepositPageState::RefundChooseWallet(d)
        | DepositPageState::RefundWalletConnected(d, _, _)
        | DepositPageState::RefundSigning(d, _, _)
        | DepositPageState::RefundConfirmed(d, _)
        | DepositPageState::CloseDepositChooseWallet(d)
        | DepositPageState::CloseDepositWalletConnected(d, _, _)
        | DepositPageState::CloseDepositSigning(d, _, _)
        | DepositPageState::CloseDepositConfirmed(d, _) => d,
        _ => return None,
    };
    if data.event_name.is_empty() {
        None
    } else {
        Some((data.event_name.clone(), data.event_tagline.clone()))
    }
}

/// Compute refund deadline info from deposit data.
pub fn compute_refund_info(data: &DepositStatusResponse) -> Option<(String, String)> {
    if data.event_end_ms > 0 && data.refund_deadline_hours > 0 {
        let deadline_ms = data.event_end_ms + (i64::from(data.refund_deadline_hours) * 3_600_000);
        let deadline_date = format_refund_deadline(deadline_ms);
        let duration_label = format_duration_label(data.refund_deadline_hours);
        Some((deadline_date, duration_label))
    } else {
        None
    }
}

/// Format a USDC amount (micro-USDC to display string).
pub fn format_usdc(micro_usdc: u64) -> String {
    format!("{:.2}", micro_usdc as f64 / 1_000_000.0)
}

/// Truncate a signature for display.
pub fn truncate_sig(sig: &str) -> String {
    if sig.len() > 20 {
        format!("{}...{}", &sig[..8], &sig[sig.len() - 8..])
    } else {
        sig.to_string()
    }
}

/// Truncate a public key for display.
pub fn truncate_pk(pk: &str) -> String {
    if pk.len() > 12 {
        format!("{}...{}", &pk[..4], &pk[pk.len() - 4..])
    } else {
        pk.to_string()
    }
}

/// Extract event_id from the current browser URL query params.
pub fn extract_event_id_from_url() -> Option<String> {
    web_sys::Url::new(&web_sys::window().unwrap().location().href().unwrap())
        .ok()
        .and_then(|url| url.search_params().get("event_id"))
}

/// Get the (method_icon, method_label) pair for a deposit method.
pub fn deposit_method_display(
    method: &crate::api::DepositMethod,
) -> (crate::icons::IconName, &'static str) {
    use crate::icons::IconName;
    match method {
        crate::api::DepositMethod::Usdc => (IconName::Coin, "USDC (Solana)"),
        crate::api::DepositMethod::Thb => (IconName::Baht, "THB (PromptPay)"),
        crate::api::DepositMethod::CreditThb => (IconName::Baht, "THB Credit (held deposit)"),
        crate::api::DepositMethod::CreditUsdc => (IconName::Coin, "USDC Credit (held deposit)"),
    }
}
