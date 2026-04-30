//! Claim API handlers for NFT minting.
//!
//! Public endpoints (no auth required) for attendees to:
//! - Look up their claim status by token (GET /api/claim/{token})
//! - Mint a compressed NFT to their wallet (POST /api/claim/{token})

use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use event_checkin_domain::models::api::{
    ClaimLookupResponse, ClaimResponse, EventConfig, QuizStatus,
};
use event_checkin_domain::models::event::EventConfig as DomainEventConfig;

use crate::event_store;
use crate::solana::{self, validate_wallet_address};
use crate::state::AppState;

/// Mask a wallet address for safe display in error messages.
/// Shows first 4 and last 4 characters: "BxRW...3KjF".
/// Returns "****" if the address is too short to mask.
fn mask_wallet(addr: &str) -> String {
    if addr.len() > 8 {
        format!("{}...{}", &addr[..4], &addr[addr.len() - 4..])
    } else {
        "****".to_string()
    }
}

/// Request body for POST /api/claim/{token}.
#[derive(Debug, Deserialize)]
pub struct ClaimRequest {
    pub wallet_address: String,
}

/// Optional event_id query parameter for event-scoped requests.
#[derive(Debug, Deserialize)]
pub struct EventIdQuery {
    pub event_id: Option<String>,
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
) -> Json<serde_json::Value> {
    tracing::info!("claim lookup for token: {token}");

    let event: DomainEventConfig = match event_store::resolve_event_or_fallback(
        state.events_kv.as_ref(),
        query.event_id.as_deref(),
        &state.config,
    )
    .await
    {
        Ok(e) => e,
        Err(e) => {
            return Json(json!({ "success": false, "error": e }));
        }
    };

    let attendee = match crate::sheets::get_attendee_by_claim_token(
        &token,
        &state,
        &event.sheet_id,
        &event.sheet_name,
    )
    .await
    {
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
    let nft_available = !event.nft_metadata_uri.is_empty()
        && !event.nft_image_url.is_empty()
        && !state.config.helius_api_key.is_empty();

    let api_event = EventConfig {
        event_name: event.name.clone(),
        event_tagline: event.tagline.clone(),
        event_link: event.link.clone(),
        event_start_ms: event.event_start_ms,
        event_end_ms: event.event_end_ms,
    };

    // Pre-registered wallet from column P — locks claim to this address if present
    let locked_wallet = attendee
        .solana_address
        .as_ref()
        .map(|w| w.trim().to_string())
        .filter(|w| !w.is_empty());

    // Determine quiz status (Issue 002 — activity-gated claim)
    let quiz_status = match &state.events_kv {
        Some(kv) => {
            let eid = &event.id;
            crate::quiz::get_quiz_status(kv, eid, &token)
                .await
                .unwrap_or(QuizStatus::NotRequired)
        }
        None => match &state.quiz_kv {
            Some(kv) => crate::quiz::get_quiz_status(kv, "default", &token)
                .await
                .unwrap_or(QuizStatus::NotRequired),
            None => QuizStatus::NotRequired,
        },
    };

    let response = ClaimLookupResponse {
        name: display_name,
        checked_in_at,
        claim_token: token.clone(),
        claimed,
        claimed_at,
        nft_available,
        locked_wallet,
        event: api_event,
        quiz_status,
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
    Query(query): Query<EventIdQuery>,
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

    // Resolve event context
    let event: DomainEventConfig = match event_store::resolve_event_or_fallback(
        state.events_kv.as_ref(),
        query.event_id.as_deref(),
        &state.config,
    )
    .await
    {
        Ok(e) => e,
        Err(e) => {
            return Json(json!({ "success": false, "error": e }));
        }
    };

    // Look up attendee by claim token
    let attendee = match crate::sheets::get_attendee_by_claim_token(
        &token,
        &state,
        &event.sheet_id,
        &event.sheet_name,
    )
    .await
    {
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

    // Quiz gate — must pass quiz before claiming (Issue 002)
    let quiz_check = if let Some(ref kv) = state.events_kv {
        Some((kv, event.id.as_str()))
    } else {
        state.quiz_kv.as_ref().map(|kv| (kv, "default"))
    };

    if let Some((kv, eid)) = quiz_check {
        let quiz_status = match crate::quiz::get_quiz_status(kv, eid, &token).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("claim mint: failed to check quiz status for token {token}: {e}");
                return Json(json!({
                    "success": false,
                    "error": format!("failed to verify quiz status: {e}"),
                }));
            }
        };
        match quiz_status {
            QuizStatus::NotRequired => {} // no quiz configured, proceed
            QuizStatus::Passed => {}      // quiz passed, proceed
            QuizStatus::NotStarted => {
                tracing::warn!("claim mint blocked: quiz not attempted for token {token}");
                return Json(json!({
                    "success": false,
                    "error": "you must complete the quiz before claiming your badge",
                    "quiz_status": "not_started",
                }));
            }
            QuizStatus::InProgress => {
                tracing::warn!("claim mint blocked: quiz not passed for token {token}");
                return Json(json!({
                    "success": false,
                    "error": "you must pass the quiz before claiming your badge",
                    "quiz_status": "in_progress",
                }));
            }
        }
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

    // Wallet match guard: if attendee pre-registered a Solana address (column P),
    // the claiming wallet must match exactly. Prevents claim theft via leaked URLs.
    if let Some(ref registered) = attendee.solana_address {
        let registered = registered.trim();
        if !registered.is_empty() {
            let claiming = body.wallet_address.trim();
            if registered != claiming {
                tracing::warn!(
                    "wallet mismatch for token {token}: registered={} claiming={}",
                    mask_wallet(registered),
                    mask_wallet(claiming)
                );
                return Json(json!({
                    "success": false,
                    "error": format!(
                        "This claim is locked to a pre-registered wallet ({})",
                        mask_wallet(registered)
                    ),
                }));
            }
        }
    }

    // Mint compressed NFT via Helius
    let config = &state.config;
    let mint_result = match solana::mint_compressed_nft(
        &body.wallet_address,
        &config.helius_rpc_url,     // global
        &config.helius_api_key,     // global
        &event.nft_collection_mint, // per-event
        &event.nft_metadata_uri,    // per-event
        &event.nft_image_url,       // per-event
        &event.nft_name(),          // per-event (template resolved)
        &event.nft_symbol,          // per-event
        &event.nft_description(),   // per-event (template resolved)
        &event.link,                // per-event (externalUrl)
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
        &event.sheet_id,
        &event.sheet_name,
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

    let cluster = if config.helius_rpc_url.contains("mainnet") {
        "mainnet-beta"
    } else {
        "devnet"
    };

    let response = ClaimResponse {
        name: display_name,
        asset_id: mint_result.asset_id,
        signature: mint_result.signature,
        wallet_address: body.wallet_address,
        claimed_at,
        cluster: cluster.to_string(),
    };

    Json(json!({
        "success": true,
        "data": response,
    }))
}
