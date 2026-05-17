use axum::{
    Extension, Json,
    extract::{Query, State},
};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::DepositMethod;
use event_checkin_domain::models::error::AppError;

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::handlers::ext::EventIdQuery;
use crate::state::AppState;

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
    Extension(claims): Extension<Claims>,
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
        super::derive_on_chain_event_id(&event.id)
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

    let rpc_url = state.config.solana.full_rpc_url();

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

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::EscrowInitialized,
            &event.id,
            "escrow PDA initialization TX built",
        ),
    )
    .await;

    Ok(ApiOk::new(InitEscrowTxResponse {
        transaction: tx.transaction_b64,
        message: tx.message,
        escrow_address: tx.escrow_address,
        vault_address: tx.vault_address,
        on_chain_event_id,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/escrow/refund — Build refund TX for attendee
// ---------------------------------------------------------------------------

/// Request body for building a refund transaction.
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
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
#[allow(dead_code)]
pub struct RefundTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet signs).
    pub transaction: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Request for combined refund + close_deposit transaction.
#[derive(serde::Deserialize)]
pub struct RefundAndCloseTxRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// Attendee's Solana wallet address (base58).
    pub wallet_address: String,
}

/// Response with the serialized combined refund+close_deposit transaction.
#[derive(Debug, serde::Serialize)]
pub struct RefundAndCloseTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet signs).
    pub transaction: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Build a combined refund + close_deposit transaction for an attendee's verified USDC deposit.
///
/// This is a **public endpoint** — attendees call it to claim their refund AND reclaim rent
/// in a single atomic transaction. One wallet signature does both.
///
/// Prerequisites:
/// - Event has deposits enabled and escrow initialized
/// - Attendee has a verified USDC deposit
#[worker::send]
pub async fn refund_and_close_tx_handler(
    State(state): State<AppState>,
    Json(body): Json<RefundAndCloseTxRequest>,
) -> Result<ApiOk<RefundAndCloseTxResponse>, WorkerError> {
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

    // Check if this deposit is in the refundable tier
    if !status.refundable {
        return Err(AppError::Validation(
            "your deposit is non-refundable (overflow tier) — refunds are only available for the first N depositors".to_string(),
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
        super::derive_on_chain_event_id(&event.id)
    };

    // Build the RPC URL with API key
    let rpc_url = state.config.solana.full_rpc_url();

    // Build the combined refund + close_deposit transaction
    let tx = crate::solana_escrow::build_refund_and_close_transaction(
        &rpc_url,
        Some(kv),
        organizer_pubkey,
        on_chain_event_id,
        &body.wallet_address,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to build refund+close TX: {e}")))?;

    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %event.id,
        "Refund+Close TX built for attendee"
    );

    Ok(ApiOk::new(RefundAndCloseTxResponse {
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

    // Check if attendee is in the refundable tier
    let deposit_status = event_store::get_deposit_status(kv, &event.id, &body.attendee_id)
        .await
        .map_err(AppError::Internal)?;

    let is_refundable = deposit_status
        .as_ref()
        .map(|d| d.refundable)
        .unwrap_or(true); // default to refundable if no record

    if !is_refundable {
        // Non-refundable tier: don't build on-chain check-in TX.
        // Check-in is tracked off-chain only — their deposit is automatically forfeited.
        return Err(AppError::Validation(
            "attendee is in non-refundable tier — no on-chain check-in needed. Deposit is automatically forfeited.".to_string(),
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

    let on_chain_event_id = if event.on_chain_event_id != 0 {
        event.on_chain_event_id
    } else {
        super::derive_on_chain_event_id(&event.id)
    };

    // Build the RPC URL with API key
    let rpc_url = state.config.solana.full_rpc_url();

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

    let rpc_url = state.config.solana.full_rpc_url();

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
    Extension(claims): Extension<Claims>,
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

    // Check escrow status — only initialized escrows can be deactivated
    use event_checkin_domain::models::event::EscrowStatus;
    if event.escrow_status != EscrowStatus::Initialized {
        return Err(AppError::Validation(format!(
            "escrow is not in initialized state (current: {}) — cannot deactivate",
            event.escrow_status
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

    let on_chain_event_id = if event.on_chain_event_id != 0 {
        event.on_chain_event_id
    } else {
        super::derive_on_chain_event_id(&event.id)
    };

    let rpc_url = state.config.solana.full_rpc_url();

    // Verify escrow exists on-chain (catches stale KV state)
    crate::solana_escrow::verify_escrow_account_exists(
        &rpc_url,
        organizer_pubkey,
        on_chain_event_id,
    )
    .await
    .map_err(|e| {
        AppError::Validation(format!(
            "escrow account verification failed: {e} — refresh page and try again"
        ))
    })?;

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

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::EscrowDeactivated,
            &event.id,
            "escrow deactivation TX built",
        ),
    )
    .await;

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
    Extension(claims): Extension<Claims>,
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

    // Check escrow status — only deactivated escrows can be closed
    use event_checkin_domain::models::event::EscrowStatus;
    if event.escrow_status != EscrowStatus::Deactivated {
        return Err(AppError::Validation(format!(
            "escrow is not in deactivated state (current: {}) — deactivate first before closing",
            event.escrow_status
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

    let on_chain_event_id = if event.on_chain_event_id != 0 {
        event.on_chain_event_id
    } else {
        super::derive_on_chain_event_id(&event.id)
    };

    let rpc_url = state.config.solana.full_rpc_url();

    // Verify escrow exists on-chain (catches stale KV state)
    crate::solana_escrow::verify_escrow_account_exists(
        &rpc_url,
        organizer_pubkey,
        on_chain_event_id,
    )
    .await
    .map_err(|e| {
        AppError::Validation(format!(
            "escrow account verification failed: {e} — refresh page and try again"
        ))
    })?;

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

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::EscrowClosed,
            &event.id,
            "escrow close TX built",
        ),
    )
    .await;

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
    Extension(claims): Extension<Claims>,
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

    // Check escrow status — forfeited claims only allowed in deactivated state
    use event_checkin_domain::models::event::EscrowStatus as Es;
    if event.escrow_status != Es::Deactivated {
        return Err(AppError::Validation(format!(
            "escrow must be deactivated before claiming forfeited (current: {})",
            event.escrow_status
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

    let on_chain_event_id = if event.on_chain_event_id != 0 {
        event.on_chain_event_id
    } else {
        super::derive_on_chain_event_id(&event.id)
    };

    let rpc_url = state.config.solana.full_rpc_url();

    // Verify escrow exists on-chain (catches stale KV state)
    crate::solana_escrow::verify_escrow_account_exists(
        &rpc_url,
        organizer_pubkey,
        on_chain_event_id,
    )
    .await
    .map_err(|e| {
        AppError::Validation(format!(
            "escrow account verification failed: {e} — refresh page and try again"
        ))
    })?;

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

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::ClaimForfeited,
            &event.id,
            "claim forfeited TX built",
        ),
    )
    .await;

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
        super::derive_on_chain_event_id(&event.id)
    };

    // Build the RPC URL with API key
    let rpc_url = state.config.solana.full_rpc_url();

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

// ---------------------------------------------------------------------------
// GET /api/escrow/refund-queue (admin)
// ---------------------------------------------------------------------------

/// USDC deposit queue item for cancellation workflow.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UsdcQueueItem {
    pub attendee_id: String,
    pub wallet_address: Option<String>,
    pub amount: u64,
    pub deposited_at: String,
}

/// List USDC deposits eligible for refund (cancellation workflow).
/// These require attendee-signed refund transactions — organizer cannot force-refund.
#[worker::send]
pub async fn usdc_refund_queue_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, query.event_id.as_deref())
            .await?;

    let deposits = event_store::list_deposit_statuses(kv, &event.id)
        .await
        .map_err(AppError::Internal)?;

    let usdc_queue: Vec<UsdcQueueItem> = deposits
        .iter()
        .filter(|d| {
            d.method == DepositMethod::Usdc
                && d.verified
                && d.wallet_address.is_some()
                && d.refundable
        })
        .map(|d| UsdcQueueItem {
            attendee_id: d.attendee_id.clone(),
            wallet_address: d.wallet_address.clone(),
            amount: d.amount,
            deposited_at: d.deposited_at.clone(),
        })
        .collect();

    Ok(ApiOk::new(serde_json::json!({
        "event_id": event.id,
        "usdc_pending": usdc_queue.len(),
        "queue": usdc_queue,
    })))
}

// ---------------------------------------------------------------------------
// GET /api/escrow/cancel-status (admin)
// ---------------------------------------------------------------------------

/// Get event cancellation status — counts of deposits, refunds, etc.
#[worker::send]
pub async fn cancel_status_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, query.event_id.as_deref())
            .await?;

    let (deposits_res, thb_res) = futures::join!(
        event_store::list_deposit_statuses(kv, &event.id),
        event_store::list_thb_deposits(kv, &event.id),
    );
    let deposits = deposits_res.map_err(AppError::Internal)?;
    let thb_deposits = thb_res.map_err(AppError::Internal)?;

    let usdc_total = deposits
        .iter()
        .filter(|d| d.method == DepositMethod::Usdc)
        .count();
    let usdc_verified = deposits
        .iter()
        .filter(|d| d.method == DepositMethod::Usdc && d.verified)
        .count();
    let usdc_refundable = deposits
        .iter()
        .filter(|d| d.method == DepositMethod::Usdc && d.verified && d.refundable)
        .count();
    let thb_total = thb_deposits.len();
    let thb_refunded = thb_deposits.iter().filter(|d| d.refunded).count();

    Ok(ApiOk::new(serde_json::json!({
        "event_id": event.id,
        "event_name": event.name,
        "escrow_status": format!("{}", event.escrow_status),
        "usdc_deposits": usdc_total,
        "usdc_verified": usdc_verified,
        "usdc_refundable": usdc_refundable,
        "thb_deposits": thb_total,
        "thb_refunded": thb_refunded,
        "thb_pending_refund": thb_total.saturating_sub(thb_refunded),
    })))
}
