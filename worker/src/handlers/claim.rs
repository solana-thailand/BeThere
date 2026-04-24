//! Claim API handlers for NFT minting.
//!
//! Public endpoints (no auth required) for attendees to:
//! - Look up their claim status by token (GET /api/claim/{token})
//! - Mint a compressed NFT to their wallet (POST /api/claim/{token})

use axum::{
    extract::{Path, State},
    response::Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use event_checkin_domain::models::api::{ClaimLookupResponse, ClaimResponse};

use crate::solana::{self, validate_wallet_address};
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
) -> Json<serde_json::Value> {
    tracing::info!("claim lookup for token: {token}");

    let attendee = match crate::sheets::get_attendee_by_claim_token(&token, &state).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            tracing::warn!("claim lookup: no attendee found for token {token}");
            return Json(json!({
                "success": false,
                "error": "claim token not found",
            }));
        }
        Err(ref e) => {
            tracing::error!("claim lookup failed for token {token}: {e}");
            return Json(json!({
                "success": false,
                "error": format!("failed to look up claim: {e}"),
            }));
        }
    };

    let display_name = attendee.display_name().to_string();
    let checked_in_at = attendee.checked_in_at.clone().unwrap_or_default();
    let claimed = attendee.claimed_at.is_some();
    let claimed_at = attendee.claimed_at.clone();

    // Check if NFT minting is fully configured (all required secrets present)
    let nft_available = !state.config.helius_api_key.is_empty()
        && !state.config.nft_metadata_uri.is_empty()
        && !state.config.nft_image_url.is_empty();

    let response = ClaimLookupResponse {
        name: display_name,
        checked_in_at,
        claim_token: token,
        claimed,
        claimed_at,
        nft_available,
    };

    Json(json!({
        "success": true,
        "data": response,
    }))
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
    Json(body): Json<ClaimRequest>,
) -> Json<serde_json::Value> {
    tracing::info!("claim mint request for token: {token}");

    // Validate wallet address format
    if let Err(e) = validate_wallet_address(&body.wallet_address) {
        tracing::warn!("invalid wallet address for claim {token}: {e}");
        return Json(json!({
            "success": false,
            "error": e,
        }));
    }

    // Look up attendee by claim token
    let attendee = match crate::sheets::get_attendee_by_claim_token(&token, &state).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            tracing::warn!("claim mint: no attendee found for token {token}");
            return Json(json!({
                "success": false,
                "error": "claim token not found",
            }));
        }
        Err(ref e) => {
            tracing::error!("claim mint lookup failed for token {token}: {e}");
            return Json(json!({
                "success": false,
                "error": format!("failed to look up claim: {e}"),
            }));
        }
    };

    let display_name = attendee.display_name().to_string();

    // Must be checked in
    if attendee.checked_in_at.is_none() {
        return Json(json!({
            "success": false,
            "error": "attendee has not been checked in yet",
        }));
    }

    // Must not be already claimed
    if attendee.claimed_at.is_some() {
        let claimed_at = attendee.claimed_at.as_deref().unwrap_or("unknown");
        tracing::warn!("claim already fulfilled for token {token} at {claimed_at}");
        return Json(json!({
            "success": false,
            "error": "NFT has already been claimed",
            "data": {
                "name": display_name,
                "claimed_at": claimed_at,
            },
        }));
    }

    // Mint compressed NFT via Helius
    let config = &state.config;
    let mint_result = match solana::mint_compressed_nft(
        &body.wallet_address,
        &config.helius_rpc_url,
        &config.helius_api_key,
        &config.nft_collection_mint,
        &config.nft_metadata_uri,
        &config.nft_image_url,
    )
    .await
    {
        Ok(result) => result,
        Err(ref e) => {
            tracing::error!("mint failed for token {token}: {e}");
            return Json(json!({
                "success": false,
                "error": format!("failed to mint NFT: {e}"),
            }));
        }
    };

    // Mark as claimed in Google Sheet (column G = wallet, column M = claimed_at)
    let claimed_at = Utc::now().to_rfc3339();
    if let Err(ref e) = crate::sheets::mark_claimed(
        attendee.row_index,
        &body.wallet_address,
        &claimed_at,
        &state,
    )
    .await
    {
        tracing::error!("mint succeeded but failed to mark claimed for token {token}: {e}");
        return Json(json!({
            "success": false,
            "error": format!(
                "NFT minted but failed to record claim. Asset ID: {}. Error: {e}",
                mint_result.asset_id
            ),
        }));
    }

    tracing::info!(
        "claim fulfilled: token={token} name={display_name} asset_id={} wallet={}",
        mint_result.asset_id,
        body.wallet_address
    );

    let response = ClaimResponse {
        name: display_name,
        asset_id: mint_result.asset_id,
        signature: mint_result.signature,
        wallet_address: body.wallet_address,
        claimed_at,
    };

    Json(json!({
        "success": true,
        "data": response,
    }))
}
