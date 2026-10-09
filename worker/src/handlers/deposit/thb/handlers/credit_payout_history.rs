//! `GET /api/deposit/credit-payouts`: the held-credit payouts already
//! recorded, newest first — who was paid, how much, by whom, and the slip.
//! Organizers only, org-scoped like the queue (`crate::credit_payout_history`).

use axum::{Extension, extract::State};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use serde::Serialize;

use super::credit_payout::{payout_scope, require_payout_operator};
use crate::audit_store::AuditAction;
use crate::credit_payout_history::{HISTORY_LIMIT, PayoutRecord, record_of, visible};
use crate::error::{ApiOk, WorkerError};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct CreditPayoutHistoryResponse {
    pub payouts: Vec<PayoutRecord>,
}

#[worker::send]
pub async fn credit_payout_history_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<CreditPayoutHistoryResponse>, WorkerError> {
    let scope = payout_scope(&state, &claims.email).await?;
    require_payout_operator(&scope)?;
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 not configured".to_string()))?;
    let action = serde_json::to_value(AuditAction::CreditRefundPaidOut)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .ok_or_else(|| AppError::Internal("audit action name".to_string()))?;
    let payouts = crate::db::get_global_audit_by_action(db, &action, HISTORY_LIMIT)
        .await
        .map_err(AppError::Internal)?
        .into_iter()
        .filter_map(|row| {
            let meta: Option<serde_json::Value> = row
                .metadata
                .as_deref()
                .and_then(|m| serde_json::from_str(m).ok());
            visible(&scope, meta.as_ref())
                .then(|| record_of(&row.timestamp, &row.actor, &row.target, meta.as_ref()))
        })
        .collect();
    Ok(ApiOk::new(CreditPayoutHistoryResponse { payouts }))
}
