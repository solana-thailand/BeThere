//! POST /api/deposit/usdc

use axum::{Json, extract::State};
use chrono::Utc;
use event_checkin_domain::models::deposit::{
    DepositMethod, DepositStatus, UsdcDepositRequest, UsdcDepositResponse,
};

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::handlers::deposit::usdc::check_in_person_capacity;
use crate::state::AppState;

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
    let kv = state.events_kv.as_ref();
    let d1 = state.d1.as_deref();

    let event = event_store::get_event_config_with_fallback(kv, d1, &body.event_id)
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
            kv,
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
                    kv,
                )
                .await
                {
                    if let Some(ctx) = &state.worker_ctx {
                        ctx.wait_until(crate::sheets::bg_sync::update_participation_type(
                            state.clone(),
                            attendee.row_index,
                            "In-Person".to_string(),
                            mapping,
                            event.sheet_id.clone(),
                            event.sheet_name.clone(),
                            kv.cloned(),
                        ));
                        tracing::info!(
                            attendee_id = %attendee.api_id,
                            "deposit deadline reclaim: switched back to In-Person (bg)"
                        );
                    } else {
                        match crate::sheets::write::update_participation_type(
                            attendee.row_index,
                            "In-Person",
                            &mapping,
                            &state,
                            &event.sheet_id,
                            &event.sheet_name,
                            kv,
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

    // Check if already deposited.
    //
    // IMPORTANT: `deposit_usdc` is called *before* the wallet signs the TX, so
    // it always saves a pending `DepositStatus` (verified=false, no
    // tx_signature). If the user rejects the wallet prompt, that record is an
    // orphan. We allow re-initiation in that case (reusing the existing
    // deposit_order so the refundable tier isn't double-assigned), and only
    // reject when a real deposit already exists (verified, or carries a TX
    // signature that was actually signed/sent).
    let existing =
        event_store::get_deposit_status_with_fallback(kv, d1, &event.id, &body.attendee_id)
            .await
            .map_err(event_checkin_domain::models::error::AppError::Internal)?;

    let is_orphan = existing.as_ref().is_some_and(|s| {
        !s.verified
            && matches!(s.method, DepositMethod::Usdc)
            && s.tx_signature.as_deref().is_none_or(|t| t.is_empty())
    });

    if existing.is_some() && !is_orphan {
        return Err(event_checkin_domain::models::error::AppError::Validation(
            "attendee already has a deposit".to_string(),
        )
        .into());
    }

    // Guard 1 (double-registration defence — plan 003): reject if this wallet
    // is already bound to a *different* attendee in this event. Without this
    // check, an organizer deleting an attendee row (off-chain only — the
    // on-chain `AttendeeDeposit` PDA persists) would let the same wallet
    // re-register as a new attendee_id and then have the read-path recovery
    // re-verify the new row against the original on-chain TX — yielding two
    // verified attendees, two QR codes, one deposit. The same bug is also
    // exploitable by a malicious user (no organizer help required).
    let wallet_owner =
        event_store::find_attendee_by_wallet_with_fallback(kv, d1, &event.id, &body.wallet_address)
            .await
            .map_err(event_checkin_domain::models::error::AppError::Internal)?;

    match wallet_owner {
        Some(owner_id) if owner_id != body.attendee_id => {
            tracing::warn!(
                event_id = %event.id,
                new_attendee_id = %body.attendee_id,
                existing_attendee_id = %owner_id,
                wallet = %body.wallet_address,
                "deposit initiation rejected: wallet already bound to another registration"
            );
            return Err(event_checkin_domain::models::error::AppError::Validation(
                "wallet is already bound to another registration for this event".to_string(),
            )
            .into());
        }
        _ => {}
    }

    // Assign deposit order.
    //
    // For a re-initiation of an orphan (user rejected the previous wallet
    // prompt), reuse the orphan's existing deposit_order and refundable flag
    // so the counter isn't double-incremented and the refundable tier stays
    // consistent. For a fresh initiation, increment the counter normally.
    let (deposit_order, refundable) = match existing.as_ref().filter(|_| is_orphan) {
        Some(orphan) => (orphan.deposit_order, orphan.refundable),
        None => {
            let order = event_store::increment_deposit_counter_with_fallback(kv, d1, &event.id)
                .await
                .map_err(event_checkin_domain::models::error::AppError::Internal)?;
            let refundable =
                event.max_refundable_deposits == 0 || order <= event.max_refundable_deposits;
            (order, refundable)
        }
    };

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

    event_store::save_deposit_status_with_fallback(kv, d1, &deposit_status)
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
    if let Some(kv) = kv {
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
            d1,
        )
        .await;
    }

    Ok(ApiOk::new(UsdcDepositResponse {
        transaction: String::new(), // Transaction is built on-demand by the callback endpoint
        solana_pay_url,
    }))
}
