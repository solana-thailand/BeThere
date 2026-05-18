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
use crate::escrow_indexer::{self, HeliusEnhancedTransaction, IndexSummary, OnChainEvent};
use crate::state::AppState;

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
                            "slot": event.slot,
                            "block_time": event.block_time,
                            "organizer": event.organizer,
                            "attendee": event.attendee,
                            "amount": event.amount,
                        }),
                    ),
                )
                .await;

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
