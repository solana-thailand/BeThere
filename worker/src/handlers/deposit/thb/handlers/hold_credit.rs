use axum::{Extension, Json, extract::State};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::DepositMethod;
use event_checkin_domain::models::error::AppError;
use serde::{Deserialize, Serialize};

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// POST /api/deposit/hold
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct HoldDepositRequest {
    pub event_id: String,
    pub attendee_id: String,
}

#[derive(Serialize)]
pub struct HoldDepositResponse {
    pub credit_thb: u64,
    pub credit_usdc: u64,
    pub message: String,
}

/// Attendee holds their deposit as rolling credit instead of claiming refund.
/// Increments their rolling deposit credit in the Master Contacts Sheet.
#[worker::send]
pub async fn hold_deposit_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<HoldDepositRequest>,
) -> Result<ApiOk<HoldDepositResponse>, WorkerError> {
    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %body.event_id,
        email = %claims.email,
        "hold deposit initiated"
    );

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    // 1. Get event config
    let event = event_store::get_event_config(kv, &body.event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound(format!("event '{}' not found", body.event_id)))?;

    if !event.deposit_enabled {
        return Err(AppError::Validation("deposit not enabled for this event".to_string()).into());
    }

    // VULN-012: Verify the authenticated user owns this attendee record
    let attendee = crate::sheets::get_attendee_by_id(
        &body.attendee_id,
        &state,
        &event.sheet_id,
        &event.sheet_name,
        Some(kv),
    )
    .await
    .map_err(AppError::Internal)?
    .ok_or_else(|| AppError::NotFound("attendee not found".to_string()))?;

    if !attendee.email.eq_ignore_ascii_case(&claims.email) {
        tracing::warn!(
            claims_email = %claims.email,
            attendee_email = %attendee.email,
            attendee_id = %body.attendee_id,
            "hold deposit rejected: email mismatch"
        );
        return Err(
            AppError::Unauthorized("you can only hold your own deposit".to_string()).into(),
        );
    }

    // 2. Look up deposit status
    let deposit = event_store::get_deposit_status(kv, &event.id, &body.attendee_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("no deposit to hold".to_string()))?;

    // 3. Must be verified
    if !deposit.verified {
        return Err(
            AppError::Validation("deposit must be verified before holding".to_string()).into(),
        );
    }

    // 4. Determine currency from deposit method
    let (currency, amount) = match deposit.method {
        DepositMethod::Usdc => ("usdc", deposit.amount),
        DepositMethod::Thb => ("thb", deposit.amount),
        DepositMethod::CreditThb | DepositMethod::CreditUsdc => {
            return Err(AppError::Validation("already a credit deposit".to_string()).into());
        }
    };

    // 5. Resolve contacts sheet per-org
    let resolved = crate::org_store::resolve_contacts_sheet(kv, &event, &state.config.sheets).await;

    if resolved.sheet_id.is_empty() {
        return Err(AppError::Internal("contacts sheet not configured".to_string()).into());
    }

    // 6. Increment credit
    crate::sheets::contacts::increment_credit(
        &state,
        &resolved.sheet_id,
        &resolved.contacts_sheet_name,
        Some(kv),
        &claims.email,
        currency,
        amount,
    )
    .await
    .map_err(AppError::Internal)?;

    // 7. Get updated balance
    let (credit_thb, credit_usdc) = crate::sheets::contacts::get_credit_balance(
        &state,
        &resolved.sheet_id,
        &resolved.contacts_sheet_name,
        Some(kv),
        &claims.email,
    )
    .await
    .map_err(AppError::Internal)?;

    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %event.id,
        %currency,
        amount,
        credit_thb,
        credit_usdc,
        "deposit held as credit"
    );

    Ok(ApiOk::new(HoldDepositResponse {
        credit_thb,
        credit_usdc,
        message: format!("{amount} {currency} deposit held as credit"),
    }))
}

// ---------------------------------------------------------------------------
// GET /api/deposit/credit-balance
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct CreditBalanceResponse {
    pub credit_thb: u64,
    pub credit_usdc: u64,
}

/// Returns the authenticated user's deposit credit balance.
#[worker::send]
pub async fn credit_balance_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<CreditBalanceResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    // Resolve contacts sheet using global config (no event context needed)
    let resolved = {
        let global = &state.config.sheets;
        event_checkin_domain::models::org::ResolvedContactsSheet {
            sheet_id: global.contacts_sheet_id.clone(),
            contacts_sheet_name: global.contacts_sheet_name.clone(),
            events_sheet_name: global.events_sheet_name.clone(),
        }
    };

    if resolved.sheet_id.is_empty() {
        return Err(AppError::Internal("contacts sheet not configured".to_string()).into());
    }

    let (credit_thb, credit_usdc) = crate::sheets::contacts::get_credit_balance(
        &state,
        &resolved.sheet_id,
        &resolved.contacts_sheet_name,
        Some(kv),
        &claims.email,
    )
    .await
    .map_err(AppError::Internal)?;

    Ok(ApiOk::new(CreditBalanceResponse {
        credit_thb,
        credit_usdc,
    }))
}
