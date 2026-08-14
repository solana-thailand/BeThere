//! Deposit-related domain types for dual-track payment (USDC on-chain + THB off-chain).

use std::str::FromStr;

use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// Payment method for event deposit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepositMethod {
    /// On-chain USDC via Solana Pay (escrow program).
    Usdc,
    /// Off-chain Thai Baht via PromptPay bank transfer + slip upload.
    Thb,
    /// THB credit from a held/rolling deposit.
    CreditThb,
    /// USDC credit from a held/rolling deposit.
    CreditUsdc,
}

impl std::fmt::Display for DepositMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usdc => write!(f, "usdc"),
            Self::Thb => write!(f, "thb"),
            Self::CreditThb => write!(f, "credit_thb"),
            Self::CreditUsdc => write!(f, "credit_usdc"),
        }
    }
}

// SSOT for string → enum parsing. Inverse of `Display`. Eliminates
// cross-crate duplication: `worker` previously hand-mapped these strings in
// `db/deposit_statuses.rs` (Plan 014 Phase 2.2 R2). The error format
// `unknown DepositMethod: '{other}'` matches the prior worker-side message
// exactly so error consumers (logs, e2e scripts) see no behavior change.
impl FromStr for DepositMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "usdc" => Ok(Self::Usdc),
            "thb" => Ok(Self::Thb),
            "credit_thb" => Ok(Self::CreditThb),
            "credit_usdc" => Ok(Self::CreditUsdc),
            other => Err(format!("unknown DepositMethod: '{other}'")),
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
    /// Deposit order within this event (1-based, assigned on deposit creation).
    #[serde(default)]
    pub deposit_order: u32,
    /// Whether this deposit is in the refundable tier (order <= max_refundable_deposits).
    #[serde(default = "default_true")]
    pub refundable: bool,
    /// Whether the deposit was explicitly rejected by admin (THB slips only).
    /// When false + verified false, the slip is still pending review.
    #[serde(default)]
    pub rejected: bool,
}

impl DepositStatus {
    /// Is this deposit within the refundable tier?
    /// Compares deposit_order against the max_refundable limit.
    /// A max_refundable of 0 means all deposits are refundable (unlimited).
    pub fn is_refundable_tier(&self, max_refundable: u32) -> bool {
        max_refundable == 0 || self.deposit_order <= max_refundable
    }

    /// Is the deposit past the refund deadline?
    /// Deadline = event_end_ms + deadline_hours * 3600_000 ms.
    pub fn is_past_deadline(&self, event_end_ms: i64, deadline_hours: u32, now_ms: i64) -> bool {
        let deadline = event_end_ms + (deadline_hours as i64 * 3_600_000);
        now_ms > deadline
    }
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
    /// Whether the attendee chose to hold this deposit as rolling credit for
    /// future events instead of claiming a refund. Distinct from `refunded` —
    /// held-as-credit retains funds as organizer liability (credit the attendee
    /// spends later), whereas `refunded` releases funds back to the attendee.
    /// Idempotency flag for `POST /api/deposit/hold` (prevents double-credit).
    #[serde(default)]
    pub held_as_credit: bool,
    /// ISO 8601 timestamp when the deposit was held as rolling credit.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub held_as_credit_at: Option<String>,
    /// Attendee display name (enriched from Google Sheets, not stored in KV).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attendee_name: Option<String>,
    /// Bank account number for THB refund.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_account: Option<String>,
    /// Bank name for THB refund.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_name: Option<String>,
    /// Account holder name for THB refund.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_name: Option<String>,
    /// R2 URL of the refund transfer receipt (uploaded by admin when marking refund).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub refund_proof_url: Option<String>,
}

impl ThbDeposit {
    /// True when this deposit is NOT backed by real cash — a rolling-credit
    /// application (`SYSTEM_ROLLING_CREDIT` / `ROLLING_CREDIT_AUTO_APPLIED`) or a
    /// staff comp (`SYSTEM_STAFF_WAIVE` / `STAFF_COMP_WAIVED` / ฿0). Such a deposit
    /// must never be cash-refunded or re-held-as-credit: doing so would pay out —
    /// or mint credit from — money that was never deposited.
    pub fn is_non_cash(&self) -> bool {
        matches!(
            self.verified_by.as_deref(),
            Some("SYSTEM_ROLLING_CREDIT" | "SYSTEM_STAFF_WAIVE")
        ) || matches!(
            self.slip_url.as_deref(),
            Some("ROLLING_CREDIT_AUTO_APPLIED" | "STAFF_COMP_WAIVED")
        ) || self.amount_thb == 0
    }
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
    /// Event start time as Unix epoch milliseconds.
    #[serde(default)]
    pub event_start_ms: i64,
    /// Event end time as Unix epoch milliseconds (for refund deadline computation).
    #[serde(default)]
    pub event_end_ms: i64,
    /// Hours after event_end for refund deadline.
    #[serde(default)]
    pub refund_deadline_hours: u32,
    /// Absolute refund deadline as Unix epoch milliseconds
    /// (= `event_end_ms + refund_deadline_hours * 3_600_000`).
    /// Precomputed by the worker so the frontend gate can evaluate the
    /// no-show path (`now < refund_deadline_ms`) without recomputing.
    /// `0` when not configured (legacy/missing data).
    #[serde(default)]
    pub refund_deadline_ms: i64,
    /// Whether the attendee has checked in (off-chain source of truth:
    /// Google Sheets / D1). Drives the two-path refund window on the
    /// frontend: checked-in attendees may refund anytime after `event_end`;
    /// no-shows may only refund before `refund_deadline_ms`.
    #[serde(default)]
    pub checked_in: bool,
    /// Event name for context display on the deposit page.
    #[serde(default)]
    pub event_name: String,
    /// Event tagline (short description).
    #[serde(default)]
    pub event_tagline: String,
    /// Event slug for navigation back to `/e/:slug`.
    #[serde(default)]
    pub event_slug: String,
    /// Current deposit status (None if not deposited).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<DepositStatus>,
    /// Whether the backend is running in dev mode.
    /// When false, Solana wallet payment options are hidden from the UI.
    #[serde(default)]
    pub dev_mode: bool,
    /// Deposit deadline in hours after registration. None = no deadline.
    #[serde(default)]
    pub deposit_deadline_hours: Option<u32>,
    /// Whether the deposit deadline has expired (no deposit received in time).
    /// When true, the attendee's participation_type has been auto-switched to "Online".
    #[serde(default)]
    pub deadline_expired: bool,
    /// Registration timestamp (ISO 8601) from the Google Sheet.
    /// Used by the frontend to compute remaining time for the countdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registration_date: Option<String>,
    /// Whether in-person capacity is still available (for reclaim flow).
    /// None = no deadline configured or not applicable.
    /// Some(true) = spots available, attendee can reclaim.
    /// Some(false) = capacity full, cannot reclaim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_person_available: Option<bool>,
    /// Whether USDC (on-chain escrow) deposits are currently accepted.
    /// `true` only when escrow_status is `Initialized`.
    /// Frontend uses this to hide the USDC payment option when escrow is closed/deactivated.
    #[serde(default)]
    pub usdc_deposits_accepted: bool,
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
    #[serde(default)]
    pub slips: Vec<ThbDeposit>,
}

/// Response for GET /api/refund/queue — THB refunds pending.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundQueueResponse {
    #[serde(default)]
    pub pending: Vec<ThbDeposit>,
}

/// Request body for POST /api/refund/mark/{attendee_id} — mark THB refund as done.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkRefundRequest {
    /// Event ID.
    pub event_id: String,
    /// R2 URL of the refund transfer receipt.
    pub refund_proof_url: String,
}

/// Request body for POST /api/refund/manual/{attendee_id} — set refund status for attendees without deposit (e.g., VIP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualRefundRequest {
    /// Event ID.
    pub event_id: String,
    /// Refund status string (e.g., "refunded", "pending", "not_applicable").
    pub refund_status: String,
    /// Optional refund link filled in by organizer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_link: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_deposit(order: u32) -> DepositStatus {
        DepositStatus {
            attendee_id: "gst-test".to_string(),
            event_id: "evt-test".to_string(),
            method: DepositMethod::Usdc,
            amount: 15_000_000,
            currency: "USDC".to_string(),
            tx_signature: Some("sig123".to_string()),
            verified: true,
            deposited_at: "2025-01-01T00:00:00Z".to_string(),
            wallet_address: Some("wallet123".to_string()),
            deposit_order: order,
            refundable: true,
            rejected: false,
        }
    }

    // ── is_refundable_tier ──────────────────────────────────────────

    #[test]
    fn test_refundable_tier_unlimited_when_max_zero() {
        let deposit = make_deposit(999);
        assert!(deposit.is_refundable_tier(0));
    }

    #[test]
    fn test_refundable_tier_within_limit() {
        let deposit = make_deposit(5);
        assert!(deposit.is_refundable_tier(10));
    }

    #[test]
    fn test_refundable_tier_at_limit() {
        let deposit = make_deposit(10);
        assert!(deposit.is_refundable_tier(10));
    }

    #[test]
    fn test_refundable_tier_over_limit() {
        let deposit = make_deposit(11);
        assert!(!deposit.is_refundable_tier(10));
    }

    #[test]
    fn test_refundable_tier_order_one() {
        let deposit = make_deposit(1);
        assert!(deposit.is_refundable_tier(5));
    }

    // ── is_past_deadline ────────────────────────────────────────────

    #[test]
    fn test_not_past_deadline_within_window() {
        let deposit = make_deposit(1);
        let event_end_ms = 2_000_001_000_000_i64;
        let deadline_hours = 168; // 7 days
        let deadline_ms = event_end_ms + (168_i64 * 3_600_000);
        assert!(!deposit.is_past_deadline(event_end_ms, deadline_hours, deadline_ms));
    }

    #[test]
    fn test_past_deadline_over() {
        let deposit = make_deposit(1);
        let event_end_ms = 2_000_001_000_000_i64;
        let deadline_hours = 168;
        let deadline_ms = event_end_ms + (168_i64 * 3_600_000);
        assert!(deposit.is_past_deadline(event_end_ms, deadline_hours, deadline_ms + 1));
    }

    #[test]
    fn test_not_past_deadline_well_before() {
        let deposit = make_deposit(1);
        let event_end_ms = 2_000_001_000_000_i64;
        let now_ms = 2_000_002_000_000_i64;
        assert!(!deposit.is_past_deadline(event_end_ms, 168, now_ms));
    }

    #[test]
    fn test_deadline_zero_hours() {
        let deposit = make_deposit(1);
        let event_end_ms = 1_000_000_i64;
        // 0 hours → deadline == event_end_ms
        assert!(!deposit.is_past_deadline(event_end_ms, 0, event_end_ms));
        assert!(deposit.is_past_deadline(event_end_ms, 0, event_end_ms + 1));
    }

    // ── FromStr / Display round-trip (Plan 014 Phase 2.2 R2) ────────
    //
    // The worker previously hand-mapped these strings in
    // `db/deposit_statuses.rs` and `handlers/attendee.rs`. The domain
    // `FromStr`/`Display` impls are now the SSOT; these tests pin the
    // exact wire strings so a future change cannot drift silently.

    #[test]
    fn test_deposit_method_from_str_round_trip() {
        // Display → FromStr → identity for every variant.
        for original in [
            DepositMethod::Usdc,
            DepositMethod::Thb,
            DepositMethod::CreditThb,
            DepositMethod::CreditUsdc,
        ] {
            let s = original.to_string();
            let parsed: DepositMethod = s.parse().expect("round-trip should succeed");
            assert_eq!(
                parsed, original,
                "Display/FromStr round-trip broke for {original:?}"
            );
        }
    }

    #[test]
    fn test_deposit_method_from_str_wire_strings() {
        // Pin the exact snake_case wire strings emitted by serde
        // (`rename_all = "snake_case"`) and accepted by `FromStr`. If
        // either drifts, downstream workers, e2e scripts, and the D1
        // `method` column all break.
        assert_eq!(
            "usdc".parse::<DepositMethod>().unwrap(),
            DepositMethod::Usdc
        );
        assert_eq!("thb".parse::<DepositMethod>().unwrap(), DepositMethod::Thb);
        assert_eq!(
            "credit_thb".parse::<DepositMethod>().unwrap(),
            DepositMethod::CreditThb
        );
        assert_eq!(
            "credit_usdc".parse::<DepositMethod>().unwrap(),
            DepositMethod::CreditUsdc
        );

        // Display output must match exactly (this is what the attendee
        // handler now emits as the JSON `"method"` field via to_string()).
        assert_eq!(DepositMethod::Usdc.to_string(), "usdc");
        assert_eq!(DepositMethod::Thb.to_string(), "thb");
        assert_eq!(DepositMethod::CreditThb.to_string(), "credit_thb");
        assert_eq!(DepositMethod::CreditUsdc.to_string(), "credit_usdc");
    }

    #[test]
    fn test_deposit_method_from_str_rejects_unknown_with_canonical_message() {
        // Worker `db/deposit_statuses.rs` now propagates the FromStr error
        // directly via `?`. The error format MUST stay
        // `unknown DepositMethod: '{other}'` so logs, e2e scripts, and any
        // error-display code see no behavior change. Pin the exact message.
        let err = "bitcoin".parse::<DepositMethod>().unwrap_err();
        assert_eq!(err, "unknown DepositMethod: 'bitcoin'");

        // Empty string and PascalCase are also rejected (serde rejects
        // PascalCase too — see frontend-leptos/tests/serde_contract.rs).
        let err = "".parse::<DepositMethod>().unwrap_err();
        assert_eq!(err, "unknown DepositMethod: ''");
        let err = "Usdc".parse::<DepositMethod>().unwrap_err();
        assert_eq!(err, "unknown DepositMethod: 'Usdc'");
    }
}
