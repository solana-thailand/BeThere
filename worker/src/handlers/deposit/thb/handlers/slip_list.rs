use axum::{
    Extension,
    extract::{Query, State},
};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::{PendingSlipResponse, RefundQueueResponse, ThbDeposit};
use event_checkin_domain::models::error::AppError;
use serde::{Deserialize, Serialize};

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::handlers::ext::EventIdQuery;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// GET /api/deposit/thb/pending (admin)
// ---------------------------------------------------------------------------

/// List all unverified THB deposits for admin review.
#[worker::send]
pub async fn pending_thb_slips_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<PendingSlipResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;
    let d1 = state.d1.as_deref();

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, query.event_id.as_deref())
            .await?;

    let all_deposits = event_store::list_thb_deposits(kv, &event.id, d1)
        .await
        .map_err(AppError::Internal)?;

    let mut pending: Vec<ThbDeposit> = all_deposits
        .into_iter()
        .filter(|d| !d.verified && d.slip_url.is_some())
        .collect();

    // Migrate any inline base64 slip URLs to R2 (keeps response payload small)
    super::migrate_data_urls(&state, kv, d1, &event.id, &mut pending).await;

    // Enrich with attendee names from Google Sheets
    let attendee_names =
        super::resolve_attendee_names(&state, &event.sheet_id, &event.sheet_name, &pending).await;
    let slips: Vec<ThbDeposit> = pending
        .into_iter()
        .map(|mut d| {
            d.attendee_name = attendee_names.get(&d.attendee_id).cloned();
            d
        })
        .collect();

    Ok(ApiOk::new(PendingSlipResponse { slips }))
}

// ---------------------------------------------------------------------------
// GET /api/refund/queue (admin)
// ---------------------------------------------------------------------------

/// List THB deposits that need refund (verified + checked-in + not yet refunded).
#[worker::send]
pub async fn refund_queue_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<RefundQueueResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;
    let d1 = state.d1.as_deref();

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, query.event_id.as_deref())
            .await?;

    let all_deposits = event_store::list_thb_deposits(kv, &event.id, d1)
        .await
        .map_err(AppError::Internal)?;

    // A deposit held as rolling credit is also terminal-settled (organizer
    // retains funds as liability); exclude it from the refund queue so the
    // admin does not double-process a deposit the attendee already converted.
    let mut pending: Vec<ThbDeposit> = all_deposits
        .into_iter()
        // Exclude non-cash deposits (rolling-credit applications, staff comps, ฿0):
        // they were never funded with cash, so they must not enter the refund
        // queue — refunding one pays out money that was never deposited.
        .filter(|d| d.verified && !d.refunded && !d.held_as_credit && !d.is_non_cash())
        .collect();

    // Migrate any inline base64 slip/refund URLs to R2 (keeps response payload small)
    super::migrate_data_urls(&state, kv, d1, &event.id, &mut pending).await;

    // Enrich with attendee names from Google Sheets
    let attendee_names =
        super::resolve_attendee_names(&state, &event.sheet_id, &event.sheet_name, &pending).await;
    let enriched: Vec<ThbDeposit> = pending
        .into_iter()
        .map(|mut d| {
            d.attendee_name = attendee_names.get(&d.attendee_id).cloned();
            d
        })
        .collect();

    Ok(ApiOk::new(RefundQueueResponse { pending: enriched }))
}

// ---------------------------------------------------------------------------
// GET /api/refund/refunded?event_id=xxx (admin)
// ---------------------------------------------------------------------------

/// Response for the refunded list endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundedListResponse {
    pub refunded: Vec<ThbDeposit>,
}

/// List all refunded THB deposits for an event.
#[worker::send]
pub async fn refunded_list_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<RefundedListResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;
    let d1 = state.d1.as_deref();

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, query.event_id.as_deref())
            .await?;

    let all_deposits = event_store::list_thb_deposits(kv, &event.id, d1)
        .await
        .map_err(AppError::Internal)?;

    let mut refunded: Vec<ThbDeposit> = all_deposits.into_iter().filter(|d| d.refunded).collect();

    // Migrate any inline base64 slip/refund URLs to R2 (keeps response payload small)
    super::migrate_data_urls(&state, kv, d1, &event.id, &mut refunded).await;

    // Enrich with attendee names from Google Sheets
    let attendee_names =
        super::resolve_attendee_names(&state, &event.sheet_id, &event.sheet_name, &refunded).await;
    let enriched: Vec<ThbDeposit> = refunded
        .into_iter()
        .map(|mut d| {
            d.attendee_name = attendee_names.get(&d.attendee_id).cloned();
            d
        })
        .collect();

    Ok(ApiOk::new(RefundedListResponse { refunded: enriched }))
}
