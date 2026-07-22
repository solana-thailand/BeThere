use axum::{Extension, Json, extract::State};
use chrono::Utc;
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
    let d1 = state.d1.as_deref();

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

    // 2. Look up deposit status (unified view across methods)
    let deposit = event_store::get_deposit_status(kv, &event.id, &body.attendee_id, d1)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("no deposit to hold".to_string()))?;

    // 3. Must be verified
    if !deposit.verified {
        return Err(
            AppError::Validation("deposit must be verified before holding".to_string()).into(),
        );
    }

    // 4. THB-only. USDC deposits must use the atomic on-chain rollover endpoint
    //    (POST /api/escrow/rollover-deposit). Off-chain hold has no settleable
    //    record for USDC, so accepting it here would allow double-credit.
    if !matches!(deposit.method, DepositMethod::Thb) {
        return Err(AppError::Validation(
            "only THB deposits can be held as credit; USDC uses the on-chain rollover endpoint"
                .to_string(),
        )
        .into());
    }

    // 5. Load the settleable THB record (authoritative for refund/hold state).
    let mut thb_deposit = event_store::get_thb_deposit(kv, &event.id, &body.attendee_id, d1)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("THB deposit record not found".to_string()))?;

    // 6. Idempotency guards — reject if already in a terminal settled state.
    //    A prior call to /hold (or /refund) settles the deposit; without these
    //    guards a re-call would double-increment credit.
    if thb_deposit.refunded {
        return Err(AppError::Validation("deposit already refunded".to_string()).into());
    }
    if thb_deposit.held_as_credit {
        return Err(AppError::Validation("deposit already held as credit".to_string()).into());
    }

    // 7. Resolve contacts sheet per-org
    let resolved = if let Some(db) = state.d1.as_deref() {
        crate::org_store::resolve_contacts_sheet(db, &event, &state.config.sheets).await
    } else {
        event_checkin_domain::models::org::ResolvedContactsSheet {
            sheet_id: state.config.sheets.contacts_sheet_id.clone(),
            contacts_sheet_name: state.config.sheets.contacts_sheet_name.clone(),
            events_sheet_name: state.config.sheets.events_sheet_name.clone(),
        }
    };

    if resolved.sheet_id.is_empty() {
        return Err(AppError::Internal("contacts sheet not configured".to_string()).into());
    }

    // 8. Settle the source deposit BEFORE incrementing credit. Failure ordering
    //    is load-bearing: if settle succeeds but the credit increment fails, the
    //    attendee receives no credit and the deposit is marked held — no money is
    //    created and an admin can reconcile manually. The reverse order would
    //    permit infinite credit via retry. Mirrors mark_refund_handler's settle.
    let held_amount = thb_deposit.amount_thb;
    let now = Utc::now().to_rfc3339();
    thb_deposit.held_as_credit = true;
    thb_deposit.held_as_credit_at = Some(now);
    event_store::save_thb_deposit(kv, &thb_deposit, d1)
        .await
        .map_err(AppError::Internal)?;

    // 9. Increment credit (THB only — USDC path rejected above).
    crate::sheets::contacts::increment_credit(
        &state,
        &resolved.sheet_id,
        &resolved.contacts_sheet_name,
        Some(kv),
        &claims.email,
        "thb",
        held_amount,
    )
    .await
    .map_err(AppError::Internal)?;

    // 10. Get updated balance
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
        amount = held_amount,
        credit_thb,
        credit_usdc,
        "deposit held as credit"
    );

    // Audit the credit-granting action (sibling of RefundMarked). Non-fatal.
    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::DepositHeldAsCredit,
            &body.attendee_id,
            &format!("{held_amount} THB held as rolling credit"),
        ),
        state.d1.as_deref(),
    )
    .await;

    Ok(ApiOk::new(HoldDepositResponse {
        credit_thb,
        credit_usdc,
        message: format!("{held_amount} THB deposit held as credit"),
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
