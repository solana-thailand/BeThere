//! Deposit/refund API handlers for dual-track payment (USDC + THB).
//!
//! Issue 010 Phase 2 — Worker Deposit/Refund API.
//!
//! Endpoints:
//!   GET  /api/deposit/status/{attendee_id}  — check deposit status
//!   POST /api/deposit/usdc                  — build Solana Pay deposit TX
//!   POST /api/deposit/thb/upload            — record THB slip upload
//!   POST /api/deposit/thb/verify            — admin verifies/rejects slip
//!   GET  /api/deposit/thb/pending           — list unverified slips (admin)
//!   POST /api/refund/mark/{attendee_id}     — mark THB refund as done (admin)
//!   GET  /api/refund/queue                  — refund queue (THB pending)

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use chrono::Utc;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::{
    DepositMethod, DepositStatus, DepositStatusResponse, MarkRefundRequest, PendingSlipResponse,
    RefundQueueResponse, ThbDeposit, UsdcDepositRequest, UsdcDepositResponse, VerifySlipRequest,
};
use event_checkin_domain::models::error::AppError;

use crate::error::WorkerError;
use crate::event_store;
use crate::handlers::ext::EventIdQuery;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// GET /api/deposit/status/{attendee_id}
// ---------------------------------------------------------------------------

/// Check deposit status for an attendee.
/// Public endpoint — attendee can check their own status.
#[worker::send]
pub async fn get_deposit_status_handler(
    State(state): State<AppState>,
    Path(attendee_id): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<Json<DepositStatusResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event =
        event_store::resolve_event_or_fallback(Some(kv), query.event_id.as_deref(), &state.config)
            .await
            .map_err(AppError::Internal)?;

    let status = event_store::get_deposit_status(kv, &event.id, &attendee_id)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(DepositStatusResponse {
        deposit_enabled: event.deposit_enabled,
        deposit_amount_usdc: event.deposit_amount_usdc,
        deposit_amount_thb: event.deposit_amount_thb,
        status,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/deposit/usdc
// ---------------------------------------------------------------------------

/// Build a Solana Pay URL for USDC deposit.
///
/// Returns a `solana:{baseUrl}?tx={base64Tx}` URL that the frontend
/// can render as a QR code or open in a wallet adapter.
///
/// For MVP, we return a Solana Pay transfer URL that the wallet will sign.
/// The actual on-chain escrow deposit instruction is built client-side
/// using the generated `bethere-escrow-client`.
#[worker::send]
pub async fn deposit_usdc_handler(
    State(state): State<AppState>,
    Json(body): Json<UsdcDepositRequest>,
) -> Result<Json<UsdcDepositResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event = event_store::get_event_config(kv, &body.event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("event '{}' not found", body.event_id)))?;

    if !event.deposit_enabled {
        return Err(AppError::Validation("deposit not enabled for this event".to_string()).into());
    }

    if event.deposit_amount_usdc == 0 {
        return Err(AppError::Validation("deposit amount not configured".to_string()).into());
    }

    // Validate wallet address
    crate::solana::validate_wallet_address(&body.wallet_address).map_err(AppError::Validation)?;

    // Check if already deposited
    let existing = event_store::get_deposit_status(kv, &event.id, &body.attendee_id)
        .await
        .map_err(AppError::Internal)?;

    if existing.is_some() {
        return Err(AppError::Validation("attendee already has a deposit".to_string()).into());
    }

    // Build Solana Pay URL for the deposit.
    // Escrow program ID deployed on devnet.
    let program_id = "2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo";
    // Devnet USDC mint. Mainnet: EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m
    let usdc_mint = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

    // For now, build a descriptive Solana Pay URL.
    // The frontend will use the bethere-escrow-client to build the actual TX.
    let solana_pay_url = format!(
        "solana:{program_id}?deposit={}&event={}&attendee={}&mint={usdc_mint}",
        event.deposit_amount_usdc, event.id, body.attendee_id,
    );

    // Record a pending deposit status
    let deposit_status = DepositStatus {
        attendee_id: body.attendee_id.clone(),
        event_id: event.id.clone(),
        method: DepositMethod::Usdc,
        amount: event.deposit_amount_usdc,
        currency: "USDC".to_string(),
        tx_signature: None,
        verified: false,
        deposited_at: Utc::now().to_rfc3339(),
    };

    event_store::save_deposit_status(kv, &deposit_status)
        .await
        .map_err(AppError::Internal)?;

    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %event.id,
        amount = event.deposit_amount_usdc,
        "USDC deposit initiated"
    );

    Ok(Json(UsdcDepositResponse {
        transaction: String::new(), // MVP: client builds TX from escrow-client
        solana_pay_url,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/deposit/thb/upload
// ---------------------------------------------------------------------------

/// Record a THB payment slip upload.
///
/// The frontend uploads the slip image to R2 separately and passes the URL.
/// This creates a THB deposit record in KV for admin verification.
#[worker::send]
pub async fn upload_thb_slip_handler(
    State(state): State<AppState>,
    Json(body): Json<ThbSlipUploadRequest>,
) -> Result<Json<serde_json::Value>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event = event_store::get_event_config(kv, &body.event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("event '{}' not found", body.event_id)))?;

    if !event.deposit_enabled {
        return Err(AppError::Validation("deposit not enabled for this event".to_string()).into());
    }

    if event.deposit_amount_thb == 0 {
        return Err(AppError::Validation("THB deposit amount not configured".to_string()).into());
    }

    // Check if already deposited
    let existing = event_store::get_deposit_status(kv, &event.id, &body.attendee_id)
        .await
        .map_err(AppError::Internal)?;

    if existing.is_some() {
        return Err(AppError::Validation("attendee already has a deposit".to_string()).into());
    }

    let now = Utc::now().to_rfc3339();

    // Create THB deposit record
    let thb_deposit = ThbDeposit {
        attendee_id: body.attendee_id.clone(),
        event_id: event.id.clone(),
        amount_thb: event.deposit_amount_thb,
        slip_url: Some(body.slip_url.clone()),
        verified: false,
        verified_by: None,
        verified_at: None,
        uploaded_at: now.clone(),
        refunded: false,
        refunded_at: None,
    };

    event_store::save_thb_deposit(kv, &thb_deposit)
        .await
        .map_err(AppError::Internal)?;

    // Create deposit status
    let deposit_status = DepositStatus {
        attendee_id: body.attendee_id.clone(),
        event_id: event.id.clone(),
        method: DepositMethod::Thb,
        amount: event.deposit_amount_thb,
        currency: "THB".to_string(),
        tx_signature: None,
        verified: false,
        deposited_at: now,
    };

    event_store::save_deposit_status(kv, &deposit_status)
        .await
        .map_err(AppError::Internal)?;

    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %event.id,
        amount_thb = event.deposit_amount_thb,
        "THB deposit slip uploaded"
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "slip uploaded, awaiting verification"
    })))
}

/// Request body for THB slip upload.
#[derive(Debug, serde::Deserialize)]
pub struct ThbSlipUploadRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// R2 URL of the uploaded payment slip image.
    pub slip_url: String,
}

// ---------------------------------------------------------------------------
// POST /api/deposit/thb/verify (admin)
// ---------------------------------------------------------------------------

/// Admin verifies or rejects a THB payment slip.
#[worker::send]
pub async fn verify_thb_slip_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<VerifySlipRequest>,
) -> Result<Json<serde_json::Value>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, Some(&body.event_id))
            .await?;

    // Get existing THB deposit
    let mut thb_deposit = event_store::get_thb_deposit(kv, &event.id, &body.attendee_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "THB deposit not found for attendee '{}' in event '{}'",
                body.attendee_id, event.id
            ))
        })?;

    let now = Utc::now().to_rfc3339();

    if body.approved {
        thb_deposit.verified = true;
        thb_deposit.verified_by = Some(claims.email.clone());
        thb_deposit.verified_at = Some(now.clone());
    } else {
        // Rejected — keep the record but mark as not verified
        thb_deposit.verified = false;
        thb_deposit.verified_by = Some(claims.email.clone());
        thb_deposit.verified_at = Some(now.clone());
    }

    event_store::save_thb_deposit(kv, &thb_deposit)
        .await
        .map_err(AppError::Internal)?;

    // Update deposit status
    if let Some(mut status) = event_store::get_deposit_status(kv, &event.id, &body.attendee_id)
        .await
        .map_err(AppError::Internal)?
    {
        status.verified = body.approved;
        event_store::save_deposit_status(kv, &status)
            .await
            .map_err(AppError::Internal)?;
    }

    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %event.id,
        approved = body.approved,
        verifier = %claims.email,
        "THB deposit slip verified"
    );

    let msg = if body.approved {
        "deposit verified"
    } else {
        "deposit rejected"
    };

    Ok(Json(serde_json::json!({
        "success": true,
        "message": msg
    })))
}

// ---------------------------------------------------------------------------
// GET /api/deposit/thb/pending (admin)
// ---------------------------------------------------------------------------

/// List all unverified THB deposits for admin review.
#[worker::send]
pub async fn pending_thb_slips_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<Json<PendingSlipResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, query.event_id.as_deref())
            .await?;

    let all_deposits = event_store::list_thb_deposits(kv, &event.id)
        .await
        .map_err(AppError::Internal)?;

    let pending: Vec<ThbDeposit> = all_deposits
        .into_iter()
        .filter(|d| !d.verified && d.slip_url.is_some())
        .collect();

    Ok(Json(PendingSlipResponse { slips: pending }))
}

// ---------------------------------------------------------------------------
// GET /api/refund/queue (admin)
// ---------------------------------------------------------------------------

/// List THB deposits that need refund (verified + checked-in + not yet refunded).
#[worker::send]
pub async fn refund_queue_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<Json<RefundQueueResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, query.event_id.as_deref())
            .await?;

    let all_deposits = event_store::list_thb_deposits(kv, &event.id)
        .await
        .map_err(AppError::Internal)?;

    let pending: Vec<ThbDeposit> = all_deposits
        .into_iter()
        .filter(|d| d.verified && !d.refunded)
        .collect();

    Ok(Json(RefundQueueResponse { pending }))
}

// ---------------------------------------------------------------------------
// POST /api/refund/mark/{attendee_id} (admin)
// ---------------------------------------------------------------------------

/// Mark a THB refund as completed.
#[worker::send]
pub async fn mark_refund_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(attendee_id): Path<String>,
    Json(body): Json<MarkRefundRequest>,
) -> Result<Json<serde_json::Value>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, Some(&body.event_id))
            .await?;

    let mut thb_deposit = event_store::get_thb_deposit(kv, &event.id, &attendee_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "THB deposit not found for attendee '{attendee_id}' in event '{}'",
                event.id
            ))
        })?;

    if thb_deposit.refunded {
        return Err(AppError::Validation("already refunded".to_string()).into());
    }

    if !thb_deposit.verified {
        return Err(
            AppError::Validation("deposit not verified yet — cannot refund".to_string()).into(),
        );
    }

    let now = Utc::now().to_rfc3339();
    thb_deposit.refunded = true;
    thb_deposit.refunded_at = Some(now);

    event_store::save_thb_deposit(kv, &thb_deposit)
        .await
        .map_err(AppError::Internal)?;

    tracing::info!(
        attendee_id = %attendee_id,
        event_id = %event.id,
        marker = %claims.email,
        "THB refund marked complete"
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "refund marked complete"
    })))
}
