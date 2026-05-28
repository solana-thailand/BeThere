use axum::{
    Extension, Json,
    extract::{Query, State},
};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::{DepositMethod, DepositStatus};
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::EscrowStatus;
use futures_util::stream::{self, StreamExt};

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

    // Log potential re-initialization — event had escrow before but was cleared
    if event.escrow_status == EscrowStatus::None
        && !event.organizer_wallet.is_empty()
        && event.escrow_address.is_empty()
    {
        tracing::warn!(
            event_id = %event.id,
            on_chain_event_id,
            organizer_wallet = %event.organizer_wallet,
            "escrow re-initialization detected — ensure previous escrow was fully closed on-chain"
        );
    }

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

    // Collision detection: verify the escrow PDA doesn't already exist on-chain
    // before building the transaction to avoid AccountAlreadyInitialized errors.
    crate::solana_escrow::check_escrow_pda_available(&rpc_url, organizer_pubkey, on_chain_event_id)
        .await
        .map_err(|e| {
            AppError::Validation(format!(
                "escrow PDA collision: {e}. \
         Use a different on_chain_event_id or close the existing escrow first."
            ))
        })?;

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
        state.d1.as_deref(),
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
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?
        .clone();

    let rpc_url = state.config.solana.full_rpc_url();

    // Resolve event IDs to scan
    let event_ids: Vec<String> = match &body.event_id {
        Some(id) => vec![id.clone()],
        None => {
            let index = event_store::get_event_index(&kv)
                .await
                .map_err(AppError::Internal)?;
            index.events.into_iter().map(|e| e.id).collect()
        }
    };

    // Phase 1: Collect all deposits that need wallet resolution (fast — KV reads)
    let mut to_resolve: Vec<(String, DepositStatus)> = Vec::new(); // (event_id, deposit)
    let mut scanned = 0usize;
    let mut already_present = 0usize;
    let mut skipped_details = Vec::new();

    for event_id in &event_ids {
        let deposits = event_store::list_deposit_statuses(&kv, event_id)
            .await
            .map_err(AppError::Internal)?;

        for deposit in deposits {
            scanned += 1;

            if deposit.wallet_address.is_some() {
                already_present += 1;
                continue;
            }

            // Only USDC deposits with tx_signature can be backfilled
            if deposit.tx_signature.is_none() {
                skipped_details.push(BackfillDetail {
                    attendee_id: deposit.attendee_id.clone(),
                    result: "skipped".to_string(),
                    wallet_address: None,
                    error: Some("no tx_signature — cannot resolve wallet".to_string()),
                });
                continue;
            }

            to_resolve.push((event_id.clone(), deposit));
        }
    }

    let missing_wallet = to_resolve.len();

    // Phase 2: Resolve wallets concurrently (bounded to 5 parallel RPC calls)
    let resolve_futures = to_resolve.into_iter().map(|(event_id, deposit)| {
        let rpc_url = rpc_url.clone();
        async move {
            let tx_sig = deposit.tx_signature.as_deref().unwrap();
            let result = resolve_wallet_from_tx(&rpc_url, tx_sig).await;
            (event_id, deposit, result)
        }
    });

    let resolved: Vec<(String, DepositStatus, Result<String, String>)> =
        stream::iter(resolve_futures)
            .buffer_unordered(5)
            .collect()
            .await;

    // Phase 3: Process results — save backfilled wallets
    let mut backfilled = 0usize;
    let mut failed = 0usize;
    let mut details: Vec<BackfillDetail> = skipped_details;

    for (event_id, mut deposit, result) in resolved {
        match result {
            Ok(wallet) => {
                tracing::info!(
                    attendee_id = %deposit.attendee_id,
                    event_id = %event_id,
                    wallet = %wallet,
                    "Backfilled wallet_address"
                );
                deposit.wallet_address = Some(wallet.clone());
                if let Err(e) = event_store::save_deposit_status(&kv, &deposit).await {
                    tracing::warn!(
                        attendee_id = %deposit.attendee_id,
                        error = %e,
                        "Failed to save backfilled wallet"
                    );
                    failed += 1;
                    details.push(BackfillDetail {
                        attendee_id: deposit.attendee_id.clone(),
                        result: "save_failed".to_string(),
                        wallet_address: Some(wallet),
                        error: Some(e),
                    });
                } else {
                    backfilled += 1;
                    details.push(BackfillDetail {
                        attendee_id: deposit.attendee_id.clone(),
                        result: "backfilled".to_string(),
                        wallet_address: Some(wallet),
                        error: None,
                    });
                }
            }
            Err(e) => {
                tracing::warn!(
                    attendee_id = %deposit.attendee_id,
                    tx_signature = ?deposit.tx_signature,
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
        state.d1.as_deref(),
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
        state.d1.as_deref(),
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

/// Build a batch `claim_forfeited` transaction for the organizer's wallet to sign.
///
/// Looks up all USDC deposits for this event, excludes checked-in and already-refunded
/// attendees (via on-chain events), and builds a multi-instruction TX to claim
/// forfeited deposits from all no-shows in one transaction.
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

    // --- Collect all USDC deposits with wallet addresses ---
    let all_deposits = event_store::list_deposit_statuses(kv, &body.event_id)
        .await
        .map_err(AppError::Internal)?;

    let usdc_wallets: Vec<String> = all_deposits
        .iter()
        .filter(|d| d.method == DepositMethod::Usdc)
        .filter_map(|d| d.wallet_address.clone())
        .collect();

    if usdc_wallets.is_empty() {
        return Err(
            AppError::Validation("no USDC deposits found for this event".to_string()).into(),
        );
    }

    // --- Exclude checked-in and refunded attendees via on-chain events ---
    let onchain_events = crate::escrow_indexer::get_onchain_events(kv, &body.event_id, 200).await;
    let mut excluded_wallets: std::collections::HashSet<String> = std::collections::HashSet::new();
    for ev in &onchain_events {
        match ev.instruction {
            crate::escrow_indexer::EscrowInstruction::MarkCheckedIn
            | crate::escrow_indexer::EscrowInstruction::Refund => {
                if let Some(ref attendee) = ev.attendee {
                    excluded_wallets.insert(attendee.clone());
                }
            }
            crate::escrow_indexer::EscrowInstruction::ClaimForfeited => {
                // Already claimed — nothing to do (attendee field is None for claim_forfeited)
            }
            _ => {}
        }
    }

    let forfeited: Vec<String> = usdc_wallets
        .into_iter()
        .filter(|w| !excluded_wallets.contains(w))
        .collect();

    if forfeited.is_empty() {
        return Err(AppError::Validation(
            "no forfeited deposits to claim — all attendees checked in or refunded".to_string(),
        )
        .into());
    }

    tracing::info!(
        event_id = %event.id,
        on_chain_event_id,
        forfeited_count = forfeited.len(),
        "Building batch claim_forfeited TX"
    );

    let tx = crate::solana_escrow::build_batch_claim_forfeited_transaction(
        &rpc_url,
        Some(kv),
        organizer_pubkey,
        on_chain_event_id,
        &forfeited,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to build batch claim_forfeited TX: {e}")))?;

    tracing::info!(
        event_id = %event.id,
        on_chain_event_id,
        forfeited_count = forfeited.len(),
        "Batch claim forfeited TX built for organizer"
    );

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::ClaimForfeited,
            &event.id,
            &format!(
                "batch claim forfeited TX built for {} attendee(s)",
                forfeited.len()
            ),
        ),
        state.d1.as_deref(),
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

    let (deposits_res, thb_res) = futures_util::join!(
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

// ---------------------------------------------------------------------------
// POST /api/escrow/confirm-init — Confirm escrow initialized on-chain
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/confirm-init.
#[derive(Debug, serde::Deserialize)]
pub struct ConfirmEscrowInitRequest {
    /// Event ID (KV key).
    pub event_id: String,
}

/// Response for escrow init confirmation.
#[derive(Debug, serde::Serialize)]
pub struct ConfirmEscrowInitResponse {
    /// Derived escrow PDA address (base58).
    pub escrow_address: String,
    /// On-chain event ID used for PDA derivation.
    pub on_chain_event_id: u64,
    /// Confirmed escrow status.
    pub escrow_status: EscrowStatus,
}

/// Confirm that an escrow has been initialized on-chain and persist the state.
///
/// Called by the frontend after the wallet confirms the init TX.
/// Also serves as a recovery endpoint — can be called anytime to sync on-chain state.
/// Idempotent: if the event already has `escrow_status=Initialized` and the
/// derived address matches, returns success without re-saving.
#[worker::send]
pub async fn confirm_escrow_init_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<ConfirmEscrowInitRequest>,
) -> Result<ApiOk<ConfirmEscrowInitResponse>, WorkerError> {
    use event_checkin_domain::models::event::UpdateEventRequest;

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    // 1. Resolve event with access check
    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, Some(&body.event_id))
            .await?;

    if !event.deposit_enabled {
        return Err(AppError::Validation("deposit not enabled for this event".to_string()).into());
    }

    // 2. Derive on_chain_event_id (same logic as init_escrow_tx_handler)
    let on_chain_event_id = if event.on_chain_event_id != 0 {
        event.on_chain_event_id
    } else {
        super::derive_on_chain_event_id(&event.id)
    };

    // 3. Validate organizer wallet
    let organizer_pubkey = if event.organizer_wallet.is_empty() {
        return Err(
            AppError::Validation("event has no organizer wallet configured".to_string()).into(),
        );
    } else {
        crate::solana::validate_wallet_address(&event.organizer_wallet)
            .map_err(|e| AppError::Validation(format!("invalid organizer_wallet: {e}")))?;
        &event.organizer_wallet
    };

    // 4. Derive escrow PDA address
    let escrow_address =
        crate::solana_escrow::derive_escrow_address(organizer_pubkey, on_chain_event_id)
            .await
            .map_err(|e| AppError::Internal(format!("failed to derive escrow address: {e}")))?;

    // 5. Verify escrow exists on-chain
    let rpc_url = state.config.solana.full_rpc_url();
    crate::solana_escrow::verify_escrow_account_exists(
        &rpc_url,
        organizer_pubkey,
        on_chain_event_id,
    )
    .await
    .map_err(|e| AppError::Validation(format!("escrow not found on-chain: {e}")))?;

    tracing::info!(
        event_id = %event.id,
        escrow_address = %escrow_address,
        on_chain_event_id,
        "escrow confirmed on-chain"
    );

    // 6. Idempotent check: if already persisted with matching address, skip update
    let already_persisted = event.escrow_status == EscrowStatus::Initialized
        && !event.escrow_address.is_empty()
        && event.escrow_address == escrow_address;

    if !already_persisted {
        let update_req = UpdateEventRequest {
            escrow_address: Some(escrow_address.clone()),
            on_chain_event_id: Some(on_chain_event_id),
            escrow_status: Some(EscrowStatus::Initialized),
            name: None,
            slug: None,
            tagline: None,
            link: None,
            status: None,
            event_start_ms: None,
            event_end_ms: None,
            time_tba: None,
            sheet_id: None,
            sheet_name: None,
            staff_sheet_name: None,
            quiz_enabled: None,
            nft_collection_mint: None,
            nft_metadata_uri: None,
            nft_image_url: None,
            nft_name_template: None,
            nft_symbol: None,
            nft_description_template: None,
            merkle_tree: None,
            organization_id: None,
            organizer_emails: None,
            staff_emails: None,
            claim_base_url: None,
            deposit_enabled: None,
            deposit_amount_usdc: None,
            deposit_amount_thb: None,
            promptpay_id: None,
            organizer_wallet: None,
            refund_deadline_hours: None,
            max_refundable_deposits: None,
            expected_updated_at: None,
            description: None,
            location: None,
            video_url: None,
            event_format: None,
            require_contact_info: None,
            in_person_capacity: None,
            online_capacity: None,
            online_open_mode: None,
            online_registration_open: None,
            deposit_deadline_hours: None,
            visibility: None,
        };

        event_store::update_event(kv, &event.id, &update_req, &claims.email)
            .await
            .map_err(|e| AppError::Internal(format!("failed to persist escrow state: {e}")))?;

        // Audit log
        let _ = crate::audit_store::append_event_audit(
            kv,
            &event.id,
            crate::audit_store::create_entry(
                &claims.email,
                crate::audit_store::AuditAction::EscrowInitialized,
                &event.id,
                "escrow init confirmed on-chain and persisted server-side",
            ),
            state.d1.as_deref(),
        )
        .await;
    } else {
        tracing::debug!(
            event_id = %event.id,
            "escrow already persisted with matching address — skipping update"
        );
    }

    Ok(ApiOk::new(ConfirmEscrowInitResponse {
        escrow_address,
        on_chain_event_id,
        escrow_status: EscrowStatus::Initialized,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/escrow/health — Escrow health check (KV vs on-chain comparison)
// ---------------------------------------------------------------------------

/// Response for the escrow health check endpoint.
#[derive(Debug, serde::Serialize)]
pub struct EscrowHealthResponse {
    /// Event ID.
    pub event_id: String,
    /// Server-side escrow status from KV.
    pub kv_escrow_status: String,
    /// Server-side escrow address from KV.
    pub kv_escrow_address: String,
    /// Server-side on-chain event ID from KV.
    pub kv_on_chain_event_id: u64,
    /// Server-side organizer wallet from KV.
    pub kv_organizer_wallet: String,
    /// Whether the escrow account exists on-chain.
    pub on_chain_exists: bool,
    /// Derived escrow PDA address (if derivable).
    pub derived_escrow_address: Option<String>,
    /// Whether KV and on-chain states are consistent.
    pub consistent: bool,
    /// Human-readable diagnosis.
    pub diagnosis: String,
}

/// GET /api/escrow/health?event_id=xxx
///
/// Compares server-side (KV) escrow state with on-chain reality.
/// Returns a health report showing whether the two are in sync.
///
/// **Requires**: SuperAdmin or Organizer role.
#[worker::send]
pub async fn escrow_health_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<ApiOk<EscrowHealthResponse>, WorkerError> {
    use event_checkin_domain::models::event::EscrowStatus;

    let event_id = params
        .get("event_id")
        .ok_or_else(|| AppError::Validation("missing event_id query parameter".to_string()))?;

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event = crate::event_store::get_event_config(kv, event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("event '{event_id}' not found")))?;

    // Access control
    let is_organizer = event
        .organizer_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(&claims.email));
    let is_super_admin = state
        .config
        .super_admin_emails
        .contains(&claims.email.to_lowercase());

    if !is_organizer && !is_super_admin {
        return Err(AppError::Forbidden(
            "only organizers or super admins can check escrow health".to_string(),
        )
        .into());
    }

    let kv_status = event.escrow_status.as_str().to_string();
    let kv_address = event.escrow_address.clone();
    let kv_event_id = event.on_chain_event_id;
    let kv_wallet = event.organizer_wallet.clone();

    // Derive on-chain info if possible
    let on_chain_id = if kv_event_id != 0 {
        kv_event_id
    } else {
        super::derive_on_chain_event_id(&event.id)
    };

    let mut derived_address: Option<String> = None;
    let mut on_chain_exists = false;

    if !kv_wallet.is_empty() {
        // Derive PDA address
        match crate::solana_escrow::derive_escrow_address(&kv_wallet, on_chain_id).await {
            Ok(addr) => {
                derived_address = Some(addr.clone());
                // Check on-chain existence
                let rpc_url = state.config.solana.full_rpc_url();
                match crate::solana_escrow::verify_escrow_account_exists(
                    &rpc_url,
                    &kv_wallet,
                    on_chain_id,
                )
                .await
                {
                    Ok(()) => on_chain_exists = true,
                    Err(_) => on_chain_exists = false,
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to derive escrow address for health check");
            }
        }
    }

    // Determine consistency and diagnosis
    let (consistent, diagnosis) = match (&event.escrow_status, on_chain_exists) {
        (EscrowStatus::None, false) => (true, "healthy: no escrow on server, no escrow on-chain".to_string()),
        (EscrowStatus::None, true) => (false, "DRIFT: server says None but escrow exists on-chain. Escrow may have been initialized outside the UI or server state was reset while on-chain escrow is still active.".to_string()),
        (EscrowStatus::Initialized, true) => (true, "healthy: escrow initialized on server and active on-chain".to_string()),
        (EscrowStatus::Initialized, false) => (false, "DRIFT: server says Initialized but escrow not found on-chain. Escrow may have been closed without updating server, or init TX failed.".to_string()),
        (EscrowStatus::Deactivated, true) => (true, "healthy: escrow deactivated on server and still exists on-chain (refunds/claims in progress)".to_string()),
        (EscrowStatus::Deactivated, false) => (false, "DRIFT: server says Deactivated but escrow not found on-chain. Escrow may have been closed without updating server.".to_string()),
        (EscrowStatus::Closed, false) => (true, "healthy: escrow closed on server and account gone from chain".to_string()),
        (EscrowStatus::Closed, true) => (false, "DRIFT: server says Closed but escrow still exists on-chain. Close TX may have failed. DO NOT re-initialize until on-chain escrow is fully closed.".to_string()),
        (EscrowStatus::Cancelled, false) => (true, "healthy: escrow cancelled on server and account gone from chain".to_string()),
        (EscrowStatus::Cancelled, true) => (false, "DRIFT: server says Cancelled but escrow still exists on-chain.".to_string()),
    };

    tracing::info!(
        event_id = %event.id,
        kv_status = %kv_status,
        on_chain_exists,
        consistent,
        "escrow health check completed"
    );

    Ok(ApiOk::new(EscrowHealthResponse {
        event_id: event.id,
        kv_escrow_status: kv_status,
        kv_escrow_address: kv_address,
        kv_on_chain_event_id: kv_event_id,
        kv_organizer_wallet: kv_wallet,
        on_chain_exists,
        derived_escrow_address: derived_address,
        consistent,
        diagnosis,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/escrow/rollover-deposit — Roll deposit to next event
// ---------------------------------------------------------------------------

/// Request body for rollover deposit transaction.
#[derive(Debug, serde::Deserialize)]
pub struct RolloverDepositTxRequest {
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
#[derive(Debug, serde::Serialize)]
pub struct RolloverDepositTxResponse {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction: String,
    /// Human-readable message.
    pub message: String,
}

/// Build a rollover_deposit transaction for an attendee.
///
/// The attendee signs the transaction, which atomically moves their USDC
/// deposit from the source event vault to the target event vault.
///
/// **Prerequisites** (validated on-chain by the program):
/// - Attendee was checked in on the source event
/// - Source and target events have the same organizer
/// - Source and target events have the same deposit amount
/// - Target event is active (accepting deposits)
///
/// **Auth**: Attendee-authenticated (JWT identity must match the attendee).
#[worker::send]
pub async fn rollover_deposit_tx_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<RolloverDepositTxRequest>,
) -> Result<ApiOk<RolloverDepositTxResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    // Resolve source event
    let source_event = event_store::get_event_config(kv, &body.source_event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| {
            AppError::NotFound(format!("source event '{}' not found", body.source_event_id))
        })?;

    // Resolve target event
    let target_event = event_store::get_event_config(kv, &body.target_event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| {
            AppError::NotFound(format!("target event '{}' not found", body.target_event_id))
        })?;

    // Both events must have deposits enabled
    if !source_event.deposit_enabled || !target_event.deposit_enabled {
        return Err(AppError::Validation(
            "both source and target events must have deposits enabled".to_string(),
        )
        .into());
    }

    // Both events must have escrow initialized
    if source_event.escrow_address.is_empty() || target_event.escrow_address.is_empty() {
        return Err(AppError::Validation(
            "both events must have escrow initialized on-chain".to_string(),
        )
        .into());
    }

    // Same organizer required
    if source_event.organizer_wallet != target_event.organizer_wallet {
        return Err(AppError::Validation(
            "source and target events must have the same organizer".to_string(),
        )
        .into());
    }

    // Validate wallet address
    crate::solana::validate_wallet_address(&body.wallet_address).map_err(AppError::Validation)?;

    // Verify attendee has a verified USDC deposit on source event
    let deposit_status = event_store::get_deposit_status(kv, &source_event.id, &body.attendee_id)
        .await
        .map_err(AppError::Internal)?;

    let status = deposit_status.ok_or_else(|| {
        AppError::NotFound(format!(
            "no deposit found for attendee '{}' on source event",
            body.attendee_id
        ))
    })?;

    if !status.verified {
        return Err(AppError::Validation(
            "source deposit not verified — cannot rollover".to_string(),
        )
        .into());
    }

    if status.method != DepositMethod::Usdc {
        return Err(
            AppError::Validation("rollover only supported for USDC deposits".to_string()).into(),
        );
    }

    // Verify no existing deposit on target event
    let target_deposit = event_store::get_deposit_status(kv, &target_event.id, &body.attendee_id)
        .await
        .map_err(AppError::Internal)?;

    if target_deposit.is_some() {
        return Err(AppError::Validation(
            "attendee already has a deposit on the target event".to_string(),
        )
        .into());
    }

    // Derive on-chain event IDs
    let source_on_chain_id = if source_event.on_chain_event_id != 0 {
        source_event.on_chain_event_id
    } else {
        super::derive_on_chain_event_id(&source_event.id)
    };

    let target_on_chain_id = if target_event.on_chain_event_id != 0 {
        target_event.on_chain_event_id
    } else {
        super::derive_on_chain_event_id(&target_event.id)
    };

    // Organizer pubkey for PDA derivation
    let organizer_pubkey = if source_event.organizer_wallet.is_empty() {
        return Err(AppError::Internal(
            "source event has no organizer wallet configured".to_string(),
        )
        .into());
    } else {
        crate::solana::validate_wallet_address(&source_event.organizer_wallet)
            .map_err(|e| AppError::Validation(format!("invalid organizer_wallet: {e}")))?;
        &source_event.organizer_wallet
    };

    let rpc_url = state.config.solana.full_rpc_url();

    let tx = crate::solana_escrow::build_rollover_deposit_transaction(
        &rpc_url,
        Some(kv),
        organizer_pubkey,
        source_on_chain_id,
        target_on_chain_id,
        &body.wallet_address,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to build rollover TX: {e}")))?;

    tracing::info!(
        attendee_id = %body.attendee_id,
        source_event_id = %source_event.id,
        target_event_id = %target_event.id,
        "Rollover deposit TX built for attendee"
    );

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &source_event.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::EscrowRolloverInitiated,
            &body.attendee_id,
            &format!("rollover deposit to event {}", target_event.id),
        ),
        state.d1.as_deref(),
    )
    .await;

    Ok(ApiOk::new(RolloverDepositTxResponse {
        transaction: tx.transaction_b64,
        message: tx.message,
    }))
}
