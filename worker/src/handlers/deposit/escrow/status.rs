use axum::{
    Extension, Json,
    extract::{Query, State},
};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::DepositMethod;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::EscrowStatus;

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::handlers::ext::EventIdQuery;
use crate::state::AppState;

use super::types::*;

// ---------------------------------------------------------------------------
// GET /api/escrow/refund-queue (admin)
// ---------------------------------------------------------------------------

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
