use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::Utc;
use event_checkin_domain::models::deposit::{
    DepositMethod, DepositStatus, DepositStatusResponse, UsdcDepositRequest, UsdcDepositResponse,
};

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::handlers::ext::EventIdQuery;
use crate::state::AppState;

use super::{
    ConfirmDepositQuery, ConfirmDepositResponse, DepositTxQuery, DepositTxResponse,
    UpdateDepositSignatureRequest, check_and_switch_deadline, check_in_person_capacity,
    verify_and_confirm_deposit, verify_tx_on_chain,
};

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
    let kv = state.events_kv.as_ref().ok_or_else(|| {
        event_checkin_domain::models::error::AppError::Internal(
            "EVENTS KV not configured".to_string(),
        )
    })?;

    let event =
        event_store::resolve_event_or_fallback(Some(kv), query.event_id.as_deref(), &state.config)
            .await
            .map_err(event_checkin_domain::models::error::AppError::Internal)?;

    let status = event_store::get_deposit_status(kv, &event.id, &attendee_id)
        .await
        .map_err(event_checkin_domain::models::error::AppError::Internal)?;

    // --- Deposit deadline edge-trigger ---
    // If event has a deposit_deadline_hours and attendee hasn't deposited,
    // check if deadline passed since registration. If so, auto-switch to Online.
    let mut deadline_expired = false;
    let mut registration_date: Option<String> = None;
    let mut in_person_available: Option<bool> = None;

    if status.is_none() && event.deposit_deadline_hours.is_some() {
        // Look up the attendee to get registration_date and participation_type
        if let Ok(Some(attendee)) = crate::sheets::get_attendee_by_id(
            &attendee_id,
            &state,
            &event.sheet_id,
            &event.sheet_name,
            Some(kv),
        )
        .await
        {
            registration_date = attendee.registration_date.clone();

            // Only auto-switch if currently in-person
            if attendee.is_in_person()
                && let Some(reg_str) = &attendee.registration_date
            {
                deadline_expired =
                    check_and_switch_deadline(&state, kv, &event, &attendee, reg_str).await;
            }

            // Check if in-person capacity is still available (reclaim flow)
            if deadline_expired {
                in_person_available = Some(check_in_person_capacity(&state, &event, kv).await);
            }
        }
    }

    Ok(ApiOk::new(DepositStatusResponse {
        deposit_enabled: event.deposit_enabled,
        deposit_amount_usdc: event.deposit_amount_usdc,
        deposit_amount_thb: event.deposit_amount_thb,
        promptpay_id: event.promptpay_id,
        event_start_ms: event.event_start_ms,
        event_end_ms: event.event_end_ms,
        refund_deadline_hours: event.refund_deadline_hours,
        event_name: event.name,
        event_tagline: event.tagline,
        event_slug: event.slug,
        status,
        dev_mode: state.config.dev_mode,
        deposit_deadline_hours: event.deposit_deadline_hours,
        deadline_expired,
        registration_date,
        in_person_available,
        usdc_deposits_accepted: event.escrow_status
            == event_checkin_domain::models::event::EscrowStatus::Initialized,
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
    let kv = state.events_kv.as_ref().ok_or_else(|| {
        event_checkin_domain::models::error::AppError::Internal(
            "EVENTS KV not configured".to_string(),
        )
    })?;

    let event = event_store::get_event_config(kv, &body.event_id)
        .await
        .map_err(event_checkin_domain::models::error::AppError::Internal)?
        .ok_or_else(|| {
            event_checkin_domain::models::error::AppError::NotFound(format!(
                "event '{}' not found",
                body.event_id
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

    // Reject deposits after the event has ended — the on-chain refund requires clock > event_end
    if event.event_end_ms > 0 {
        let now_ms = chrono::Utc::now().timestamp_millis();
        if now_ms > event.event_end_ms {
            return Err(event_checkin_domain::models::error::AppError::Validation(
                "event has ended — deposits are no longer accepted".to_string(),
            )
            .into());
        }
    }

    // Deposit deadline check: reject OR reclaim
    // If deadline expired but in-person capacity is still available,
    // switch the attendee back to In-Person and allow the deposit (reclaim flow).
    if let Some(deadline_hours) = event.deposit_deadline_hours
        && let Ok(Some(attendee)) = crate::sheets::get_attendee_by_id(
            &body.attendee_id,
            &state,
            &event.sheet_id,
            &event.sheet_name,
            Some(kv),
        )
        .await
        && let Some(reg_str) = &attendee.registration_date
        && let Ok(reg_time) = chrono::DateTime::parse_from_rfc3339(reg_str)
    {
        let deadline =
            reg_time.with_timezone(&Utc) + chrono::Duration::hours(i64::from(deadline_hours));
        if Utc::now() > deadline {
            // Deadline passed — check if reclaim is possible
            if check_in_person_capacity(&state, &event, kv).await {
                // Reclaim: switch back to In-Person so the deposit proceeds
                if let Ok(mapping) = crate::sheets::get_column_mapping(
                    &state,
                    &event.sheet_id,
                    &event.sheet_name,
                    Some(kv),
                )
                .await
                {
                    match crate::sheets::write::update_participation_type(
                        attendee.row_index,
                        "In-Person",
                        &mapping,
                        &state,
                        &event.sheet_id,
                        &event.sheet_name,
                        Some(kv),
                    )
                    .await
                    {
                        Ok(()) => tracing::info!(
                            attendee_id = %attendee.api_id,
                            "deposit deadline reclaim: switched back to In-Person"
                        ),
                        Err(e) => tracing::warn!(
                            attendee_id = %attendee.api_id,
                            error = %e,
                            "deposit deadline reclaim: failed to switch back to In-Person"
                        ),
                    }
                }
                // Continue to accept the deposit
            } else {
                return Err(event_checkin_domain::models::error::AppError::Validation(
                    "deposit deadline has passed and in-person spots are now full. You have been moved to the online track.".to_string(),
                ).into());
            }
        }
    }

    // Validate wallet address
    crate::solana::validate_wallet_address(&body.wallet_address)
        .map_err(event_checkin_domain::models::error::AppError::Validation)?;

    // Check if already deposited
    let existing = event_store::get_deposit_status(kv, &event.id, &body.attendee_id)
        .await
        .map_err(event_checkin_domain::models::error::AppError::Internal)?;

    if existing.is_some() {
        return Err(event_checkin_domain::models::error::AppError::Validation(
            "attendee already has a deposit".to_string(),
        )
        .into());
    }

    // Atomically increment deposit counter for this event
    let deposit_order = event_store::increment_deposit_counter(kv, &event.id)
        .await
        .map_err(event_checkin_domain::models::error::AppError::Internal)?;
    let refundable =
        event.max_refundable_deposits == 0 || deposit_order <= event.max_refundable_deposits;

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
        deposit_order,
        refundable,
        rejected: false,
    };

    event_store::save_deposit_status(kv, &deposit_status)
        .await
        .map_err(event_checkin_domain::models::error::AppError::Internal)?;

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
        deposit_order,
        refundable,
        tier = if refundable { "refundable" } else { "non-refundable" },
        "USDC deposit initiated"
    );

    // Audit log
    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry(
            "attendee",
            crate::audit_store::AuditAction::DepositSubmitted,
            &body.attendee_id,
            &format!(
                "USDC deposit initiated: {} lamports",
                event.deposit_amount_usdc
            ),
        ),
        state.d1.as_deref(),
    )
    .await;

    Ok(ApiOk::new(UsdcDepositResponse {
        transaction: String::new(), // Transaction is built on-demand by the callback endpoint
        solana_pay_url,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/deposit/usdc/tx — Solana Pay Transaction Request callback
// ---------------------------------------------------------------------------

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
    let kv = state.events_kv.as_ref().ok_or_else(|| {
        event_checkin_domain::models::error::AppError::Internal(
            "EVENTS KV not configured".to_string(),
        )
    })?;

    let event = event_store::get_event_config(kv, &query.event_id)
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
    let existing = event_store::get_deposit_status(kv, &event.id, &query.attendee_id)
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
        Some(kv),
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

// ---------------------------------------------------------------------------
// GET /api/deposit/usdc/confirm — Poll for deposit TX confirmation
// ---------------------------------------------------------------------------

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
    let kv = state.events_kv.as_ref().ok_or_else(|| {
        event_checkin_domain::models::error::AppError::Internal(
            "EVENTS KV not configured".to_string(),
        )
    })?;

    let event = event_store::get_event_config(kv, &query.event_id)
        .await
        .map_err(event_checkin_domain::models::error::AppError::Internal)?
        .ok_or_else(|| {
            event_checkin_domain::models::error::AppError::NotFound(format!(
                "event '{}' not found",
                query.event_id
            ))
        })?;

    // Check deposit status in KV
    let deposit_status = event_store::get_deposit_status(kv, &event.id, &query.attendee_id)
        .await
        .map_err(event_checkin_domain::models::error::AppError::Internal)?;

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
                    let rpc_url = state.config.solana.full_rpc_url();
                    let confirmed = verify_tx_on_chain(&rpc_url, sig).await;

                    if confirmed {
                        // Update the deposit status to verified
                        let mut updated = status.clone();
                        updated.verified = true;
                        event_store::save_deposit_status(kv, &updated)
                            .await
                            .map_err(event_checkin_domain::models::error::AppError::Internal)?;

                        tracing::info!(
                            attendee_id = %query.attendee_id,
                            tx_signature = %sig,
                            "USDC deposit confirmed on-chain"
                        );

                        // H5: Parallelize independent Sheets lookups
                        let deposit_amount_str = status.amount.to_string();
                        let (mapping_result, attendee_result) = futures_util::join!(
                            crate::sheets::get_column_mapping(
                                &state,
                                &event.sheet_id,
                                &event.sheet_name,
                                Some(kv),
                            ),
                            crate::sheets::get_attendee_by_id(
                                &query.attendee_id,
                                &state,
                                &event.sheet_id,
                                &event.sheet_name,
                                Some(kv),
                            )
                        );
                        if let (Ok(mapping), Ok(Some(attendee))) = (mapping_result, attendee_result)
                        {
                            let ctx = crate::sheets::write::SheetContext {
                                mapping: &mapping,
                                state: &state,
                                sheet_id: &event.sheet_id,
                                sheet_name: &event.sheet_name,
                                kv: Some(kv),
                            };

                            // Write deposit columns to sheet
                            if let Err(e) = crate::sheets::write::write_deposit_verification(
                                attendee.row_index,
                                "USDC",
                                &deposit_amount_str,
                                true,
                                &ctx,
                            )
                            .await
                            {
                                tracing::warn!(error = %e, "failed to write deposit verification to sheet (non-fatal)");
                            }

                            // Auto-generate QR if attendee doesn't have one
                            if attendee.qr_code_url.as_ref().is_none_or(|u| u.is_empty()) {
                                let server_url = &state.config.server.url;
                                let qr_url =
                                    format!("{server_url}/staff/?scan={}", attendee.api_id);
                                if let Err(e) = crate::sheets::write::update_qr_urls(
                                    &[(attendee.row_index, qr_url)],
                                    &mapping,
                                    &state,
                                    &event.sheet_id,
                                    &event.sheet_name,
                                    Some(kv),
                                )
                                .await
                                {
                                    tracing::warn!(error = %e, "failed to auto-generate QR for verified attendee (non-fatal)");
                                }
                            }
                        }

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

// ---------------------------------------------------------------------------
// POST /api/deposit/usdc/webhook — Helius webhook for TX confirmation
// ---------------------------------------------------------------------------

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
    let kv = state.events_kv.as_ref().ok_or_else(|| {
        event_checkin_domain::models::error::AppError::Internal(
            "EVENTS KV not configured".to_string(),
        )
    })?;

    // Get existing deposit status
    let mut deposit_status = event_store::get_deposit_status(kv, &body.event_id, &body.attendee_id)
        .await
        .map_err(event_checkin_domain::models::error::AppError::Internal)?
        .ok_or_else(|| {
            event_checkin_domain::models::error::AppError::NotFound(format!(
                "no deposit record for attendee '{}' in event '{}'",
                body.attendee_id, body.event_id
            ))
        })?;

    // Update with TX signature
    deposit_status.tx_signature = Some(body.tx_signature.clone());

    // Save immediately so the frontend sees the TX signature (H8: verification detached)
    event_store::save_deposit_status(kv, &deposit_status)
        .await
        .map_err(event_checkin_domain::models::error::AppError::Internal)?;

    tracing::info!(
        attendee_id = %body.attendee_id,
        tx_signature = %body.tx_signature,
        "USDC deposit TX signature recorded, pending on-chain verification"
    );

    // Detach on-chain verification — response returns immediately (H8)
    // If verified, updates deposit status + audit log in background via wait_until.
    if let Some(ctx) = &state.worker_ctx {
        let verify_state = state.clone();
        let verify_body = body.clone();
        ctx.wait_until(async move {
            verify_and_confirm_deposit(&verify_state, &verify_body).await;
        });
    } else {
        tracing::warn!(
            attendee_id = %body.attendee_id,
            "no worker_ctx available — skipping detached on-chain verification"
        );
    }

    Ok(ApiOk::new(serde_json::json!({
        "success": true,
        "confirmed": false, // pending — will be verified in background
        "tx_signature": body.tx_signature,
    })))
}
