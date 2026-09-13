//! GET /api/deposit/usdc/tx — Solana Pay Transaction Request callback

use axum::{Json, extract::Query, extract::State};

use crate::error::WorkerError;
use crate::event_store;
use crate::handlers::deposit::usdc::{DepositTxQuery, DepositTxResponse};
use crate::state::AppState;

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
    let kv = state.events_kv.as_ref();
    let d1 = state.d1.as_deref();

    let event = event_store::get_event_config_with_fallback(kv, d1, &query.event_id)
        .await
        .map_err(event_checkin_domain::models::error::AppError::Internal)?
        .ok_or_else(|| {
            event_checkin_domain::models::error::AppError::NotFound(format!(
                "event '{}' not found",
                query.event_id
            ))
        })?;

    if !event.deposit_enabled {
        return Err(event_checkin_domain::models::error::AppError::Validation(
            "deposit not enabled for this event".to_string(),
        )
        .into());
    }

    if event.deposit_amount_usdc == 0 {
        return Err(event_checkin_domain::models::error::AppError::Validation(
            "deposit amount not configured".to_string(),
        )
        .into());
    }

    // Validate wallet address
    crate::solana::validate_wallet_address(&query.wallet)
        .map_err(event_checkin_domain::models::error::AppError::Validation)?;

    // Verify deposit is still pending (not already completed)
    let existing =
        event_store::get_deposit_status_with_fallback(kv, d1, &event.id, &query.attendee_id)
            .await
            .map_err(event_checkin_domain::models::error::AppError::Internal)?;

    if let Some(status) = &existing
        && status.verified
    {
        return Err(event_checkin_domain::models::error::AppError::Validation(
            "deposit already verified".to_string(),
        )
        .into());
    }

    // Determine organizer pubkey for PDA derivation.
    // The event must have `organizer_wallet` set (the organizer's Solana address).
    // This is configured when the event is set up for deposits.
    let organizer_pubkey = if event.organizer_wallet.is_empty() {
        return Err(event_checkin_domain::models::error::AppError::Internal(
            "event has no organizer wallet configured — set organizer_wallet before enabling deposits".to_string(),
        ).into());
    } else {
        // Validate it's a proper base58 Solana address
        crate::solana::validate_wallet_address(&event.organizer_wallet).map_err(|e| {
            event_checkin_domain::models::error::AppError::Internal(format!(
                "invalid organizer_wallet: {e}"
            ))
        })?;
        &event.organizer_wallet
    };

    // The on_chain_event_id for PDA derivation.
    // If explicitly set (non-zero), use it. Otherwise, derive from event ID hash.
    let on_chain_event_id = if event.on_chain_event_id != 0 {
        event.on_chain_event_id
    } else {
        crate::handlers::deposit::derive_on_chain_event_id(&event.id)
    };

    let rpc_url = state.config.solana.full_rpc_url();

    // Build the deposit transaction
    let tx = crate::solana_escrow::build_deposit_transaction(
        &rpc_url,
        kv,
        organizer_pubkey,
        on_chain_event_id,
        &query.wallet,
        event.deposit_amount_usdc,
    )
    .await
    .map_err(|e| {
        event_checkin_domain::models::error::AppError::Internal(format!(
            "failed to build deposit TX: {e}"
        ))
    })?;

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
