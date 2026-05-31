//! Handlers for on-chain escrow event indexing.
//!
//! Endpoints:
//!   POST /api/escrow/onchain-webhook  — Helius enhanced webhook (receives TX data)
//!   POST /api/escrow/sync             — Manual sync trigger (admin, polls RPC)
//!   GET  /api/escrow/events/{event_id} — Query indexed on-chain events

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::HeaderMap,
};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::error::{ApiOk, WorkerError};
use crate::escrow_indexer::{
    self, EscrowInstruction, HeliusEnhancedTransaction, IndexSummary, OnChainEvent,
};
use crate::state::AppState;
use worker::KvStore;

// ---------------------------------------------------------------------------
// Shared: post-indexing hook for RolloverDeposit events
// ---------------------------------------------------------------------------

/// After a `RolloverDeposit` is indexed on the source event, create a
/// `DepositStatus` on the **target** event so the admin panel and downstream
/// flows recognize the deposit.
pub(crate) async fn apply_rollover_deposit_status(kv: &KvStore, event: &OnChainEvent) {
    if event.instruction != EscrowInstruction::RolloverDeposit {
        return;
    }
    let Some(ref target_escrow) = event.target_escrow_address else {
        tracing::warn!(sig = %event.signature, "rollover event has no target_escrow_address");
        return;
    };

    let target_event_id = escrow_indexer::resolve_event_by_escrow(kv, target_escrow).await;
    let Some(target_id) = target_event_id else {
        tracing::warn!(
            sig = %event.signature,
            target_escrow = %target_escrow,
            "could not resolve target event ID for rollover deposit_status"
        );
        return;
    };

    // Resolve attendee API ID from wallet address.
    // First try the target event, then fall back to the source event
    // (the attendee may not have deposited on the target event yet —
    // that's exactly what the rollover is creating).
    let attendee_wallet = event.attendee.as_deref().unwrap_or("");
    let attendee_api_id =
        crate::event_store::find_attendee_by_wallet(kv, &target_id, attendee_wallet)
            .await
            .ok()
            .flatten();

    // Fallback: search source event deposit statuses for the attendee
    let attendee_api_id = match attendee_api_id {
        Some(id) => Some(id),
        None => {
            let source_event_id =
                escrow_indexer::resolve_event_by_escrow(kv, &event.escrow_address).await;
            match source_event_id {
                Some(src_id) => {
                    crate::event_store::find_attendee_by_wallet(kv, &src_id, attendee_wallet)
                        .await
                        .ok()
                        .flatten()
                }
                None => None,
            }
        }
    };

    let Some(api_id) = attendee_api_id else {
        tracing::warn!(
            sig = %event.signature,
            target_escrow = %target_escrow,
            source_escrow = %event.escrow_address,
            wallet = %attendee_wallet,
            "could not resolve attendee API ID for rollover target deposit_status (tried both target and source events)"
        );
        return;
    };

    // Check if deposit_status already exists (dedup)
    let existing = crate::event_store::get_deposit_status(kv, &target_id, &api_id)
        .await
        .ok()
        .flatten();

    if existing.is_some() {
        tracing::info!(
            sig = %event.signature,
            target_event_id = %target_id,
            attendee_id = %api_id,
            "DepositStatus already exists on target event, skipping"
        );
        return;
    }

    // Resolve deposit amount: prefer on-chain parsed amount, fall back to source event's deposit record.
    let resolved_amount = match event.amount {
        Some(a) if a > 0 => a,
        _ => {
            let mut found = 0u64;
            let source_event_id =
                escrow_indexer::resolve_event_by_escrow(kv, &event.escrow_address).await;
            if let Some(src_id) = source_event_id
                && let Ok(Some(src_status)) =
                    crate::event_store::get_deposit_status(kv, &src_id, &api_id).await
            {
                found = src_status.amount;
            }
            if found == 0 {
                tracing::warn!(
                    sig = %event.signature,
                    source_escrow = %event.escrow_address,
                    attendee_id = %api_id,
                    "could not resolve rollover amount from on-chain or source deposit"
                );
            }
            found
        }
    };

    let deposit_status = event_checkin_domain::models::deposit::DepositStatus {
        attendee_id: api_id.clone(),
        event_id: target_id.clone(),
        method: event_checkin_domain::models::deposit::DepositMethod::Usdc,
        amount: resolved_amount,
        currency: "USDC".to_string(),
        tx_signature: Some(event.signature.clone()),
        verified: true,
        deposited_at: event.indexed_at.clone(),
        wallet_address: event.attendee.clone(),
        deposit_order: 0,
        refundable: true,
        rejected: false,
    };

    match crate::event_store::save_deposit_status(kv, &deposit_status).await {
        Ok(()) => {
            tracing::info!(
                sig = %event.signature,
                target_event_id = %target_id,
                attendee_id = %api_id,
                "created DepositStatus for rollover target event"
            );
        }
        Err(e) => {
            tracing::warn!(
                sig = %event.signature,
                error = %e,
                "failed to save DepositStatus for rollover target"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// POST /api/escrow/onchain-webhook — Helius enhanced webhook
// ---------------------------------------------------------------------------

/// Request body for the Helius on-chain webhook.
///
/// Helius sends an array of enhanced transaction objects.
#[derive(Debug, serde::Deserialize)]
pub struct OnchainWebhookRequest {
    /// Array of enhanced transactions from Helius.
    #[serde(default)]
    pub transactions: Vec<HeliusEnhancedTransaction>,
}

/// Helius webhook handler for on-chain escrow events.
///
/// Called by Helius when a transaction involves the escrow program.
/// Parses each transaction, resolves the event ID, and stores in KV.
///
/// **Authentication**: Validates `Authorization: Bearer <token>` header
/// against the `WEBHOOK_SECRET` env var. If the var is not set or empty,
/// validation is skipped for backward compatibility.
#[worker::send]
pub async fn onchain_webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<OnchainWebhookRequest>,
) -> Result<ApiOk<IndexSummary>, WorkerError> {
    // Validate Bearer token if webhook secret is configured
    if !state.webhook_secret.is_empty() {
        let expected = format!("Bearer {}", state.webhook_secret);
        let auth_header = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if auth_header != expected {
            tracing::warn!(
                auth = %auth_header,
                "webhook rejected: invalid or missing Authorization header"
            );
            return Err(AppError::Unauthorized(
                "invalid or missing Authorization header".to_string(),
            )
            .into());
        }
    }
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    if body.transactions.is_empty() {
        return Ok(ApiOk::new(IndexSummary::default()));
    }

    tracing::info!(
        count = body.transactions.len(),
        "received on-chain webhook transactions"
    );

    // Process transactions — resolve event IDs async per transaction
    let mut summary = IndexSummary::default();

    for tx in &body.transactions {
        // Skip failed transactions
        if tx.transaction_error.is_some() {
            summary.skipped_failed += 1;
            continue;
        }

        let Some(event) = escrow_indexer::parse_helius_transaction(tx) else {
            summary.skipped_no_event += 1;
            continue;
        };

        // Resolve event ID from escrow address (async)
        let event_id = escrow_indexer::resolve_event_by_escrow(kv, &event.escrow_address).await;

        let Some(event_id) = event_id else {
            tracing::warn!(
                escrow = %event.escrow_address,
                sig = %event.signature,
                "no off-chain event found for escrow address, skipping"
            );
            summary.skipped_no_event += 1;
            continue;
        };

        match escrow_indexer::save_onchain_event(kv, &event_id, event.clone()).await {
            Ok(true) => {
                tracing::info!(
                    sig = %event.signature,
                    instruction = %event.instruction,
                    event_id = %event_id,
                    "indexed on-chain event via webhook"
                );

                // Also append to audit trail
                let _ = crate::audit_store::append_event_audit(
                    kv,
                    &event_id,
                    crate::audit_store::create_entry_with_meta(
                        "on-chain",
                        crate::audit_store::AuditAction::OnChainEventIndexed,
                        &event.signature,
                        &format!("on-chain: {}", event.instruction),
                        serde_json::json!({
                            "instruction": event.instruction.to_string(),
                            "escrow_address": event.escrow_address,
                            "target_escrow_address": event.target_escrow_address,
                            "slot": event.slot,
                            "block_time": event.block_time,
                            "organizer": event.organizer,
                            "attendee": event.attendee,
                            "amount": event.amount,
                        }),
                    ),
                    state.d1.as_deref(),
                )
                .await;

                // For RolloverDeposit: create DepositStatus on the target event
                apply_rollover_deposit_status(kv, &event).await;

                summary.indexed += 1;
            }
            Ok(false) => {
                summary.duplicates += 1;
            }
            Err(e) => {
                tracing::error!(
                    sig = %event.signature,
                    error = %e,
                    "failed to save on-chain event"
                );
                summary.errors += 1;
            }
        }
    }

    tracing::info!(
        indexed = summary.indexed,
        duplicates = summary.duplicates,
        skipped_failed = summary.skipped_failed,
        skipped_no_event = summary.skipped_no_event,
        errors = summary.errors,
        "on-chain webhook processing complete"
    );

    Ok(ApiOk::new(summary))
}

// ---------------------------------------------------------------------------
// POST /api/escrow/sync — Manual RPC polling sync
// ---------------------------------------------------------------------------

/// Request body for manual escrow sync.
#[derive(Debug, serde::Deserialize)]
pub struct EscrowSyncRequest {
    /// Event ID to sync.
    pub event_id: String,
}

/// Manual sync trigger for an event's on-chain escrow events.
///
/// Uses `getSignaturesForAddress` + `getTransaction` to poll for
/// recent transactions against the escrow PDA.
///
/// **Requires**: SuperAdmin or organizer role.
#[worker::send]
pub async fn escrow_sync_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<EscrowSyncRequest>,
) -> Result<ApiOk<IndexSummary>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event = crate::event_store::get_event_config(kv, &body.event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("event '{}' not found", body.event_id)))?;

    // Access control: must be organizer or super admin
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
            "only organizers or super admins can sync escrow events".to_string(),
        )
        .into());
    }

    if event.escrow_address.is_empty() {
        return Err(AppError::Validation(
            "event has no escrow address — initialize escrow first".to_string(),
        )
        .into());
    }

    let rpc_url = state.config.solana.full_rpc_url();

    tracing::info!(
        event_id = %event.id,
        escrow = %event.escrow_address,
        email = %claims.email,
        "manual escrow sync triggered"
    );

    let summary =
        escrow_indexer::poll_escrow_events(kv, &rpc_url, &event.escrow_address, &event.id)
            .await
            .map_err(AppError::Internal)?;

    // Apply rollover deposit status hook for any newly indexed RolloverDeposit events
    if summary.indexed > 0 {
        let onchain_events = escrow_indexer::read_onchain_events(kv, &event.id).await;
        for ev in &onchain_events {
            apply_rollover_deposit_status(kv, ev).await;
        }
    }

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry_with_meta(
            &claims.email,
            crate::audit_store::AuditAction::OnChainEventIndexed,
            &event.id,
            "manual escrow sync",
            serde_json::json!({
                "indexed": summary.indexed,
                "duplicates": summary.duplicates,
                "skipped_failed": summary.skipped_failed,
                "skipped_no_event": summary.skipped_no_event,
                "errors": summary.errors,
            }),
        ),
        state.d1.as_deref(),
    )
    .await;

    Ok(ApiOk::new(summary))
}

// ---------------------------------------------------------------------------
// GET /api/escrow/events/{event_id} — Query indexed on-chain events
// ---------------------------------------------------------------------------

/// Response for querying indexed on-chain events.
#[derive(Debug, serde::Serialize)]
pub struct OnchainEventsResponse {
    /// Event ID.
    pub event_id: String,
    /// Escrow PDA address.
    pub escrow_address: String,
    /// Indexed on-chain events (newest first).
    pub events: Vec<OnChainEvent>,
}

/// Get indexed on-chain events for an event.
///
/// **Requires**: Staff auth (organizer or super admin).
#[worker::send]
pub async fn get_onchain_events_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<String>,
) -> Result<ApiOk<OnchainEventsResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let event = crate::event_store::get_event_config(kv, &event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("event '{}' not found", event_id)))?;

    // Access control
    let is_staff = state.is_staff(&claims.email);
    let is_organizer = event
        .organizer_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(&claims.email));
    let is_super_admin = state
        .config
        .super_admin_emails
        .contains(&claims.email.to_lowercase());

    if !is_staff && !is_organizer && !is_super_admin {
        return Err(AppError::Forbidden("insufficient permissions".to_string()).into());
    }

    let events = escrow_indexer::get_onchain_events(kv, &event.id, 100).await;

    Ok(ApiOk::new(OnchainEventsResponse {
        event_id: event.id,
        escrow_address: event.escrow_address,
        events,
    }))
}
