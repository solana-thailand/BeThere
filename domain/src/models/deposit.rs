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
    /// BLAKE3 of the *decoded image bytes* of the uploaded slip, lowercase hex.
    ///
    /// Anyone can upload any image to the deposit page: nothing about a slip is
    /// checked, so the organizer catches re-used slips by recognising the
    /// person. This is the cheapest thing that makes the system remember
    /// instead — two attendees who upload byte-identical images are now
    /// detectable, and `.issues/129` can build the bank-reference check on top
    /// rather than starting from nothing.
    ///
    /// `None` for every row uploaded before 2026-09-22 and for any slip stored
    /// as an external URL rather than an upload. A missing hash means *not
    /// known*, never *not a duplicate* — a comparison against `None` must never
    /// be read as a clean result.
    ///
    /// Hashed from the decoded bytes, not the data URL text: base64 padding,
    /// MIME-type casing and the `;base64` marker all vary between clients for
    /// the same image, so hashing the string would miss the duplicate it exists
    /// to catch.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub slip_blake3: Option<String>,
    /// The recorded economic source of this deposit, when one has been decided
    /// outright (migration 0047).
    ///
    /// `None` means *not recorded*, and [`ThbDeposit::source`] then falls back
    /// to sniffing the legacy sentinels out of `verified_by` / `slip_url`. That
    /// fallback is not deprecated scaffolding: those sentinels are still what
    /// `register::signup::record_staff_comp` and the rolling-credit application
    /// write, and they carry other meanings besides. One classifier, two
    /// inputs — never two classifiers.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub deposit_source: Option<DepositSource>,
}

/// The economic source of a THB deposit — the single classification every
/// refund / hold / roll rule keys on. Replaces scattered `slip_url` /
/// `verified_by` sentinel sniffing with one exhaustive, typed decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepositSource {
    /// Real cash (a bank-transfer slip). Cash-refundable; holdable-as-credit.
    Cash,
    /// Covered by the attendee's rolling credit. NOT cash-refundable; rolls back
    /// to the balance on check-in (Model B); exit-to-cash via credit-refund.
    Credit,
    /// Staff / organizer comp (฿0 waive). Never cash, never credit — nothing to
    /// refund, hold, or roll.
    Comp,
}

impl DepositSource {
    /// The stored/wire spelling — the same string the `deposit_source` column
    /// holds, the same string `#[serde(rename_all = "snake_case")]` produces,
    /// and the same string migration 0047's `CHECK` constraint allows.
    ///
    /// Three places already agreed on these spellings by coincidence; this makes
    /// it one place. `as_str_matches_the_serde_representation` below is what
    /// stops them drifting apart again.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cash => "cash",
            Self::Credit => "credit",
            Self::Comp => "comp",
        }
    }
}

impl ThbDeposit {
    /// Classify this deposit's economic source — the one place the
    /// credit/comp/cash decision is made.
    ///
    /// An explicitly recorded `deposit_source` wins. That is the whole point of
    /// the column: before it existed, the only way to mark a real ฿500 slip as
    /// non-refundable was to destroy the evidence — blank `slip_url` or zero
    /// `amount_thb` — so an organizer who knew somebody had not paid could only
    /// admit them and owe them ฿500, or refuse them entry (`.issues/129` Gap 1).
    ///
    /// Everything else falls through to the legacy sentinels, unchanged. The
    /// backfill in migration 0047 is a transcription of the fallback below, in
    /// this order, so no existing row changes classification on migration day.
    pub fn source(&self) -> DepositSource {
        if let Some(recorded) = self.deposit_source {
            return recorded;
        }
        if matches!(self.verified_by.as_deref(), Some("SYSTEM_ROLLING_CREDIT"))
            || matches!(
                self.slip_url.as_deref(),
                Some("ROLLING_CREDIT_AUTO_APPLIED")
            )
        {
            DepositSource::Credit
        } else if matches!(self.verified_by.as_deref(), Some("SYSTEM_STAFF_WAIVE"))
            || matches!(self.slip_url.as_deref(), Some("STAFF_COMP_WAIVED"))
            || self.amount_thb == 0
        {
            DepositSource::Comp
        } else {
            DepositSource::Cash
        }
    }

    /// NOT backed by real cash (credit application or staff comp): must never be
    /// cash-refunded or re-held-as-credit (would pay out / mint money never
    /// deposited).
    pub fn is_non_cash(&self) -> bool {
        !matches!(self.source(), DepositSource::Cash)
    }

    /// Specifically a ROLLING-CREDIT-covered deposit (not a staff comp). Model B:
    /// on check-in these roll the ฿ back to the attendee's balance; a no-show
    /// forfeits it.
    pub fn is_credit_covered(&self) -> bool {
        matches!(self.source(), DepositSource::Credit)
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
    /// Slip fingerprints that appear on more than one attendee **in this
    /// event** — i.e. the same image, submitted by two different people.
    ///
    /// Computed over every deposit for the event, not just the pending ones, so
    /// a slip that collides with an already-approved deposit is still flagged.
    /// The admin screen matches a row's `slip_blake3` against this list;
    /// carrying the set once rather than a boolean per row keeps `ThbDeposit`
    /// a record of what was stored rather than a view model.
    ///
    /// Empty is the normal case and also the honest answer when no slip has a
    /// hash yet (everything uploaded before 2026-09-22) — absence of evidence.
    #[serde(default)]
    pub duplicate_slip_hashes: Vec<String>,
}

/// Response for GET /api/refund/queue — THB refunds pending.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundQueueResponse {
    #[serde(default)]
    pub pending: Vec<ThbDeposit>,
    /// Who each pending row belongs to, keyed by attendee id, so the queue
    /// can be narrowed to the people actually owed money back after a
    /// postponement (moved online, or answered "can't come"). Best-effort:
    /// empty when D1 is unavailable, and a row missing here is shown under
    /// every filter except the narrowing ones.
    #[serde(default)]
    pub context: std::collections::HashMap<String, RefundQueueContext>,
}

/// One refund-queue attendee's participation, check-in and answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RefundQueueContext {
    pub participation_type: crate::models::attendee::ParticipationType,
    pub checked_in: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attendance_answer: Option<crate::models::attendee::AttendanceAnswer>,
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

    /// `as_str` is used for the D1 `deposit_source` column, the admin roster's
    /// wire field and migration 0047's CHECK. If it ever disagreed with the
    /// serde representation, a deposit would round-trip through JSON as one
    /// source and through SQL as another.
    #[test]
    fn as_str_matches_the_serde_representation() {
        for source in [
            DepositSource::Cash,
            DepositSource::Credit,
            DepositSource::Comp,
        ] {
            let json = serde_json::to_string(&source).expect("serializes");
            assert_eq!(
                json.trim_matches('"'),
                source.as_str(),
                "as_str and serde disagree for {source:?}"
            );
        }
    }

    // ── DepositSource: the recorded column vs the legacy sentinels ──────────
    //
    // `.issues/129` Gap 1. Before migration 0047 the only way to mark a real
    // ฿500 slip as non-refundable was to destroy the evidence — blank
    // `slip_url` or zero `amount_thb` — so an organizer who knew somebody had
    // not paid could only admit them and owe the money, or turn them away.

    fn thb(amount: u64, slip: Option<&str>, verified_by: Option<&str>) -> ThbDeposit {
        ThbDeposit {
            attendee_id: "a".into(),
            event_id: "e".into(),
            amount_thb: amount,
            slip_url: slip.map(str::to_string),
            verified: false,
            verified_by: verified_by.map(str::to_string),
            verified_at: None,
            uploaded_at: "2026-09-22T00:00:00Z".into(),
            refunded: false,
            refunded_at: None,
            held_as_credit: false,
            held_as_credit_at: None,
            attendee_name: None,
            bank_account: None,
            bank_name: None,
            account_name: None,
            refund_proof_url: None,
            slip_blake3: None,
            deposit_source: None,
        }
    }

    /// The fallback, unchanged. Migration 0047's backfill is a transcription of
    /// exactly these arms, in this order, so no row changed classification on
    /// migration day.
    #[test]
    fn the_legacy_sentinels_still_classify_every_row_that_has_no_recorded_source() {
        assert_eq!(
            thb(500, Some("/api/storage/slips/e/a"), Some("admin@x")).source(),
            DepositSource::Cash
        );
        assert_eq!(
            thb(500, Some("/s"), Some("SYSTEM_ROLLING_CREDIT")).source(),
            DepositSource::Credit
        );
        assert_eq!(
            thb(500, Some("ROLLING_CREDIT_AUTO_APPLIED"), Some("admin@x")).source(),
            DepositSource::Credit
        );
        assert_eq!(
            thb(0, Some("/s"), Some("SYSTEM_STAFF_WAIVE")).source(),
            DepositSource::Comp
        );
        assert_eq!(
            thb(500, Some("STAFF_COMP_WAIVED"), Some("admin@x")).source(),
            DepositSource::Comp
        );
        assert_eq!(
            thb(0, Some("/api/storage/slips/e/a"), Some("admin@x")).source(),
            DepositSource::Comp
        );
    }

    /// Order is load-bearing, and the SQL backfill has to reproduce it: a row
    /// that is BOTH credit-applied and ฿0 is Credit, not Comp. Getting this
    /// backwards would have moved rolling-credit deposits into the comp bucket,
    /// which is money the attendee is still owed.
    #[test]
    fn credit_beats_comp_when_a_row_matches_both() {
        let both = thb(
            0,
            Some("ROLLING_CREDIT_AUTO_APPLIED"),
            Some("SYSTEM_ROLLING_CREDIT"),
        );
        assert_eq!(both.source(), DepositSource::Credit);
        assert!(both.is_credit_covered());
    }

    /// The point of the column: a real ฿500 slip, with its evidence intact,
    /// classified as a comp because an organizer said so.
    #[test]
    fn a_recorded_source_overrides_the_sentinels_without_destroying_evidence() {
        let mut d = thb(500, Some("/api/storage/slips/e/a"), Some("admin@x"));
        assert_eq!(d.source(), DepositSource::Cash);

        d.deposit_source = Some(DepositSource::Comp);
        assert_eq!(d.source(), DepositSource::Comp);
        assert!(d.is_non_cash(), "a comp must never reach the refund queue");
        assert!(!d.is_credit_covered(), "a comp is not rolling credit");
        // The evidence of what was claimed survives the write-off. That is the
        // whole reason the column exists.
        assert_eq!(d.amount_thb, 500);
        assert_eq!(d.slip_url.as_deref(), Some("/api/storage/slips/e/a"));
    }

    /// The override works in the other direction too — a ฿0 row an organizer
    /// explicitly records as cash is cash. Guards against the column being
    /// read only when it agrees with the sentinels, which would make it
    /// decorative.
    #[test]
    fn the_recorded_source_wins_even_against_a_sentinel_that_disagrees() {
        let mut zero = thb(0, Some("/api/storage/slips/e/a"), Some("admin@x"));
        assert_eq!(zero.source(), DepositSource::Comp);
        zero.deposit_source = Some(DepositSource::Cash);
        assert_eq!(zero.source(), DepositSource::Cash);

        let mut waived = thb(500, Some("STAFF_COMP_WAIVED"), Some("admin@x"));
        assert_eq!(waived.source(), DepositSource::Comp);
        waived.deposit_source = Some(DepositSource::Credit);
        assert_eq!(waived.source(), DepositSource::Credit);
    }
}
