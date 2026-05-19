//! Deposit/refund types and API functions, including escrow operations.

// serde derives used via full path in attribute macros

use super::types::{ApiError, ApiResponse};
use super::{api_get, api_post_json};

// ===== Deposit/Refund Types =====

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DepositStatusInfo {
    pub attendee_id: String,
    pub event_id: String,
    pub method: String,
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
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VerifySlipRequest {
    pub event_id: String,
    pub attendee_id: String,
    pub approved: bool,
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
    #[serde(default)]
    pub attendee_name: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PendingSlipResponse {
    pub slips: Vec<ThbDepositInfo>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct RefundQueueResponse {
    pub pending: Vec<ThbDepositInfo>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MarkRefundRequest {
    pub event_id: String,
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
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to get deposit status".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<DepositStatusResponse> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse deposit status: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
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
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to check deposit confirmation".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<ConfirmDepositResponse> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse deposit confirmation: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/deposit/thb/upload
pub async fn upload_thb_slip(body: &ThbSlipUploadRequest) -> Result<serde_json::Value, ApiError> {
    api_post_json("/deposit/thb/upload", body).await
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
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to get pending slips".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<PendingSlipResponse> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse pending slips: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// GET /api/refund/queue?event_id=xxx
pub async fn get_refund_queue(event_id: Option<&str>) -> Result<RefundQueueResponse, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/refund/queue?event_id={eid}"),
        _ => "/refund/queue".to_string(),
    };
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to get refund queue".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<RefundQueueResponse> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse refund queue: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/refund/mark/{attendee_id}
pub async fn mark_refund(
    attendee_id: &str,
    body: &MarkRefundRequest,
) -> Result<serde_json::Value, ApiError> {
    let path = format!("/refund/mark/{attendee_id}");
    api_post_json(&path, body).await
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
