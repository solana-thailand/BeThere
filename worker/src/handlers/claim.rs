//! Claim API handlers — thin HTTP wrappers for the claim service.
//!
//! Public endpoints (no auth required) for attendees to:
//! - Look up their claim status by token (GET /api/claim/{token})
//! - Mint a compressed NFT to their wallet (POST /api/claim/{token})

use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use serde::Deserialize;
use serde_json::json;

use event_checkin_domain::models::api::{ClaimLookupResponse, ClaimResponse};
use event_checkin_domain::models::error::AppError;

use super::ext::EventIdQuery;
use crate::solana::validate_wallet_address;
use crate::state::AppState;

/// Request body for POST /api/claim/{token}.
#[derive(Debug, Deserialize)]
pub struct ClaimRequest {
    pub wallet_address: String,
}

/// GET /api/claim/{token}
/// Look up an attendee by their claim token.
///
/// Returns the attendee's name, check-in time, and claim status.
/// The claim token is generated during check-in (column L in the sheet).
#[worker::send]
pub async fn get_claim(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<Json<serde_json::Value>, crate::error::WorkerError> {
    let lookup = crate::claim::lookup_claim(&state, &token, query.event_id.as_deref()).await?;

    let response = ClaimLookupResponse {
        name: lookup.name,
        checked_in_at: lookup.checked_in_at,
        claim_token: lookup.claim_token,
        claimed: lookup.claimed,
        claimed_at: lookup.claimed_at,
        nft_available: lookup.nft_available,
        locked_wallet: lookup.locked_wallet,
        event: lookup.event,
        quiz_status: lookup.quiz_status,
        total_checked_in: lookup.total_checked_in,
        total_claimed: lookup.total_claimed,
    };

    Ok(Json(json!({
        "success": true,
        "data": response,
    })))
}

/// POST /api/claim/{token}
/// Mint a compressed NFT and mark the attendee's claim as fulfilled.
///
/// Request body must include a Solana wallet address (base58, 32-44 chars).
/// The attendee must be checked in and not already claimed.
#[worker::send]
pub async fn post_claim(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<EventIdQuery>,
    Json(body): Json<ClaimRequest>,
) -> Result<Json<serde_json::Value>, crate::error::WorkerError> {
    // Validate wallet address format
    if let Err(e) = validate_wallet_address(&body.wallet_address) {
        tracing::warn!(claim_token = %token, error = %e, "invalid wallet address for claim");
        return Err(AppError::Validation(e).into());
    }

    let result = crate::claim::execute_claim(
        &state,
        &token,
        &body.wallet_address,
        query.event_id.as_deref(),
    )
    .await?;

    let response = ClaimResponse {
        name: result.name,
        asset_id: result.asset_id,
        signature: result.signature,
        wallet_address: result.wallet_address,
        claimed_at: result.claimed_at,
        cluster: result.cluster,
    };

    Ok(Json(json!({
        "success": true,
        "data": response,
    })))
}
