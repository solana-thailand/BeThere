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

use crate::error::{ApiOk, WorkerError};
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
) -> Result<ApiOk<DepositStatusResponse>, WorkerError> {
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

    Ok(ApiOk::new(DepositStatusResponse {
        deposit_enabled: event.deposit_enabled,
        deposit_amount_usdc: event.deposit_amount_usdc,
        deposit_amount_thb: event.deposit_amount_thb,
        promptpay_id: event.promptpay_id,
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
) -> Result<ApiOk<UsdcDepositResponse>, WorkerError> {
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

    // Reject deposits after the event has ended — the on-chain refund requires clock > event_end
    if event.event_end_ms > 0 {
        let now_ms = chrono::Utc::now().timestamp_millis();
        if now_ms > event.event_end_ms {
            return Err(AppError::Validation(
                "event has ended — deposits are no longer accepted".to_string(),
            )
            .into());
        }
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
        wallet_address: Some(body.wallet_address.clone()),
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

    Ok(ApiOk::new(UsdcDepositResponse {
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
        && status.verified
    {
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
        Some(kv),
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

// Derive a stable u64 event ID from a string event ID for on-chain PDA derivation.
// Uses FNV-1a hash for deterministic, collision-resistant mapping.
// ---------------------------------------------------------------------------
// GET /api/deposit/usdc/confirm — Poll for deposit TX confirmation
// ---------------------------------------------------------------------------

/// Query parameters for checking deposit confirmation.
#[derive(Debug, serde::Deserialize)]
pub struct ConfirmDepositQuery {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID.
    pub attendee_id: String,
}

/// Response for deposit confirmation check.
#[derive(Debug, serde::Serialize)]
pub struct ConfirmDepositResponse {
    /// Whether the deposit has been confirmed on-chain.
    pub confirmed: bool,
    /// Transaction signature if confirmed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_signature: Option<String>,
    /// Solana Pay URL to retry (if not yet confirmed and TX not sent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solana_pay_url: Option<String>,
}

/// Check if a USDC deposit has been confirmed on-chain.
///
/// This endpoint is polled by the frontend after the attendee sends the deposit TX.
/// It checks the KV-stored `DepositStatus` for the `verified` flag and
/// optionally calls the RPC to verify the TX landed.
#[worker::send]
pub async fn confirm_deposit_handler(
    State(state): State<AppState>,
    Query(query): Query<ConfirmDepositQuery>,
) -> Result<ApiOk<ConfirmDepositResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event = event_store::get_event_config(kv, &query.event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("event '{}' not found", query.event_id)))?;

    // Check deposit status in KV
    let deposit_status = event_store::get_deposit_status(kv, &event.id, &query.attendee_id)
        .await
        .map_err(AppError::Internal)?;

    match deposit_status {
        Some(status) if status.verified => {
            // Already verified
            Ok(ApiOk::new(ConfirmDepositResponse {
                confirmed: true,
                tx_signature: status.tx_signature.clone(),
                solana_pay_url: None,
            }))
        }
        Some(status) => {
            // Pending — check if there's a tx_signature to verify on-chain
            match &status.tx_signature {
                Some(sig) if !sig.is_empty() => {
                    // Verify the TX on-chain via RPC
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
                    let confirmed = verify_tx_on_chain(&rpc_url, sig).await;

                    if confirmed {
                        // Update the deposit status to verified
                        let mut updated = status.clone();
                        updated.verified = true;
                        event_store::save_deposit_status(kv, &updated)
                            .await
                            .map_err(AppError::Internal)?;

                        tracing::info!(
                            attendee_id = %query.attendee_id,
                            tx_signature = %sig,
                            "USDC deposit confirmed on-chain"
                        );

                        Ok(ApiOk::new(ConfirmDepositResponse {
                            confirmed: true,
                            tx_signature: Some(sig.clone()),
                            solana_pay_url: None,
                        }))
                    } else {
                        // TX not yet confirmed, keep polling
                        Ok(ApiOk::new(ConfirmDepositResponse {
                            confirmed: false,
                            tx_signature: Some(sig.clone()),
                            solana_pay_url: None,
                        }))
                    }
                }
                _ => {
                    // No TX signature yet — still pending
                    // Return the Solana Pay URL so frontend can retry
                    let callback_url = format!(
                        "{}/api/deposit/usdc/tx?event_id={}&attendee_id={}&wallet=",
                        state.config.server.url,
                        urlencoding::encode(&event.id),
                        urlencoding::encode(&query.attendee_id),
                    );
                    Ok(ApiOk::new(ConfirmDepositResponse {
                        confirmed: false,
                        tx_signature: None,
                        solana_pay_url: Some(format!("solana:{callback_url}")),
                    }))
                }
            }
        }
        None => {
            // No deposit record — attendee hasn't initiated yet
            Ok(ApiOk::new(ConfirmDepositResponse {
                confirmed: false,
                tx_signature: None,
                solana_pay_url: None,
            }))
        }
    }
}

/// Verify a transaction signature on-chain by checking its confirmation status via RPC.
async fn verify_tx_on_chain(rpc_url: &str, signature: &str) -> bool {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-confirm",
        "method": "getSignatureStatuses",
        "params": [
            [signature],
            { "searchTransactionHistory": true }
        ]
    });

    let json_body = match serde_json::to_string(&body) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("failed to serialize signature status request: {e}");
            return false;
        }
    };

    let headers = worker::Headers::new();
    if let Err(e) = headers.set("Content-Type", "application/json") {
        tracing::error!("failed to set header: {e:?}");
        return false;
    }

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = match worker::Request::new_with_init(rpc_url, &init) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("failed to create RPC request: {e:?}");
            return false;
        }
    };

    let mut response = match worker::Fetch::Request(request).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("RPC request failed: {e:?}");
            return false;
        }
    };

    let body_text = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("failed to read RPC response: {e:?}");
            return false;
        }
    };

    // Parse the response: { result: { value: [ { confirmationStatus: "confirmed" | "finalized", err: null } ] } }
    let parsed: serde_json::Value = match serde_json::from_str(&body_text) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("failed to parse RPC response: {e}");
            return false;
        }
    };

    // Navigate to result.value[0]
    let status_opt = parsed
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .cloned();

    let Some(status) = status_opt else {
        tracing::debug!(
            tx_signature = %signature,
            "getSignatureStatuses returned null — TX not found (may have expired from status cache)"
        );
        return false;
    };

    // Null status entry means TX not found
    if status.is_null() {
        tracing::debug!(
            tx_signature = %signature,
            "Signature status is null — TX not found on-chain"
        );
        return false;
    }

    // Check if there's an error
    if status.get("err").is_some_and(|e| !e.is_null()) {
        tracing::warn!(tx_signature = %signature, "TX failed on-chain: {:?}", status.get("err"));
        return false;
    }

    // Check confirmation status
    let confirmation = status
        .get("confirmationStatus")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");
    let confirmed = confirmation == "confirmed" || confirmation == "finalized";
    tracing::debug!(
        tx_signature = %signature,
        confirmation_status = %confirmation,
        confirmed = %confirmed,
        "Signature status checked"
    );
    confirmed
}

// ---------------------------------------------------------------------------
// POST /api/deposit/usdc/webhook — Helius webhook for TX confirmation
// ---------------------------------------------------------------------------

/// Helius webhook payload for transaction notification.
/// See: https://docs.helius.dev/webhooks/webhook-payload
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct HeliusWebhookPayload {
    /// Array of transaction notifications.
    #[serde(default)]
    pub data: Vec<HeliusTransactionData>,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct HeliusTransactionData {
    /// Transaction signature.
    pub signature: String,
    /// Transaction type.
    #[serde(default)]
    pub r#type: String,
    /// Description (human-readable).
    #[serde(default)]
    pub description: String,
}

/// Response body for updating deposit status with TX signature.
#[derive(Debug, serde::Deserialize)]
pub struct UpdateDepositSignatureRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID.
    pub attendee_id: String,
    /// On-chain transaction signature.
    pub tx_signature: String,
}

/// Helius webhook handler for USDC deposit confirmations.
///
/// Called by Helius when a monitored transaction is confirmed on-chain.
/// Updates the deposit status in KV to `verified: true`.
///
/// This endpoint is also called directly by the frontend after a wallet
/// sends a deposit TX, to record the TX signature for later polling.
#[worker::send]
pub async fn deposit_webhook_handler(
    State(state): State<AppState>,
    Json(body): Json<UpdateDepositSignatureRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    // Get existing deposit status
    let mut deposit_status = event_store::get_deposit_status(kv, &body.event_id, &body.attendee_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "no deposit record for attendee '{}' in event '{}'",
                body.attendee_id, body.event_id
            ))
        })?;

    // Update with TX signature
    deposit_status.tx_signature = Some(body.tx_signature.clone());

    // Try to verify the TX on-chain immediately
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
    let confirmed = verify_tx_on_chain(&rpc_url, &body.tx_signature).await;

    if confirmed {
        deposit_status.verified = true;
        tracing::info!(
            attendee_id = %body.attendee_id,
            tx_signature = %body.tx_signature,
            "USDC deposit verified via webhook"
        );
    } else {
        tracing::info!(
            attendee_id = %body.attendee_id,
            tx_signature = %body.tx_signature,
            "USDC deposit TX signature recorded, pending on-chain confirmation"
        );
    }

    event_store::save_deposit_status(kv, &deposit_status)
        .await
        .map_err(AppError::Internal)?;

    Ok(ApiOk::new(serde_json::json!({
        "success": true,
        "confirmed": confirmed,
        "tx_signature": body.tx_signature,
    })))
}

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
// POST /api/escrow/init — Combined ATA + CreateEvent in one TX
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/init.
#[derive(Debug, serde::Deserialize)]
pub struct InitEscrowTxRequest {
    /// Event ID (slug or KV key).
    pub event_id: String,
}

/// Response with the combined init escrow transaction.
#[derive(Debug, serde::Serialize)]
pub struct InitEscrowTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet signs).
    pub transaction: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
    /// Derived EventEscrow PDA address (base58).
    pub escrow_address: String,
    /// Derived vault ATA address (base58).
    pub vault_address: String,
    /// The on-chain event ID used for PDA derivation.
    pub on_chain_event_id: u64,
}

/// Build a single transaction that combines:
/// 1. Create the vault Associated Token Account (ATA program, idempotent)
/// 2. Initialize the on-chain event escrow (escrow program)
///
/// The organizer signs once instead of twice.
#[worker::send]
pub async fn init_escrow_tx_handler(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(body): Json<InitEscrowTxRequest>,
) -> Result<ApiOk<InitEscrowTxResponse>, WorkerError> {
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

    // Check if already created on-chain
    if !event.escrow_address.is_empty() {
        return Err(AppError::Validation(format!(
            "event already has escrow address: {}",
            event.escrow_address
        ))
        .into());
    }

    // Validate organizer wallet
    let organizer_pubkey = if event.organizer_wallet.is_empty() {
        return Err(AppError::Validation(
            "event has no organizer wallet configured — set organizer_wallet first".to_string(),
        )
        .into());
    } else {
        crate::solana::validate_wallet_address(&event.organizer_wallet)
            .map_err(|e| AppError::Validation(format!("invalid organizer_wallet: {e}")))?;
        &event.organizer_wallet
    };

    // Determine on-chain event ID
    let on_chain_event_id = if event.on_chain_event_id != 0 {
        event.on_chain_event_id
    } else {
        derive_on_chain_event_id(&event.id)
    };

    // Calculate event_end and refund_deadline as unix timestamps (seconds)
    let event_end = if event.event_end_ms > 0 {
        event.event_end_ms / 1000
    } else {
        // Default: 7 days from now
        chrono::Utc::now().timestamp() + 86400 * 7
    };

    let refund_deadline = if event.refund_deadline_hours > 0 {
        event_end + (event.refund_deadline_hours as i64 * 3600)
    } else {
        // Default: 7 days after event end
        event_end + 86400 * 7
    };

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

    let tx = crate::solana_escrow::build_init_escrow_transaction(
        &rpc_url,
        Some(kv),
        organizer_pubkey,
        on_chain_event_id,
        event.deposit_amount_usdc,
        event_end,
        refund_deadline,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to build init escrow TX: {e}")))?;

    tracing::info!(
        event_id = %event.id,
        escrow_address = %tx.escrow_address,
        vault_address = %tx.vault_address,
        on_chain_event_id,
        "Combined init escrow TX built for organizer"
    );

    Ok(ApiOk::new(InitEscrowTxResponse {
        transaction: tx.transaction_b64,
        message: tx.message,
        escrow_address: tx.escrow_address,
        vault_address: tx.vault_address,
        on_chain_event_id,
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
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
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
        wallet_address: None,
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

    Ok(ApiOk::new(serde_json::json!({
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
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
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

    Ok(ApiOk::new(serde_json::json!({
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
) -> Result<ApiOk<PendingSlipResponse>, WorkerError> {
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

    Ok(ApiOk::new(PendingSlipResponse { slips: pending }))
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
) -> Result<ApiOk<RefundQueueResponse>, WorkerError> {
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

    Ok(ApiOk::new(RefundQueueResponse { pending }))
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
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
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

    Ok(ApiOk::new(serde_json::json!({
        "success": true,
        "message": "refund marked complete"
    })))
}

// ---------------------------------------------------------------------------
// POST /api/escrow/refund — Build refund TX for attendee
// ---------------------------------------------------------------------------

/// Request body for building a refund transaction.
#[derive(Debug, serde::Deserialize)]
pub struct RefundTxRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// Attendee's Solana wallet address (base58).
    pub wallet_address: String,
}

/// Response with the serialized refund transaction.
#[derive(Debug, serde::Serialize)]
pub struct RefundTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet signs).
    pub transaction: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Build a refund transaction for an attendee's verified USDC deposit.
///
/// This is a **public endpoint** — attendees call it to claim their refund.
/// The attendee's wallet signature provides on-chain authentication.
///
/// Prerequisites:
/// - Event has deposits enabled and escrow initialized
/// - Attendee has a verified USDC deposit
#[worker::send]
pub async fn refund_tx_handler(
    State(state): State<AppState>,
    Json(body): Json<RefundTxRequest>,
) -> Result<ApiOk<RefundTxResponse>, WorkerError> {
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

    // Must have escrow initialized on-chain
    if event.escrow_address.is_empty() {
        return Err(
            AppError::Validation("event escrow not initialized on-chain".to_string()).into(),
        );
    }

    // Validate wallet address
    crate::solana::validate_wallet_address(&body.wallet_address).map_err(AppError::Validation)?;

    // Check attendee has a verified USDC deposit
    let deposit_status = event_store::get_deposit_status(kv, &event.id, &body.attendee_id)
        .await
        .map_err(AppError::Internal)?;

    let status = deposit_status.ok_or_else(|| {
        AppError::NotFound(format!(
            "no deposit found for attendee '{}'",
            body.attendee_id
        ))
    })?;

    if !status.verified {
        return Err(
            AppError::Validation("deposit not verified yet — cannot refund".to_string()).into(),
        );
    }

    if status.method != DepositMethod::Usdc {
        return Err(AppError::Validation(
            "refund TX only supported for USDC deposits — use THB refund flow instead".to_string(),
        )
        .into());
    }

    // Determine organizer pubkey for PDA derivation
    let organizer_pubkey = if event.organizer_wallet.is_empty() {
        return Err(
            AppError::Internal("event has no organizer wallet configured".to_string()).into(),
        );
    } else {
        crate::solana::validate_wallet_address(&event.organizer_wallet)
            .map_err(|e| AppError::Validation(format!("invalid organizer_wallet: {e}")))?;
        &event.organizer_wallet
    };

    // The on_chain_event_id for PDA derivation
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

    // Build the refund transaction
    let tx = crate::solana_escrow::build_refund_transaction(
        &rpc_url,
        Some(kv),
        organizer_pubkey,
        on_chain_event_id,
        &body.wallet_address,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to build refund TX: {e}")))?;

    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %event.id,
        "Refund TX built for attendee"
    );

    Ok(ApiOk::new(RefundTxResponse {
        transaction: tx.transaction_b64,
        message: tx.message,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/escrow/mark-checked-in — build mark_checked_in TX (organizer)
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
pub struct MarkCheckedInTxRequest {
    /// Event ID (slug or KV key).
    pub event_id: String,
    /// Attendee API ID from Google Sheets (used to look up wallet from deposit).
    pub attendee_id: String,
    /// Attendee's Solana wallet address (base58).
    /// If not provided, looked up from the deposit record.
    #[serde(default)]
    pub attendee_wallet: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct MarkCheckedInTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet signs).
    pub transaction: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Build a `mark_checked_in` transaction for the escrow program.
///
/// This is called by the organizer to mark an attendee as checked in.
/// The organizer's wallet signs and submits the returned transaction.
/// After this, the attendee can claim a refund.
#[worker::send]
pub async fn mark_checked_in_tx_handler(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(body): Json<MarkCheckedInTxRequest>,
) -> Result<ApiOk<MarkCheckedInTxResponse>, WorkerError> {
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

    // Must have escrow initialized on-chain
    if event.escrow_address.is_empty() {
        return Err(
            AppError::Validation("event escrow not initialized on-chain".to_string()).into(),
        );
    }

    // Resolve attendee wallet: use provided value or look up from deposit record
    let attendee_wallet = match &body.attendee_wallet {
        Some(w) if !w.is_empty() => w.clone(),
        _ => {
            // Look up from deposit status
            let deposit = event_store::get_deposit_status(kv, &event.id, &body.attendee_id)
                .await
                .map_err(AppError::Internal)?
                .ok_or_else(|| {
                    AppError::NotFound(format!(
                        "no deposit found for attendee '{}' — cannot resolve wallet",
                        body.attendee_id
                    ))
                })?;
            deposit.wallet_address.ok_or_else(|| {
                AppError::NotFound(format!(
                    "deposit for attendee '{}' has no wallet address",
                    body.attendee_id
                ))
            })?
        }
    };

    // Validate attendee wallet
    crate::solana::validate_wallet_address(&attendee_wallet).map_err(AppError::Validation)?;

    // Determine organizer pubkey
    let organizer_pubkey = if event.organizer_wallet.is_empty() {
        return Err(
            AppError::Internal("event has no organizer wallet configured".to_string()).into(),
        );
    } else {
        crate::solana::validate_wallet_address(&event.organizer_wallet)
            .map_err(|e| AppError::Validation(format!("invalid organizer_wallet: {e}")))?;
        &event.organizer_wallet
    };

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

    let tx = crate::solana_escrow::build_mark_checked_in_transaction(
        &rpc_url,
        Some(kv),
        organizer_pubkey,
        on_chain_event_id,
        &attendee_wallet,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to build mark_checked_in TX: {e}")))?;

    tracing::info!(
        attendee_wallet = %attendee_wallet,
        attendee_id = %body.attendee_id,
        event_id = %event.id,
        "Mark checked-in TX built for organizer"
    );

    Ok(ApiOk::new(MarkCheckedInTxResponse {
        transaction: tx.transaction_b64,
        message: tx.message,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/escrow/backfill-wallets — Admin: backfill wallet_address for deposits
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/backfill-wallets.
#[derive(Debug, serde::Deserialize)]
pub struct BackfillWalletsRequest {
    /// Event ID to backfill. If omitted, backfills all events.
    #[serde(default)]
    pub event_id: Option<String>,
}

/// Response for POST /api/escrow/backfill-wallets.
#[derive(Debug, serde::Serialize)]
pub struct BackfillWalletsResponse {
    /// Total deposits scanned.
    pub scanned: usize,
    /// Deposits missing wallet_address.
    pub missing_wallet: usize,
    /// Successfully backfilled.
    pub backfilled: usize,
    /// Failed to resolve (TX expired, RPC error, etc.).
    pub failed: usize,
    /// Already had wallet_address.
    pub already_present: usize,
    /// Per-attendee details.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<BackfillDetail>,
}

#[derive(Debug, serde::Serialize)]
pub struct BackfillDetail {
    pub attendee_id: String,
    pub result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[worker::send]
pub async fn backfill_wallets_handler(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(body): Json<BackfillWalletsRequest>,
) -> Result<ApiOk<BackfillWalletsResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

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

    // Resolve event IDs to scan
    let event_ids: Vec<String> = match &body.event_id {
        Some(id) => vec![id.clone()],
        None => {
            let index = event_store::get_event_index(kv)
                .await
                .map_err(AppError::Internal)?;
            index.events.into_iter().map(|e| e.id).collect()
        }
    };

    let mut scanned = 0usize;
    let mut missing_wallet = 0usize;
    let mut backfilled = 0usize;
    let mut failed = 0usize;
    let mut already_present = 0usize;
    let mut details = Vec::new();

    for event_id in &event_ids {
        let deposits = event_store::list_deposit_statuses(kv, event_id)
            .await
            .map_err(AppError::Internal)?;

        for mut deposit in deposits {
            scanned += 1;

            // Already has wallet — skip
            if deposit.wallet_address.is_some() {
                already_present += 1;
                continue;
            }

            missing_wallet += 1;

            // Only USDC deposits with tx_signature can be backfilled
            let Some(tx_sig) = &deposit.tx_signature else {
                details.push(BackfillDetail {
                    attendee_id: deposit.attendee_id.clone(),
                    result: "skipped".to_string(),
                    wallet_address: None,
                    error: Some("no tx_signature — cannot resolve wallet".to_string()),
                });
                failed += 1;
                continue;
            };

            // Resolve wallet from on-chain TX
            match resolve_wallet_from_tx(&rpc_url, tx_sig).await {
                Ok(wallet) => {
                    tracing::info!(
                        attendee_id = %deposit.attendee_id,
                        event_id = %event_id,
                        wallet = %wallet,
                        "Backfilled wallet_address"
                    );
                    deposit.wallet_address = Some(wallet.clone());
                    event_store::save_deposit_status(kv, &deposit)
                        .await
                        .map_err(AppError::Internal)?;
                    backfilled += 1;
                    details.push(BackfillDetail {
                        attendee_id: deposit.attendee_id.clone(),
                        result: "backfilled".to_string(),
                        wallet_address: Some(wallet),
                        error: None,
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        attendee_id = %deposit.attendee_id,
                        tx_signature = %tx_sig,
                        error = %e,
                        "Failed to resolve wallet from TX"
                    );
                    failed += 1;
                    details.push(BackfillDetail {
                        attendee_id: deposit.attendee_id.clone(),
                        result: "failed".to_string(),
                        wallet_address: None,
                        error: Some(e),
                    });
                }
            }
        }
    }

    tracing::info!(
        scanned = %scanned,
        missing_wallet = %missing_wallet,
        backfilled = %backfilled,
        failed = %failed,
        already_present = %already_present,
        "Wallet backfill complete"
    );

    Ok(ApiOk::new(BackfillWalletsResponse {
        scanned,
        missing_wallet,
        backfilled,
        failed,
        already_present,
        details,
    }))
}

/// Resolve the attendee's wallet address from an on-chain deposit transaction.
///
/// Uses `getTransaction` RPC to fetch the TX and extracts the first account key
/// (which is the attendee/signer for deposit transactions built by this system).
async fn resolve_wallet_from_tx(rpc_url: &str, tx_signature: &str) -> Result<String, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-backfill",
        "method": "getTransaction",
        "params": [
            tx_signature,
            { "encoding": "json", "maxSupportedTransactionVersion": 0 }
        ]
    });

    let json_body = serde_json::to_string(&body)
        .map_err(|e| format!("failed to serialize getTransaction request: {e}"))?;

    let headers = worker::Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("failed to set header: {e:?}"))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = worker::Request::new_with_init(rpc_url, &init)
        .map_err(|e| format!("failed to create RPC request: {e:?}"))?;

    let mut response = worker::Fetch::Request(request)
        .send()
        .await
        .map_err(|e| format!("RPC request failed: {e:?}"))?;

    let body_text = response
        .text()
        .await
        .map_err(|e| format!("failed to read RPC response: {e:?}"))?;

    let parsed: serde_json::Value = serde_json::from_str(&body_text)
        .map_err(|e| format!("failed to parse RPC response: {e}"))?;

    // Check for RPC error
    if let Some(error) = parsed.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown");
        return Err(format!("RPC error: {msg}"));
    }

    // Navigate to result.transaction.message.accountKeys[0]
    let account_keys = parsed
        .get("result")
        .and_then(|r| r.get("transaction"))
        .and_then(|t| t.get("message"))
        .and_then(|m| m.get("accountKeys"))
        .and_then(|a| a.as_array())
        .ok_or_else(|| "TX not found or expired — account keys not available".to_string())?;

    let first_account = account_keys
        .first()
        .and_then(|k| k.as_str())
        .ok_or_else(|| "no account keys in transaction".to_string())?;

    // Validate it looks like a Solana pubkey (base58, ~32-44 chars)
    if first_account.len() < 32 || first_account.len() > 44 {
        return Err(format!("invalid pubkey length: {}", first_account.len()));
    }

    Ok(first_account.to_string())
}

// ---------------------------------------------------------------------------
// POST /api/escrow/deactivate-event
// ---------------------------------------------------------------------------

/// Request body for deactivate_event TX builder.
#[derive(serde::Deserialize)]
pub struct DeactivateEventTxRequest {
    pub event_id: String,
}

/// Response body for deactivate_event TX builder.
#[derive(serde::Serialize)]
pub struct DeactivateEventTxResponse {
    pub transaction: String,
    pub message: String,
}

/// Build a `deactivate_event` transaction for the organizer's wallet to sign.
///
/// Sets `is_active = false` on the event escrow, stopping new deposits.
/// Refunds are still allowed. After deactivation, `close_event` can reclaim rent.
#[worker::send]
pub async fn deactivate_event_tx_handler(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(body): Json<DeactivateEventTxRequest>,
) -> Result<ApiOk<DeactivateEventTxResponse>, WorkerError> {
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

    // Validate organizer wallet
    let organizer_pubkey = if event.organizer_wallet.is_empty() {
        return Err(AppError::Validation(
            "event has no organizer wallet configured — set organizer_wallet first".to_string(),
        )
        .into());
    } else {
        crate::solana::validate_wallet_address(&event.organizer_wallet)
            .map_err(|e| AppError::Validation(format!("invalid organizer_wallet: {e}")))?;
        &event.organizer_wallet
    };

    let on_chain_event_id = if event.on_chain_event_id != 0 {
        event.on_chain_event_id
    } else {
        derive_on_chain_event_id(&event.id)
    };

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

    let tx = crate::solana_escrow::build_deactivate_event_transaction(
        &rpc_url,
        Some(kv),
        organizer_pubkey,
        on_chain_event_id,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to build deactivate_event TX: {e}")))?;

    tracing::info!(
        event_id = %event.id,
        on_chain_event_id,
        "Deactivate event TX built for organizer"
    );

    Ok(ApiOk::new(DeactivateEventTxResponse {
        transaction: tx.transaction_b64,
        message: tx.message,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/escrow/close-event
// ---------------------------------------------------------------------------

/// Request body for close_event TX builder.
#[derive(serde::Deserialize)]
pub struct CloseEventTxRequest {
    pub event_id: String,
}

/// Response body for close_event TX builder.
#[derive(serde::Serialize)]
pub struct CloseEventTxResponse {
    pub transaction: String,
    pub message: String,
}

/// Build a `close_event` transaction for the organizer's wallet to sign.
///
/// Closes the event escrow and vault token account, reclaiming rent.
/// Requires event to be deactivated and vault to be empty (all funds
/// refunded or claimed as forfeited).
#[worker::send]
pub async fn close_event_tx_handler(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(body): Json<CloseEventTxRequest>,
) -> Result<ApiOk<CloseEventTxResponse>, WorkerError> {
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

    // Validate organizer wallet
    let organizer_pubkey = if event.organizer_wallet.is_empty() {
        return Err(AppError::Validation(
            "event has no organizer wallet configured — set organizer_wallet first".to_string(),
        )
        .into());
    } else {
        crate::solana::validate_wallet_address(&event.organizer_wallet)
            .map_err(|e| AppError::Validation(format!("invalid organizer_wallet: {e}")))?;
        &event.organizer_wallet
    };

    let on_chain_event_id = if event.on_chain_event_id != 0 {
        event.on_chain_event_id
    } else {
        derive_on_chain_event_id(&event.id)
    };

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

    let tx = crate::solana_escrow::build_close_event_transaction(
        &rpc_url,
        Some(kv),
        organizer_pubkey,
        on_chain_event_id,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to build close_event TX: {e}")))?;

    tracing::info!(
        event_id = %event.id,
        on_chain_event_id,
        "Close event TX built for organizer"
    );

    Ok(ApiOk::new(CloseEventTxResponse {
        transaction: tx.transaction_b64,
        message: tx.message,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/escrow/claim-forfeited
// ---------------------------------------------------------------------------

/// Request body for claim_forfeited TX builder.
#[derive(serde::Deserialize)]
pub struct ClaimForfeitedTxRequest {
    pub event_id: String,
}

/// Response body for claim_forfeited TX builder.
#[derive(serde::Serialize)]
pub struct ClaimForfeitedTxResponse {
    pub transaction: String,
    pub message: String,
}

/// Build a `claim_forfeited` transaction for the organizer's wallet to sign.
///
/// Transfers forfeited USDC (deposits from no-shows) from the vault to the
/// organizer's USDC token account. Only callable after refund_deadline.
#[worker::send]
pub async fn claim_forfeited_tx_handler(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(body): Json<ClaimForfeitedTxRequest>,
) -> Result<ApiOk<ClaimForfeitedTxResponse>, WorkerError> {
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

    // Validate organizer wallet
    let organizer_pubkey = if event.organizer_wallet.is_empty() {
        return Err(AppError::Validation(
            "event has no organizer wallet configured — set organizer_wallet first".to_string(),
        )
        .into());
    } else {
        crate::solana::validate_wallet_address(&event.organizer_wallet)
            .map_err(|e| AppError::Validation(format!("invalid organizer_wallet: {e}")))?;
        &event.organizer_wallet
    };

    let on_chain_event_id = if event.on_chain_event_id != 0 {
        event.on_chain_event_id
    } else {
        derive_on_chain_event_id(&event.id)
    };

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

    let tx = crate::solana_escrow::build_claim_forfeited_transaction(
        &rpc_url,
        Some(kv),
        organizer_pubkey,
        on_chain_event_id,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to build claim_forfeited TX: {e}")))?;

    tracing::info!(
        event_id = %event.id,
        on_chain_event_id,
        "Claim forfeited TX built for organizer"
    );

    Ok(ApiOk::new(ClaimForfeitedTxResponse {
        transaction: tx.transaction_b64,
        message: tx.message,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/escrow/close-deposit — Build close_deposit TX for attendee
// ---------------------------------------------------------------------------

/// Request body for building a close_deposit transaction.
#[derive(Debug, serde::Deserialize)]
pub struct CloseDepositTxRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// Attendee's Solana wallet address (base58).
    pub wallet_address: String,
}

/// Response with the serialized close_deposit transaction.
#[derive(Debug, serde::Serialize)]
pub struct CloseDepositTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet signs).
    pub transaction: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Build a close_deposit transaction for an attendee's verified USDC deposit.
///
/// This is a **public endpoint** — attendees call it to close their deposit PDAs
/// and reclaim rent lamports. The attendee's wallet signature provides on-chain authentication.
///
/// Prerequisites:
/// - Event has deposits enabled
/// - Organizer wallet is configured
/// - Attendee has a verified USDC deposit
#[worker::send]
pub async fn close_deposit_tx_handler(
    State(state): State<AppState>,
    Json(body): Json<CloseDepositTxRequest>,
) -> Result<ApiOk<CloseDepositTxResponse>, WorkerError> {
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

    // Validate wallet address
    crate::solana::validate_wallet_address(&body.wallet_address).map_err(AppError::Validation)?;

    // Check attendee has a verified USDC deposit
    let deposit_status = event_store::get_deposit_status(kv, &event.id, &body.attendee_id)
        .await
        .map_err(AppError::Internal)?;

    let status = deposit_status.ok_or_else(|| {
        AppError::NotFound(format!(
            "no deposit found for attendee '{}'",
            body.attendee_id
        ))
    })?;

    if !status.verified {
        return Err(
            AppError::Validation("deposit not verified yet — cannot close".to_string()).into(),
        );
    }

    if status.method != DepositMethod::Usdc {
        return Err(AppError::Validation(
            "close_deposit TX only supported for USDC deposits".to_string(),
        )
        .into());
    }

    // Determine organizer pubkey for PDA derivation
    let organizer_pubkey = if event.organizer_wallet.is_empty() {
        return Err(
            AppError::Internal("event has no organizer wallet configured".to_string()).into(),
        );
    } else {
        crate::solana::validate_wallet_address(&event.organizer_wallet)
            .map_err(|e| AppError::Validation(format!("invalid organizer_wallet: {e}")))?;
        &event.organizer_wallet
    };

    // The on_chain_event_id for PDA derivation
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

    // Build the close_deposit transaction
    let tx = crate::solana_escrow::build_close_deposit_transaction(
        &rpc_url,
        Some(kv),
        organizer_pubkey,
        on_chain_event_id,
        &body.wallet_address,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to build close_deposit TX: {e}")))?;

    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %event.id,
        "Close deposit TX built for attendee"
    );

    Ok(ApiOk::new(CloseDepositTxResponse {
        transaction: tx.transaction_b64,
        message: tx.message,
    }))
}
