use axum::{Extension, Json, extract::State};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::DepositMethod;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::EscrowStatus;

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::state::AppState;

use super::types::*;
use super::workflows;

// ---------------------------------------------------------------------------
// POST /api/escrow/init — Combined ATA + CreateEvent in one TX
// ---------------------------------------------------------------------------

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

/// Build a combined refund + close_deposit transaction for an attendee's verified USDC deposit.
///
/// This is a **public endpoint** — attendees call it to claim their refund AND reclaim rent
/// in a single atomic transaction. One wallet signature does both.
#[worker::send]
pub async fn refund_and_close_tx_handler(
    State(state): State<AppState>,
    Json(body): Json<RefundAndCloseTxRequest>,
) -> Result<ApiOk<RefundAndCloseTxResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;
    let d1 = state.d1.as_deref();

    let event = event_store::get_event_config(kv, &body.event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("event '{}' not found", body.event_id)))?;

    if !event.deposit_enabled {
        return Err(AppError::Validation("deposit not enabled for this event".to_string()).into());
    }

    if event.escrow_address.is_empty() {
        return Err(
            AppError::Validation("event escrow not initialized on-chain".to_string()).into(),
        );
    }

    crate::solana::validate_wallet_address(&body.wallet_address).map_err(AppError::Validation)?;

    let deposit_status = event_store::get_deposit_status(kv, &event.id, &body.attendee_id, d1)
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

    if !status.refundable {
        return Err(AppError::Validation(
            "your deposit is non-refundable (overflow tier) — refunds are only available for the first N depositors".to_string(),
        )
        .into());
    }

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

    let rpc_url = state.config.solana.full_rpc_url();

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

/// Build a `mark_checked_in` transaction for the escrow program.
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
    let d1 = state.d1.as_deref();

    let event = event_store::get_event_config(kv, &body.event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("event '{}' not found", body.event_id)))?;

    if !event.deposit_enabled {
        return Err(AppError::Validation("deposit not enabled for this event".to_string()).into());
    }

    if event.escrow_address.is_empty() {
        return Err(
            AppError::Validation("event escrow not initialized on-chain".to_string()).into(),
        );
    }

    let attendee_wallet = match &body.attendee_wallet {
        Some(w) if !w.is_empty() => w.clone(),
        _ => {
            let deposit = event_store::get_deposit_status(kv, &event.id, &body.attendee_id, d1)
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

    crate::solana::validate_wallet_address(&attendee_wallet).map_err(AppError::Validation)?;

    let deposit_status = event_store::get_deposit_status(kv, &event.id, &body.attendee_id, d1)
        .await
        .map_err(AppError::Internal)?;

    let is_refundable = deposit_status
        .as_ref()
        .map(|d| d.refundable)
        .unwrap_or(true);

    if !is_refundable {
        return Err(AppError::Validation(
            "attendee is in non-refundable tier — no on-chain check-in needed. Deposit is automatically forfeited.".to_string(),
        )
        .into());
    }

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

    let event_ids: Vec<String> = match &body.event_id {
        Some(id) => vec![id.clone()],
        None => {
            let index = event_store::get_event_index(&kv)
                .await
                .map_err(AppError::Internal)?;
            index.events.into_iter().map(|e| e.id).collect()
        }
    };

    let result = workflows::run_backfill_workflow(&kv, &rpc_url, &event_ids).await;

    Ok(ApiOk::new(BackfillWalletsResponse {
        scanned: result.scanned,
        missing_wallet: result.missing_wallet,
        backfilled: result.backfilled,
        failed: result.failed,
        already_present: result.already_present,
        details: result.details,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/escrow/deactivate-event
// ---------------------------------------------------------------------------

/// Build a `deactivate_event` transaction for the organizer's wallet to sign.
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

    if event.escrow_status != EscrowStatus::Initialized {
        return Err(AppError::Validation(format!(
            "escrow is not in initialized state (current: {}) — cannot deactivate",
            event.escrow_status
        ))
        .into());
    }

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

/// Build a `close_event` transaction for the organizer's wallet to sign.
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

    if event.escrow_status != EscrowStatus::Deactivated {
        return Err(AppError::Validation(format!(
            "escrow is not in deactivated state (current: {}) — deactivate first before closing",
            event.escrow_status
        ))
        .into());
    }

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

/// Build a batch `claim_forfeited` transaction for the organizer's wallet to sign.
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
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    let event = event_store::get_event_config(kv, &body.event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("event '{}' not found", body.event_id)))?;

    if !event.deposit_enabled {
        return Err(AppError::Validation("deposit not enabled for this event".to_string()).into());
    }

    use event_checkin_domain::models::event::EscrowStatus as Es;
    if event.escrow_status != Es::Deactivated {
        return Err(AppError::Validation(format!(
            "escrow must be deactivated before claiming forfeited (current: {})",
            event.escrow_status
        ))
        .into());
    }

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

    let all_deposits = event_store::list_deposit_statuses(kv, &body.event_id, Some(db))
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

    let onchain_events = crate::escrow_indexer::get_onchain_events(db, &body.event_id, 200).await;

    // Exclusions come in two shapes that MUST be matched differently:
    //  - `Refund` events carry the attendee WALLET (accounts[0]).
    //  - `MarkCheckedIn` events carry the attendee_deposit PDA (accounts[2]),
    //    NOT a wallet.
    // The previous code lumped both into one wallet set, so the checked-in PDAs
    // never matched any wallet and checked-in attendees were never excluded —
    // their still-refundable deposits were offered as forfeit candidates. Fix:
    // derive each candidate wallet's deposit PDA and exclude it when that PDA
    // appears in the checked-in set.
    let mut refunded_wallets: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    let mut checked_in_pdas: std::collections::HashSet<String> = std::collections::HashSet::new();
    for ev in &onchain_events {
        match ev.instruction {
            crate::escrow_indexer::EscrowInstruction::Refund => {
                if let Some(ref wallet) = ev.attendee {
                    refunded_wallets.insert(wallet.clone());
                }
            }
            crate::escrow_indexer::EscrowInstruction::MarkCheckedIn => {
                if let Some(ref pda) = ev.attendee {
                    checked_in_pdas.insert(pda.clone());
                }
            }
            _ => {}
        }
    }

    // Map each candidate wallet → its deposit PDA, then exclude the checked-in ones.
    let wallet_pda_pairs = crate::solana_escrow::derive_attendee_deposit_pdas(
        organizer_pubkey,
        on_chain_event_id,
        &usdc_wallets,
    )
    .await
    .map_err(|e| AppError::Internal(format!("deposit PDA derivation failed: {e}")))?;
    let checked_in_wallets: std::collections::HashSet<String> = wallet_pda_pairs
        .into_iter()
        .filter(|(_, pda)| checked_in_pdas.contains(pda))
        .map(|(wallet, _)| wallet)
        .collect();

    let indexer_candidates: Vec<String> = usdc_wallets
        .into_iter()
        .filter(|w| !refunded_wallets.contains(w) && !checked_in_wallets.contains(w))
        .collect();

    // Defense against indexer lag: the D1 on-chain event index (populated by the
    // Helius webhook / RPC poller) may trail the chain. An attendee who was just
    // refunded-and-closed would still appear as a candidate here, but claiming
    // their now-closed AttendeeDeposit PDA fails on-chain with `IllegalOwner`
    // (closed PDAs revert to SystemProgram ownership, rejected by Quasar's
    // `Account<AttendeeDeposit>` owner check in ~195 CU, before any instruction
    // logic). Batch-verify each candidate's deposit PDA still exists and is owned
    // by the escrow program; drop any that don't, so we never build a doomed TX.
    let forfeited = crate::solana_escrow::filter_forfeitable_deposits(
        &rpc_url,
        organizer_pubkey,
        on_chain_event_id,
        &indexer_candidates,
    )
    .await
    .map_err(|e| AppError::Internal(format!("forfeitable deposit verification failed: {e}")))?;

    let dropped = indexer_candidates.len().saturating_sub(forfeited.len());
    if dropped > 0 {
        tracing::warn!(
            event_id = %event.id,
            candidates = indexer_candidates.len(),
            dropped,
            "Dropped {} candidate(s) whose AttendeeDeposit PDAs are no longer open (already refunded/closed) — likely indexer lag",
            dropped
        );
    }

    if forfeited.is_empty() {
        return Err(AppError::Validation(
            "no forfeited deposits to claim — all attendees checked in, refunded, or already settled".to_string(),
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

/// Build a close_deposit transaction for an attendee's verified USDC deposit.
#[worker::send]
pub async fn close_deposit_tx_handler(
    State(state): State<AppState>,
    Json(body): Json<CloseDepositTxRequest>,
) -> Result<ApiOk<CloseDepositTxResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;
    let d1 = state.d1.as_deref();

    let event = event_store::get_event_config(kv, &body.event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("event '{}' not found", body.event_id)))?;

    if !event.deposit_enabled {
        return Err(AppError::Validation("deposit not enabled for this event".to_string()).into());
    }

    crate::solana::validate_wallet_address(&body.wallet_address).map_err(AppError::Validation)?;

    let deposit_status = event_store::get_deposit_status(kv, &event.id, &body.attendee_id, d1)
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

    let rpc_url = state.config.solana.full_rpc_url();

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
