//! Deposit-related domain types for dual-track payment (USDC on-chain + THB off-chain).

use serde::{Deserialize, Serialize};

/// Payment method for event deposit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepositMethod {
    /// On-chain USDC via Solana Pay (escrow program).
    Usdc,
    /// Off-chain Thai Baht via PromptPay bank transfer + slip upload.
    Thb,
}

impl std::fmt::Display for DepositMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usdc => write!(f, "usdc"),
            Self::Thb => write!(f, "thb"),
        }
    }
}

/// Deposit status for an attendee (cached in KV).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositStatus {
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// Event ID this deposit belongs to.
    pub event_id: String,
    /// Payment method used.
    pub method: DepositMethod,
    /// Deposit amount in original currency (USDC smallest unit or THB).
    pub amount: u64,
    /// Currency code.
    pub currency: String,
    /// On-chain transaction signature (USDC only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_signature: Option<String>,
    /// Whether the deposit has been verified (USDC: on-chain confirmed, THB: admin verified).
    pub verified: bool,
    /// ISO 8601 timestamp when deposit was recorded.
    pub deposited_at: String,
    /// Attendee's Solana wallet address (USDC deposits only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_address: Option<String>,
}

/// THB deposit record (stored in KV, no on-chain record).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThbDeposit {
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// Event ID this deposit belongs to.
    pub event_id: String,
    /// Deposit amount in Thai Baht.
    pub amount_thb: u64,
    /// R2 URL of the uploaded payment slip image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slip_url: Option<String>,
    /// Whether admin has verified the slip.
    pub verified: bool,
    /// Email of the admin who verified (null if not verified).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_by: Option<String>,
    /// ISO 8601 timestamp of verification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
    /// ISO 8601 timestamp when slip was uploaded.
    pub uploaded_at: String,
    /// Whether THB refund has been processed.
    pub refunded: bool,
    /// ISO 8601 timestamp when refund was marked complete.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refunded_at: Option<String>,
}

/// Request body for POST /api/deposit/usdc — build a Solana Pay deposit TX.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsdcDepositRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// Attendee's Solana wallet address (base58).
    pub wallet_address: String,
}

/// Response for POST /api/deposit/usdc — Solana Pay transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsdcDepositResponse {
    /// Base64-encoded serialized transaction.
    pub transaction: String,
    /// Solana Pay transaction URL for QR code generation.
    pub solana_pay_url: String,
}

/// Response for GET /api/deposit/status/{attendee_id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositStatusResponse {
    /// Whether deposit is enabled for this event.
    pub deposit_enabled: bool,
    /// Deposit amount in USDC (smallest unit).
    pub deposit_amount_usdc: u64,
    /// Deposit amount in THB.
    pub deposit_amount_thb: u64,
    /// PromptPay ID for THB payments (Thai phone number or national ID).
    #[serde(default)]
    pub promptpay_id: String,
    /// Event end time as Unix epoch milliseconds (for refund deadline computation).
    #[serde(default)]
    pub event_end_ms: i64,
    /// Hours after event_end for refund deadline.
    #[serde(default)]
    pub refund_deadline_hours: u32,
    /// Current deposit status (None if not deposited).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<DepositStatus>,
}

/// Request body for POST /api/deposit/thb/verify — admin verifies/rejects a slip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifySlipRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID.
    pub attendee_id: String,
    /// Whether the slip is approved (false = rejected).
    pub approved: bool,
}

/// Response for GET /api/deposit/thb/pending — list of unverified slips.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSlipResponse {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub slips: Vec<ThbDeposit>,
}

/// Response for GET /api/refund/queue — THB refunds pending.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundQueueResponse {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pending: Vec<ThbDeposit>,
}

/// Request body for POST /api/refund/mark/{attendee_id} — mark THB refund as done.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkRefundRequest {
    /// Event ID.
    pub event_id: String,
}
