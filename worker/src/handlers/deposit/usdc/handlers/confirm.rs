//! GET /api/deposit/usdc/confirm — Poll for deposit TX confirmation

use axum::extract::{Query, State};

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::handlers::deposit::usdc::{
    ConfirmDepositQuery, ConfirmDepositResponse, recover_and_verify_deposit,
};
use crate::state::AppState;

/// Check if a USDC deposit has been confirmed on-chain.
///
/// This endpoint is polled by the frontend after the attendee sends the deposit
/// TX. It hands the record to [`recover_and_verify_deposit`] and reports back
/// whatever state that leaves it in.
///
/// **It does not verify anything itself.** It used to: a private copy of the
/// discovery-then-signer-cross-check transition, ~270 lines duplicating
/// `recover.rs` down to the sheet write and the QR auto-gen. That copy never
/// received the two guards the read path grew afterwards — **F1** (bind
/// verification to the on-chain `AttendeeDeposit` PDA; a signer cross-check
/// only proves the wallet signed *a* confirmed tx, so a self-transfer earned a
/// free verified ticket) and **Guard 2** (refuse a wallet or signature already
/// bound to another attendee). The route requires identity but does not bind
/// the caller to `query.attendee_id`, so any signed-in attendee could poll on
/// anyone's behalf. Delegating is what keeps the guards from drifting again;
/// see `worker/tests/deposit_verify_guard.rs`.
///
/// Delegation also puts the polling loop behind the read path's discovery
/// cooldown, which this endpoint — the one actually being polled — never had.
#[worker::send]
pub async fn confirm_deposit_handler(
    State(state): State<AppState>,
    Query(query): Query<ConfirmDepositQuery>,
) -> Result<ApiOk<ConfirmDepositResponse>, WorkerError> {
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

    let deposit_status =
        event_store::get_deposit_status_with_fallback(kv, d1, &event.id, &query.attendee_id)
            .await
            .map_err(event_checkin_domain::models::error::AppError::Internal)?;

    // No deposit record — the attendee hasn't initiated yet. Nothing to poll,
    // and no Solana Pay retry URL either: they have not been through initiation.
    let Some(status) = deposit_status else {
        return Ok(ApiOk::new(ConfirmDepositResponse {
            confirmed: false,
            tx_signature: None,
            solana_pay_url: None,
        }));
    };

    // Discovery, verification, both guards and every side effect. Returns
    // immediately when already verified, so the fast path stays fast.
    let status = recover_and_verify_deposit(&state, &event, status).await;
    let tx_signature = status.tx_signature.clone().filter(|s| !s.is_empty());

    if status.verified {
        return Ok(ApiOk::new(ConfirmDepositResponse {
            confirmed: true,
            tx_signature,
            solana_pay_url: None,
        }));
    }

    // A signature is recorded but not (yet) verified — keep polling. Covers
    // "not yet confirmed", "RPC error" and "a guard refused" alike: the
    // frontend's only useful move in all three is to poll again, and each is
    // already logged at warn level where it was decided.
    if tx_signature.is_some() {
        return Ok(ApiOk::new(ConfirmDepositResponse {
            confirmed: false,
            tx_signature,
            solana_pay_url: None,
        }));
    }

    // Nothing recorded and nothing discoverable on-chain — hand back the
    // Solana Pay URL so the frontend can restart the deposit flow.
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
