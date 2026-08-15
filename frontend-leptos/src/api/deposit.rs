//! Deposit/refund types and API functions, including escrow operations.

// serde derives used via full path in attribute macros

use super::types::ApiError;
use super::{api_get_json, api_post_json};

// ===== Deposit/Refund Types =====

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DepositStatusInfo {
    pub attendee_id: String,
    pub event_id: String,
    pub method: super::types::DepositMethod,
    pub amount: u64,
    pub currency: String,
    pub tx_signature: Option<String>,
    pub verified: bool,
    pub deposited_at: String,
    #[serde(default)]
    pub wallet_address: Option<String>,
    /// Deposit order within this event (1-based).
    #[serde(default)]
    pub deposit_order: u32,
    /// Whether this deposit is in the refundable tier.
    #[serde(default = "super::types::default_true")]
    pub refundable: bool,
    /// Whether the deposit was explicitly rejected by admin (THB slips).
    #[serde(default)]
    pub rejected: bool,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct DepositStatusResponse {
    pub deposit_enabled: bool,
    pub deposit_amount_usdc: u64,
    pub deposit_amount_thb: u64,
    pub promptpay_id: String,
    #[serde(default)]
    pub event_start_ms: i64,
    #[serde(default)]
    pub event_end_ms: i64,
    #[serde(default)]
    pub refund_deadline_hours: u32,
    /// Absolute refund deadline as Unix epoch milliseconds
    /// (= `event_end_ms + refund_deadline_hours * 3_600_000`).
    /// Precomputed by the worker so the gate can evaluate the no-show path
    /// without recomputing. `0` when not configured (legacy/missing data).
    #[serde(default)]
    pub refund_deadline_ms: i64,
    /// Whether the attendee has checked in (off-chain source of truth).
    /// Drives the two-path refund window: checked-in attendees may refund
    /// anytime after `event_end`; no-shows may only refund before
    /// `refund_deadline_ms`.
    #[serde(default)]
    pub checked_in: bool,
    #[serde(default)]
    pub event_name: String,
    #[serde(default)]
    pub event_tagline: String,
    /// Event slug for navigation back to `/e/:slug`.
    #[serde(default)]
    pub event_slug: String,
    pub status: Option<DepositStatusInfo>,
    /// Whether the backend is in dev mode (shows Solana wallet options).
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
    #[serde(default)]
    pub registration_date: Option<String>,
    /// Whether in-person capacity is still available (for reclaim flow).
    #[serde(default)]
    pub in_person_available: Option<bool>,
    /// Whether USDC (on-chain escrow) deposits are currently accepted.
    /// `true` only when escrow_status is `Initialized`.
    #[serde(default)]
    pub usdc_deposits_accepted: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UsdcDepositRequest {
    pub event_id: String,
    pub attendee_id: String,
    pub wallet_address: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct UsdcDepositResponse {
    pub transaction: String,
    pub solana_pay_url: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ThbSlipUploadRequest {
    pub event_id: String,
    pub attendee_id: String,
    pub slip_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VerifySlipRequest {
    pub event_id: String,
    pub attendee_id: String,
    pub approved: bool,
}

/// Request body for `POST /api/deposit/thb/admin-upload`.
///
/// Mirrors `ThbSlipUploadRequest` plus an `auto_verify` flag (default `true` —
/// admin recording a confirmed payment typically also verifies it in the same
/// call). Used when an attendee cannot upload themselves (JWT expired, browser
/// bug, slip sent via LINE/email). Staff-authed + audited server-side.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AdminSlipUploadRequest {
    pub event_id: String,
    pub attendee_id: String,
    pub slip_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_name: Option<String>,
    /// When true (default), also marks the deposit as verified in the same call.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub auto_verify: bool,
}

impl Default for AdminSlipUploadRequest {
    fn default() -> Self {
        Self {
            event_id: String::new(),
            attendee_id: String::new(),
            slip_url: String::new(),
            bank_account: None,
            bank_name: None,
            account_name: None,
            auto_verify: true,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ThbDepositInfo {
    pub attendee_id: String,
    pub event_id: String,
    pub amount_thb: u64,
    pub slip_url: Option<String>,
    pub verified: bool,
    pub verified_by: Option<String>,
    pub verified_at: Option<String>,
    pub uploaded_at: String,
    pub refunded: bool,
    pub refunded_at: Option<String>,
    /// Idempotency flag — deposit was held as rolling credit (sibling of
    /// `refunded`). Mutually exclusive with `refunded`. Surfaced by the
    /// `/refund/held` admin endpoint (Issue #061 Phase 2).
    #[serde(default)]
    pub held_as_credit: bool,
    #[serde(default)]
    pub held_as_credit_at: Option<String>,
    #[serde(default)]
    pub attendee_name: Option<String>,
    #[serde(default)]
    pub bank_account: Option<String>,
    #[serde(default)]
    pub bank_name: Option<String>,
    #[serde(default)]
    pub account_name: Option<String>,
    #[serde(default)]
    pub refund_proof_url: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PendingSlipResponse {
    #[serde(default)]
    pub slips: Vec<ThbDepositInfo>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct RefundQueueResponse {
    pub pending: Vec<ThbDepositInfo>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MarkRefundRequest {
    pub event_id: String,
    pub refund_proof_url: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ConfirmDepositResponse {
    pub confirmed: bool,
    pub tx_signature: Option<String>,
    pub solana_pay_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Escrow Refund
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/refund.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RefundTxRequest {
    pub event_id: String,
    pub attendee_id: String,
    pub wallet_address: String,
}

/// Response from POST /api/escrow/refund.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct RefundTxResponse {
    pub transaction: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Escrow: Mark Checked In (admin)
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/mark-checked-in.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MarkCheckedInRequest {
    pub event_id: String,
    /// Attendee API ID — backend resolves wallet from deposit record.
    pub attendee_id: String,
}

/// Response from POST /api/escrow/mark-checked-in.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct MarkCheckedInResponse {
    pub transaction: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Escrow: Deactivate Event (admin)
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/deactivate-event.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeactivateEventRequest {
    pub event_id: String,
}

/// Response from POST /api/escrow/deactivate-event.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct DeactivateEventResponse {
    pub transaction: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Escrow: Close Event (admin)
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/close-event.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CloseEventRequest {
    pub event_id: String,
}

/// Response from POST /api/escrow/close-event.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CloseEventResponse {
    pub transaction: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Escrow: Close Deposit (attendee reclaims rent)
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/close-deposit.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CloseDepositRequest {
    pub event_id: String,
    pub attendee_id: String,
    pub wallet_address: String,
}

/// Response from POST /api/escrow/close-deposit.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CloseDepositResponse {
    pub transaction: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Escrow: Claim Forfeited (admin)
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/claim-forfeited.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClaimForfeitedRequest {
    pub event_id: String,
}

/// Response from POST /api/escrow/claim-forfeited.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ClaimForfeitedResponse {
    pub transaction: String,
    pub message: String,
}

// ===== Deposit API =====

/// GET /api/deposit/status/{attendee_id}?event_id=xxx
pub async fn get_deposit_status(
    attendee_id: &str,
    event_id: Option<&str>,
) -> Result<DepositStatusResponse, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/deposit/status/{attendee_id}?event_id={eid}"),
        _ => format!("/deposit/status/{attendee_id}"),
    };
    api_get_json(&path).await
}

/// POST /api/deposit/usdc
pub async fn deposit_usdc(body: &UsdcDepositRequest) -> Result<UsdcDepositResponse, ApiError> {
    api_post_json("/deposit/usdc", body).await
}

/// POST /api/deposit/usdc/webhook — record TX signature
pub async fn record_deposit_tx(
    event_id: &str,
    attendee_id: &str,
    tx_signature: &str,
) -> Result<serde_json::Value, ApiError> {
    let body = serde_json::json!({
        "event_id": event_id,
        "attendee_id": attendee_id,
        "tx_signature": tx_signature,
    });
    api_post_json("/deposit/usdc/webhook", &body).await
}

/// GET /api/deposit/usdc/confirm?event_id=xxx&attendee_id=xxx
pub async fn confirm_deposit(
    event_id: &str,
    attendee_id: &str,
) -> Result<ConfirmDepositResponse, ApiError> {
    let path = format!("/deposit/usdc/confirm?event_id={event_id}&attendee_id={attendee_id}");
    api_get_json(&path).await
}

/// POST /api/deposit/thb/upload
pub async fn upload_thb_slip(body: &ThbSlipUploadRequest) -> Result<serde_json::Value, ApiError> {
    api_post_json("/deposit/thb/upload", body).await
}

/// POST /api/deposit/thb/admin-upload (admin).
///
/// Records a THB slip on behalf of an attendee who cannot upload themselves.
/// Staff-authed + audited server-side; skips the attendee email-match gate.
/// When `auto_verify` is true (default), the deposit is also verified in the
/// same call (QR generated, sheet mirrored, D1 dual-written).
pub async fn admin_upload_thb_slip(
    body: &AdminSlipUploadRequest,
) -> Result<serde_json::Value, ApiError> {
    api_post_json("/deposit/thb/admin-upload", body).await
}

/// POST /api/deposit/thb/verify (admin)
pub async fn verify_thb_slip(body: &VerifySlipRequest) -> Result<serde_json::Value, ApiError> {
    api_post_json("/deposit/thb/verify", body).await
}

/// GET /api/deposit/thb/pending?event_id=xxx
pub async fn get_pending_slips(event_id: Option<&str>) -> Result<PendingSlipResponse, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/deposit/thb/pending?event_id={eid}"),
        _ => "/deposit/thb/pending".to_string(),
    };
    api_get_json(&path).await
}

/// GET /api/refund/queue?event_id=xxx
pub async fn get_refund_queue(event_id: Option<&str>) -> Result<RefundQueueResponse, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/refund/queue?event_id={eid}"),
        _ => "/refund/queue".to_string(),
    };
    api_get_json(&path).await
}

/// POST /api/refund/mark/{attendee_id}
pub async fn mark_refund(
    attendee_id: &str,
    body: &MarkRefundRequest,
) -> Result<serde_json::Value, ApiError> {
    let path = format!("/refund/mark/{attendee_id}");
    api_post_json(&path, body).await
}

/// Request body for POST /api/refund/manual/{attendee_id} — set refund status without deposit.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ManualRefundRequest {
    pub event_id: String,
    pub refund_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_link: Option<String>,
}

/// POST /api/refund/manual/{attendee_id}
pub async fn mark_manual_refund(
    attendee_id: &str,
    body: &ManualRefundRequest,
) -> Result<serde_json::Value, ApiError> {
    let path = format!("/refund/manual/{attendee_id}");
    api_post_json(&path, body).await
}

/// Response for GET /api/refund/refunded
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct RefundedListResponse {
    #[serde(default)]
    pub refunded: Vec<ThbDepositInfo>,
}

/// GET /api/refund/refunded?event_id=xxx
pub async fn get_refunded_list(
    event_id: Option<&str>,
) -> Result<RefundedListResponse, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/refund/refunded?event_id={eid}"),
        _ => "/refund/refunded".to_string(),
    };
    api_get_json(&path).await
}

/// Response for GET /api/refund/held — deposits held as rolling credit.
/// Mirrors `RefundedListResponse` (sibling terminal state).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct HeldListResponse {
    #[serde(default)]
    pub held: Vec<ThbDepositInfo>,
}

/// GET /api/refund/held?event_id=xxx — list THB deposits held as rolling credit.
pub async fn get_held_list(event_id: Option<&str>) -> Result<HeldListResponse, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/refund/held?event_id={eid}"),
        _ => "/refund/held".to_string(),
    };
    api_get_json(&path).await
}

/// Request body for POST /api/refund/hold/{attendee_id} — admin marks a
/// verified THB deposit as held-as-rolling-credit on behalf of an attendee.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AdminHoldRequest {
    pub event_id: String,
}

/// POST /api/refund/hold/{attendee_id} — admin hold deposit as rolling credit.
pub async fn admin_hold_deposit(
    attendee_id: &str,
    body: &AdminHoldRequest,
) -> Result<serde_json::Value, ApiError> {
    let path = format!("/refund/hold/{attendee_id}");
    api_post_json(&path, body).await
}

/// Request body for POST /api/deposit/apply-credit/{attendee_id} — admin applies
/// an attendee's rolling deposit credit to complete a registration stuck at the
/// deposit step (credit-holder who never uploaded a slip).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplyCreditRequest {
    pub event_id: String,
}

/// POST /api/deposit/apply-credit/{attendee_id} — spends the attendee's rolling
/// credit (only if it covers the event deposit) and writes a credit-covered
/// deposit so they proceed to the ticket. Backend enforces sufficiency.
pub async fn apply_credit(
    attendee_id: &str,
    body: &ApplyCreditRequest,
) -> Result<serde_json::Value, ApiError> {
    let path = format!("/deposit/apply-credit/{attendee_id}");
    api_post_json(&path, body).await
}

/// Response for GET /api/deposit/credit-liability — the organizer's total
/// deposit-credit liability across all contacts (Issue #061 Phase 2 option a2).
/// Backing data for the "Total credit held: X THB across N contacts" header chip.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CreditLiability {
    #[serde(default)]
    pub total_thb: i64,
    #[serde(default)]
    pub total_usdc: i64,
    #[serde(default)]
    pub contact_count: i64,
}

/// GET /api/deposit/credit-liability — total credit held across all contacts.
pub async fn get_credit_liability() -> Result<CreditLiability, ApiError> {
    api_get_json("/deposit/credit-liability").await
}

/// Cash / Credit / Comp deposit-source breakdown for one event — reconciles how
/// attendees got in (paid cash vs spent rolling credit vs staff comp).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct DepositSourceSummary {
    #[serde(default)]
    pub cash_count: u32,
    #[serde(default)]
    pub cash_thb: u64,
    #[serde(default)]
    pub credit_count: u32,
    #[serde(default)]
    pub credit_thb: u64,
    #[serde(default)]
    pub comp_count: u32,
}

/// Response for GET /api/deposit/credit-used — the event-level money summary plus
/// the list of attendees who got in by spending rolling credit.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CreditUsedResponse {
    #[serde(default)]
    pub summary: DepositSourceSummary,
    #[serde(default)]
    pub credit_used: Vec<ThbDepositInfo>,
}

/// GET /api/deposit/credit-used?event_id= — Cash/Credit/Comp summary for the event.
pub async fn get_credit_used(event_id: Option<&str>) -> Result<CreditUsedResponse, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/deposit/credit-used?event_id={eid}"),
        _ => "/deposit/credit-used".to_string(),
    };
    api_get_json(&path).await
}

// ===== Phase 3 — Credit Refund Request (exit path) =====

/// Response for POST /api/deposit/request-credit-refund — attendee requests
/// return of their held rolling credit (Issue #061 §D3). The flag is the queue
/// signal only; the organizer processes the actual payout via existing refund
/// tooling.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct RequestCreditRefundResponse {
    #[serde(default)]
    pub requested: bool,
    #[serde(default)]
    pub message: String,
}

/// POST /api/deposit/request-credit-refund — attendee sets the flag on their
/// own contact. No request body — the email comes from the JWT
/// (VULN-012 pattern). Idempotent: re-calls re-stamp the timestamp.
///
/// `api_post_json` is generic + unwraps the `ApiResponse` envelope internally,
/// so a one-liner returns the inner `RequestCreditRefundResponse` directly
/// (matches the `create_campaign` / `put_admin_quiz` convention).
pub async fn request_credit_refund() -> Result<RequestCreditRefundResponse, ApiError> {
    let body = serde_json::json!({});
    api_post_json("/deposit/request-credit-refund", &body).await
}

/// Response for GET /api/deposit/credit-refund-request — the attendee's own
/// flag state. Backs the ticket page's already-requested card state on reload
/// (mirrors the held_as_credit UX pattern).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CreditRefundRequestStatus {
    #[serde(default)]
    pub requested: bool,
}

/// GET /api/deposit/credit-refund-request — read the attendee's own flag state.
pub async fn get_credit_refund_request_status() -> Result<CreditRefundRequestStatus, ApiError> {
    api_get_json("/deposit/credit-refund-request").await
}

/// One row in the admin "credit refund requested" listing (Issue #061 Phase 3).
/// Cross-event — backs the badge on the Held-as-Credit tab. Matches the worker
/// `CreditRefundRequest` struct field-for-field.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CreditRefundRequest {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub credit_thb: i64,
    #[serde(default)]
    pub credit_usdc: i64,
    #[serde(default)]
    pub requested_at: String,
}

/// Response for GET /api/deposit/credit-refund-requests — admin lists contacts
/// with an open credit-refund-request flag.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CreditRefundRequestsResponse {
    #[serde(default)]
    pub requests: Vec<CreditRefundRequest>,
}

/// GET /api/deposit/credit-refund-requests — admin lists open credit-refund
/// requests. Cross-event (global), backs the badge on the Held-as-Credit tab.
pub async fn get_credit_refund_requests() -> Result<CreditRefundRequestsResponse, ApiError> {
    api_get_json("/deposit/credit-refund-requests").await
}

/// Request body for POST /api/deposit/clear-credit-refund-request — admin
/// clears the credit-refund-request flag after processing the payout.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClearCreditRefundRequest {
    pub email: String,
}

/// Response for POST /api/deposit/clear-credit-refund-request (admin).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ClearCreditRefundResponse {
    #[serde(default)]
    pub cleared: bool,
    #[serde(default)]
    pub message: String,
}

/// POST /api/deposit/clear-credit-refund-request — admin clears the flag after
/// processing the payout. Email in body (not path) — emails can contain
/// characters awkward to URL-encode in a path segment.
pub async fn clear_credit_refund_request(
    body: &ClearCreditRefundRequest,
) -> Result<ClearCreditRefundResponse, ApiError> {
    api_post_json("/deposit/clear-credit-refund-request", body).await
}

// ===== Escrow API =====

/// POST /api/escrow/refund — build refund TX
pub async fn build_refund_tx(body: &RefundTxRequest) -> Result<RefundTxResponse, ApiError> {
    api_post_json("/escrow/refund", body).await
}

/// POST /api/escrow/mark-checked-in — mark attendee checked in
pub async fn mark_checked_in(body: &MarkCheckedInRequest) -> Result<MarkCheckedInResponse, ApiError> {
    api_post_json("/escrow/mark-checked-in", body).await
}

/// POST /api/escrow/deactivate-event — build deactivate_event TX
pub async fn deactivate_event(body: &DeactivateEventRequest) -> Result<DeactivateEventResponse, ApiError> {
    api_post_json("/escrow/deactivate-event", body).await
}

/// POST /api/escrow/close-event — build close_event TX
pub async fn close_event(body: &CloseEventRequest) -> Result<CloseEventResponse, ApiError> {
    api_post_json("/escrow/close-event", body).await
}

/// POST /api/escrow/close-deposit — build close_deposit TX
pub async fn close_deposit(body: &CloseDepositRequest) -> Result<CloseDepositResponse, ApiError> {
    api_post_json("/escrow/close-deposit", body).await
}

/// POST /api/escrow/claim-forfeited — build claim_forfeited TX
pub async fn claim_forfeited(body: &ClaimForfeitedRequest) -> Result<ClaimForfeitedResponse, ApiError> {
    api_post_json("/escrow/claim-forfeited", body).await
}

// ===== Rollover API =====

/// Request body for rollover deposit transaction.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RolloverDepositRequest {
    /// Source event ID (past event with checked-in deposit).
    pub source_event_id: String,
    /// Target event ID (new event to roll deposit into).
    pub target_event_id: String,
    /// Attendee API ID.
    pub attendee_id: String,
    /// Attendee's wallet address (base58).
    pub wallet_address: String,
}

/// Response for rollover deposit transaction.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct RolloverDepositResponse {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction: String,
    /// Human-readable message.
    pub message: String,
}

/// POST /api/escrow/rollover-deposit — build rollover_deposit TX
pub async fn rollover_deposit(body: &RolloverDepositRequest) -> Result<RolloverDepositResponse, ApiError> {
    api_post_json("/escrow/rollover-deposit", body).await
}

// ===== Hold Deposit (THB rolling credit) API =====

/// Request body for POST /api/deposit/hold — attendee holds deposit as rolling credit.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HoldDepositRequest {
    /// Event the deposit belongs to.
    pub event_id: String,
    /// Attendee API ID holding the deposit.
    pub attendee_id: String,
}

/// Response for POST /api/deposit/hold — returns the new credit balance after hold.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct HoldDepositResponse {
    /// THB credit balance after hold (smallest unit if applicable, here raw THB).
    pub credit_thb: u64,
    /// USDC credit balance after hold (smallest unit).
    pub credit_usdc: u64,
    /// Human-readable message.
    pub message: String,
}

/// Response for GET /api/deposit/credit-balance — attendee's current rolling credit.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CreditBalanceResponse {
    /// THB credit balance.
    pub credit_thb: u64,
    /// USDC credit balance.
    pub credit_usdc: u64,
}

/// POST /api/deposit/hold — hold deposit as rolling credit (THB or USDC).
pub async fn hold_deposit(body: &HoldDepositRequest) -> Result<HoldDepositResponse, ApiError> {
    api_post_json("/deposit/hold", body).await
}

/// GET /api/deposit/credit-balance — fetch the authenticated attendee's rolling credit balance.
pub async fn get_credit_balance() -> Result<CreditBalanceResponse, ApiError> {
    api_get_json("/deposit/credit-balance").await
}
