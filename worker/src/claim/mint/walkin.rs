//! Walk-in claim logic (D1 + KV best-effort).

use chrono::Utc;

use event_checkin_domain::models::attendee::WalkinAttendee;
use event_checkin_domain::models::error::AppError;

use crate::handlers::ext::resolve_kv;
use crate::solana::MintRequest;
use crate::state::AppState;

use super::helpers::crossmint_image_url;
use super::types::ClaimResult;
use crate::claim::lock::{
    FinalizeClaimLockParams, acquire_claim_lock, finalize_claim_lock, release_claim_lock,
};

/// Execute the claim flow for a walk-in attendee.
/// Walk-ins skip quiz/adventure gates. Claims are recorded in D1 (primary)
/// with KV best-effort mirror for legacy compatibility.
pub(super) async fn execute_walkin_claim(
    state: &AppState,
    event: &event_checkin_domain::models::event::EventConfig,
    token: &str,
    requested_wallet: Option<&str>,
    walkin: WalkinAttendee,
) -> Result<ClaimResult, AppError> {
    let display_name = walkin.name.clone();
    let kv = resolve_kv(state);

    // Walk-ins have no developer profile, so there's no linked wallet to mint to:
    // an explicit wallet address is required and validated server-side.
    let recipient = match requested_wallet.map(str::trim).filter(|w| !w.is_empty()) {
        Some(w) => w.to_string(),
        None => return Err(AppError::Validation("a wallet address is required".into())),
    };
    if let Err(e) = crate::solana::validate_wallet_address(&recipient) {
        tracing::warn!(claim_token = %token, error = %e, "walk-in recipient wallet invalid");
        return Err(AppError::Validation(e));
    }
    let wallet_address: &str = &recipient;

    // Note: walk-ins are always in-person — no online claim timing gate needed

    // Already claimed check
    if walkin.claimed_at.is_some() {
        tracing::warn!(claim_token = %token, "walk-in already claimed");
        return Err(AppError::Validation("NFT has already been claimed".into()));
    }

    // Claim dedup lock
    if let Some(kv) = kv
        && let Err(e) = acquire_claim_lock(
            kv,
            &event.id,
            token,
            wallet_address,
            state.d1.as_deref(),
            state.event_do.as_ref(),
        )
        .await
    {
        return Err(AppError::RateLimited(e));
    }

    // Mint compressed NFT via Crossmint (custodial signer + tree + fees)
    let config = &state.config;
    let mint_image = crossmint_image_url(&event.nft_image_url);
    let mint_req = MintRequest {
        wallet_address,
        host: &config.solana.crossmint_host,
        api_key: &config.solana.crossmint_api_key,
        collection_id: &config.solana.crossmint_collection_id,
        image_url: &mint_image,
        nft_name: &event.nft_name(),
        nft_description: &event.nft_description(),
        nft_external_url: &event.link,
        compressed: true,
        idempotency_key: token,
    };

    let mint_result = match crate::solana::mint_compressed_nft(&mint_req, kv).await {
        Ok(result) => result,
        Err(ref e) => {
            tracing::error!(claim_token = %token, error = %e, "walk-in mint failed");
            if let Some(kv) = kv {
                let _ = release_claim_lock(
                    kv,
                    &event.id,
                    token,
                    state.d1.as_deref(),
                    state.event_do.as_ref(),
                )
                .await;
            }
            return Err(AppError::External {
                service: "helius".into(),
                status: 502,
                body: e.to_string(),
            });
        }
    };

    // Mark as claimed in D1 (primary)
    let claimed_at = Utc::now().to_rfc3339();
    if let Some(ref d1) = state.d1
        && let Err(e) = crate::db::attendees::claim_attendee(
            d1,
            token,
            &claimed_at,
            &mint_result.asset_id,
            &mint_result.signature,
        )
        .await
    {
        tracing::error!(
            claim_token = %token,
            error = %e,
            "walk-in D1 claim write failed (mint succeeded, data may be inconsistent)"
        );
        // Don't fail the response — the NFT was already minted.
    }

    // Finalize claim lock
    if let Some(kv) = kv
        && let Err(e) = finalize_claim_lock(
            kv,
            FinalizeClaimLockParams {
                event_id: &event.id,
                token,
                wallet: wallet_address,
                asset_id: &mint_result.asset_id,
                signature: &mint_result.signature,
            },
            state.d1.as_deref(),
            state.event_do.as_ref(),
        )
        .await
    {
        tracing::warn!(error = %e, "walk-in claim lock finalize failed");
    }

    tracing::info!(
        claim_token = %token,
        name = %display_name,
        asset_id = %mint_result.asset_id,
        wallet_address = %wallet_address,
        "walk-in claim fulfilled"
    );

    let cluster = if config.solana.rpc_url.contains("mainnet") {
        "mainnet-beta"
    } else {
        "devnet"
    };

    Ok(ClaimResult {
        name: display_name,
        asset_id: mint_result.asset_id,
        signature: mint_result.signature,
        wallet_address: wallet_address.to_string(),
        claimed_at,
        cluster: cluster.to_string(),
    })
}
