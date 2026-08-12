//! Claim API handlers — thin HTTP wrappers for the claim service.
//!
//! Public endpoints (no auth required) for attendees to:
//! - Look up their claim status by token (GET /api/claim/{token})
//! - Mint a compressed NFT to their wallet (POST /api/claim/{token})

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;

use crate::error::ApiOk;

use event_checkin_domain::models::api::{ClaimLookupResponse, ClaimResponse};
use event_checkin_domain::models::error::AppError;

use super::ext::EventIdQuery;
use crate::solana::validate_wallet_address;
use crate::state::AppState;

/// Request body for POST /api/claim/{token}.
///
/// The mint recipient is resolved SERVER-SIDE. A client may either ask to mint to
/// its verified linked profile wallet (`use_linked_wallet: true`, no address
/// trusted from the client) or supply an explicit override `wallet_address`.
#[derive(Debug, Default, Deserialize)]
pub struct ClaimRequest {
    #[serde(default)]
    pub wallet_address: Option<String>,
    #[serde(default)]
    pub use_linked_wallet: bool,
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
) -> Result<ApiOk<ClaimLookupResponse>, crate::error::WorkerError> {
    let lookup = crate::claim::lookup_claim(&state, &token, query.event_id.as_deref()).await?;

    let response = ClaimLookupResponse {
        name: lookup.name,
        checked_in_at: lookup.checked_in_at,
        claim_token: lookup.claim_token,
        claimed: lookup.claimed,
        claimed_at: lookup.claimed_at,
        nft_available: lookup.nft_available,
        locked_wallet: lookup.locked_wallet,
        linked_wallet_display: lookup.linked_wallet_display,
        event: lookup.event,
        quiz_status: lookup.quiz_status,
        total_checked_in: lookup.total_checked_in,
        total_claimed: lookup.total_claimed,
        api_id: lookup.api_id,
        event_id: lookup.event_id,
        deposit_enabled: lookup.deposit_enabled,
        deposit_amount_usdc: lookup.deposit_amount_usdc,
        deposit_amount_thb: lookup.deposit_amount_thb,
        participation_type: lookup.participation_type,
        claimed_signature: lookup.claimed_signature,
        claimed_asset_id: lookup.claimed_asset_id,
        claimed_wallet: lookup.claimed_wallet,
        cluster: lookup.cluster,
    };

    Ok(ApiOk::new(response))
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
) -> Result<ApiOk<ClaimResponse>, crate::error::WorkerError> {
    // The recipient wallet is validated and resolved server-side inside
    // execute_claim (locked column-P wallet > verified linked wallet > explicit
    // override), so a client address is only ever consulted for the override case.
    let requested = body.wallet_address.as_deref();
    if !body.use_linked_wallet
        && let Some(w) = requested
        && let Err(e) = validate_wallet_address(w)
    {
        tracing::warn!(claim_token = %token, error = %e, "invalid wallet address for claim");
        return Err(AppError::Validation(e).into());
    }

    let result = crate::claim::execute_claim(
        &state,
        &token,
        requested,
        body.use_linked_wallet,
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

    Ok(ApiOk::new(response))
}
