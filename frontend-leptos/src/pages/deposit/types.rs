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
    /// THB slip was rejected by admin — user can re-upload.
    ThbRejected(DepositStatusResponse),
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

/// Which deposit flow the user is in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DepositFlow {
    /// No flow active (loading, error, already deposited, etc.).
    None,
    /// USDC wallet or QR payment flow.
    Usdc,
    /// THB PromptPay slip upload flow.
    Thb,
    /// Refund flow.
    Refund,
    /// Close deposit / reclaim rent flow.
    CloseDeposit,
}

/// Compute the current step (1-based) within the active flow.
/// Returns (flow, current_step, total_steps) — None if no stepper should show.
///
/// `payment_choice` is used to determine the flow when state is `ChoosePayment`
/// (before the state machine transitions to a flow-specific variant).
pub fn deposit_step(
    state: &DepositPageState,
    payment_choice: Option<PaymentChoice>,
) -> Option<(DepositFlow, usize, usize)> {
    match state {
        // ChoosePayment: show stepper only after user picks a method
        DepositPageState::ChoosePayment(_) => match payment_choice {
            Some(PaymentChoice::Thb) => Some((DepositFlow::Thb, 2, 3)),
            Some(PaymentChoice::Usdc) => Some((DepositFlow::Usdc, 1, 4)),
            None => None,
        },

        // USDC flow: Choose → Connect → Pay → Confirm (4 steps)
        DepositPageState::WalletConnected(_, _, _) => Some((DepositFlow::Usdc, 2, 4)),
        DepositPageState::AwaitingConfirmation(_, _, _) | DepositPageState::UsdcQrReady(_, _) => {
            Some((DepositFlow::Usdc, 3, 4))
        }
        DepositPageState::DepositConfirmed(_, _) => Some((DepositFlow::Usdc, 4, 4)),

        // THB flow: Choose → Upload → Submitted (3 steps)
        DepositPageState::ThbUploading(_) | DepositPageState::ThbRejected(_) => {
            Some((DepositFlow::Thb, 2, 3))
        }
        DepositPageState::ThbUploaded(_, _, _) => Some((DepositFlow::Thb, 3, 3)),

        // Refund flow: Connect → Sign → Confirmed (3 steps)
        DepositPageState::RefundChooseWallet(_) => Some((DepositFlow::Refund, 1, 3)),
        DepositPageState::RefundWalletConnected(_, _, _)
        | DepositPageState::RefundSigning(_, _, _) => Some((DepositFlow::Refund, 2, 3)),
        DepositPageState::RefundConfirmed(_, _) => Some((DepositFlow::Refund, 3, 3)),

        // Close deposit flow: Connect → Sign → Confirmed (3 steps)
        DepositPageState::CloseDepositChooseWallet(_) => Some((DepositFlow::CloseDeposit, 1, 3)),
        DepositPageState::CloseDepositWalletConnected(_, _, _)
        | DepositPageState::CloseDepositSigning(_, _, _) => Some((DepositFlow::CloseDeposit, 2, 3)),
        DepositPageState::CloseDepositConfirmed(_, _) => Some((DepositFlow::CloseDeposit, 3, 3)),

        // No stepper for terminal / pre-flow states
        DepositPageState::Loading
        | DepositPageState::Error(_)
        | DepositPageState::NotEnabled
        | DepositPageState::AlreadyDeposited(_) => None,
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Current time as Unix epoch milliseconds, read from the client clock.
/// Used to gate time-based UI (e.g. refund availability) on the deposit page.
/// Note: the on-chain program remains the source of truth for actual refund
/// validity; this is advisory only and tolerant of client clock skew.
pub fn now_ms() -> i64 {
    js_sys::Date::now() as i64
}

/// Whether the on-chain refund window is open as of the client clock.
///
/// Mirrors `bethere-escrow::instructions::refund::validate_and_update`:
/// refunds are allowed iff `clock.unix_timestamp >= event_end`. The on-chain
/// check uses seconds; we compare in milliseconds for consistency with
/// `DepositStatusResponse.event_end_ms`.
///
/// Treats `event_end_ms <= 0` (legacy/missing field) as "not yet open" so
/// the refund CTA stays hidden on bad data — fails safe.
pub fn event_refund_window_open(event_end_ms: i64) -> bool {
    event_end_ms > 0 && now_ms() >= event_end_ms
}

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

/// Format remaining seconds into a compact countdown string.
/// Shows days/hours/minutes if > 1 day, hours/minutes/seconds if < 1 day,
/// just minutes/seconds if < 1 hour.
pub fn format_countdown(seconds: i64) -> String {
    if seconds <= 0 {
        return String::new();
    }
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let mins = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if days > 0 {
        format!("{days}d {hours}h {mins}m {secs}s")
    } else if hours > 0 {
        format!("{hours}h {mins}m {secs}s")
    } else {
        format!("{mins}m {secs}s")
    }
}

/// Compute the deposit deadline as epoch milliseconds from registration_date + deadline_hours.
/// Returns None if either field is missing or parsing fails.
pub fn compute_deadline_ms(
    registration_date: &Option<String>,
    deadline_hours: Option<u32>,
) -> Option<f64> {
    let reg_str = registration_date.as_ref()?;
    let hours = deadline_hours?;
    let ms = js_sys::Date::parse(reg_str);
    if ms.is_nan() {
        return None;
    }
    Some(ms + (f64::from(hours) * 3_600_000.0))
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
        | DepositPageState::ThbRejected(d)
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
