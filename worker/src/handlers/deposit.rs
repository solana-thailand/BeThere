//! Deposit/refund API handlers for dual-track payment (USDC + THB).
//!
//! Issue 010 Phase 5 — USDC on-chain TX building + Worker Deposit/Refund API.
//!
//! Endpoints:
//!   GET  /api/deposit/status/{attendee_id}  — check deposit status
//!   POST /api/deposit/usdc                  — initiate USDC deposit (Solana Pay URL)
//!   GET  /api/deposit/usdc/tx               — Solana Pay TX callback (wallet fetches TX)
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

/// Initiate a USDC deposit by building a Solana Pay Transaction Request.
///
/// This endpoint:
/// 1. Validates the event, deposit config, and wallet address
/// 2. Records a pending deposit status in KV
/// 3. Returns a Solana Pay URL that points to our TX callback endpoint
///
/// The Solana Pay flow works as follows:
/// - Frontend renders the `solana_pay_url` as a QR code
/// - Wallet scans the QR and calls our callback endpoint (`GET /api/deposit/usdc/tx`)
/// - Callback returns a serialized transaction for the wallet to sign and send
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

    // Build Solana Pay Transaction Request URL.
    // Format: `solana:{callback_url}` — the wallet fetches the actual TX from this URL.
    let callback_url = format!(
        "{}/api/deposit/usdc/tx?event_id={}&attendee_id={}&wallet={}",
        state.config.server.url,
        urlencoding::encode(&event.id),
        urlencoding::encode(&body.attendee_id),
        urlencoding::encode(&body.wallet_address),
    );
    let solana_pay_url = format!("solana:{callback_url}");

    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %event.id,
        amount = event.deposit_amount_usdc,
        "USDC deposit initiated"
    );

    Ok(Json(UsdcDepositResponse {
        transaction: String::new(), // Transaction is built on-demand by the callback endpoint
        solana_pay_url,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/deposit/usdc/tx — Solana Pay Transaction Request callback
// ---------------------------------------------------------------------------

/// Query parameters for the Solana Pay TX callback.
#[derive(Debug, serde::Deserialize)]
pub struct DepositTxQuery {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// Attendee's Solana wallet address (base58).
    pub wallet: String,
}

/// Solana Pay Transaction Request response.
///
/// Returned when a wallet calls the callback URL from a Solana Pay QR code.
/// Contains a base64-encoded serialized transaction for the wallet to sign and submit.
#[derive(Debug, serde::Serialize)]
pub struct DepositTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet adds signature).
    pub transaction: String,
    /// Human-readable message shown in the wallet confirmation UI.
    pub message: String,
}

/// Solana Pay Transaction Request callback.
///
/// When a wallet scans the Solana Pay QR code, it fetches this endpoint
/// to get the serialized deposit transaction. The wallet then:
/// 1. Shows the `message` to the user
/// 2. Signs the transaction with the attendee's keypair
/// 3. Submits it to the Solana network
///
/// This builds the actual on-chain `deposit` instruction with:
/// - PDA-derived `EventEscrow` and `AttendeeDeposit` accounts
/// - Associated Token Account for the attendee's USDC
/// - The escrow vault (ATA of the EventEscrow PDA)
#[worker::send]
pub async fn deposit_usdc_tx_handler(
    State(state): State<AppState>,
    Query(query): Query<DepositTxQuery>,
) -> Result<Json<DepositTxResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event = event_store::get_event_config(kv, &query.event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("event '{}' not found", query.event_id)))?;

    if !event.deposit_enabled {
        return Err(AppError::Validation("deposit not enabled for this event".to_string()).into());
    }

    if event.deposit_amount_usdc == 0 {
        return Err(AppError::Validation("deposit amount not configured".to_string()).into());
    }

    // Validate wallet address
    crate::solana::validate_wallet_address(&query.wallet).map_err(AppError::Validation)?;

    // Verify deposit is still pending (not already completed)
    let existing = event_store::get_deposit_status(kv, &event.id, &query.attendee_id)
        .await
        .map_err(AppError::Internal)?;

    if let Some(status) = &existing
        && status.verified {
            return Err(AppError::Validation("deposit already verified".to_string()).into());
        }

    // Determine organizer pubkey for PDA derivation.
    // The event must have `organizer_wallet` set (the organizer's Solana address).
    // This is configured when the event is set up for deposits.
    let organizer_pubkey = if event.organizer_wallet.is_empty() {
        return Err(AppError::Internal(
            "event has no organizer wallet configured — set organizer_wallet before enabling deposits".to_string(),
        ).into());
    } else {
        // Validate it's a proper base58 Solana address
        crate::solana::validate_wallet_address(&event.organizer_wallet)
            .map_err(|e| AppError::Internal(format!("invalid organizer_wallet: {e}")))?;
        &event.organizer_wallet
    };

    // The on_chain_event_id for PDA derivation.
    // If explicitly set (non-zero), use it. Otherwise, derive from event ID hash.
    let on_chain_event_id = if event.on_chain_event_id != 0 {
        event.on_chain_event_id
    } else {
        derive_on_chain_event_id(&event.id)
    };

    // Build the RPC URL with API key
    let rpc_url = format!(
        "{}{}{}",
        state.config.solana.rpc_url,
        if state.config.solana.rpc_url.contains('?') {
            "&"
        } else {
            "?api-key="
        },
        state.config.solana.api_key
    );

    // Build the deposit transaction
    let tx = crate::solana_escrow::build_deposit_transaction(
        &rpc_url,
        organizer_pubkey,
        on_chain_event_id,
        &query.wallet,
        event.deposit_amount_usdc,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to build deposit TX: {e}")))?;

    tracing::info!(
        attendee_id = %query.attendee_id,
        event_id = %event.id,
        "Deposit TX built for wallet callback"
    );

    Ok(Json(DepositTxResponse {
        transaction: tx.transaction_b64,
        message: tx.message,
    }))
}

/// Derive a stable u64 event ID from a string event ID for on-chain PDA derivation.
/// Uses FNV-1a hash for deterministic, collision-resistant mapping.
fn derive_on_chain_event_id(event_id: &str) -> u64 {
    // FNV-1a 64-bit hash
    let mut hash: u64 = 0xcbf29ce484222325; // FNV offset basis
    for byte in event_id.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3); // FNV prime
    }
    // Ensure non-zero
    if hash == 0 {
        hash = 1;
    }
    hash
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
