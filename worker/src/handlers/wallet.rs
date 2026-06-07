//! Wallet API handlers — on-chain NFT verification and leaderboard.
//!
//! Public endpoints for reading developer NFT inventories via Helius DAS API.

use std::collections::HashSet;

use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};

use crate::error::ApiOk;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct WalletNftsResponse {
    pub wallet_address: String,
    pub total_nfts: i64,
    pub campaign_mints: Vec<String>,
    pub nfts: Vec<NftItem>,
}

#[derive(Debug, Serialize)]
pub struct NftItem {
    pub asset_id: String,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub description: Option<String>,
    pub image_uri: Option<String>,
    pub collection_mint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub wallet_address: String,
    pub event_nfts: i64,
    pub campaign_nfts: i64,
    pub weighted_score: i64,
    pub tier: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LeaderboardResponse {
    pub entries: Vec<LeaderboardEntry>,
}

// ---------------------------------------------------------------------------
// GET /wallet/{address}/nfts
// ---------------------------------------------------------------------------

/// Fetch all compressed NFTs owned by a wallet address.
#[worker::send]
pub async fn get_wallet_nfts(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<ApiOk<WalletNftsResponse>, crate::error::WorkerError> {
    // Validate wallet address format
    if let Err(e) = crate::solana::validate_wallet_address(&address) {
        return Err(event_checkin_domain::models::error::AppError::Validation(e).into());
    }

    let config = &state.config;

    // Fetch first page of assets (up to 100 per page)
    let das_response = crate::solana::get_assets_by_owner(
        &config.solana.rpc_url,
        &config.solana.api_key,
        &address,
        1,
        100,
    )
    .await
    .map_err(
        |e| event_checkin_domain::models::error::AppError::External {
            service: "helius-das".into(),
            status: 502,
            body: e,
        },
    )?;

    let nfts: Vec<NftItem> = das_response
        .items
        .into_iter()
        .map(|asset| {
            let (name, symbol, description) = asset
                .content
                .as_ref()
                .and_then(|c| c.metadata.as_ref())
                .map(|m| (m.name.clone(), m.symbol.clone(), m.description.clone()))
                .unwrap_or((None, None, None));

            let image_uri = asset
                .content
                .as_ref()
                .and_then(|c| c.files.as_ref())
                .and_then(|f| f.first())
                .and_then(|f| f.uri.clone())
                .or_else(|| asset.content.as_ref().and_then(|c| c.json_uri.clone()));

            let collection_mint = asset
                .grouping
                .unwrap_or_default()
                .into_iter()
                .find(|g| g.group_key.as_deref() == Some("collection"))
                .and_then(|g| g.group_value);

            NftItem {
                asset_id: asset.id,
                name,
                symbol,
                description,
                image_uri,
                collection_mint,
            }
        })
        .collect();

    let total = nfts.len() as i64;

    // Fetch campaign collection mints so frontend can classify NFTs
    let campaign_mints = match state.d1.as_ref() {
        Some(d1) => crate::db::campaigns::campaign_collection_mints(d1)
            .await
            .unwrap_or_default(),
        None => vec![],
    };

    Ok(ApiOk::new(WalletNftsResponse {
        wallet_address: address,
        total_nfts: total,
        campaign_mints,
        nfts,
    }))
}

// ---------------------------------------------------------------------------
// GET /wallet/leaderboard
// ---------------------------------------------------------------------------

/// KV cache key for the leaderboard.
const KV_LEADERBOARD_KEY: &str = "leaderboard";
/// KV TTL for cached leaderboard (5 minutes).
const KV_LEADERBOARD_TTL: u64 = 300;
/// Maximum entries to return.
const MAX_LEADERBOARD_ENTRIES: usize = 50;

/// Leaderboard — queries D1 for developers with wallets, fetches NFTs
/// via Helius DAS, computes weighted scores, and caches in KV with 5-min TTL.
#[worker::send]
pub async fn get_leaderboard(
    State(state): State<AppState>,
) -> Result<ApiOk<LeaderboardResponse>, crate::error::WorkerError> {
    // 1. Try KV cache first
    if let Some(ref kv) = state.events_kv
        && let Some(cached) = kv.get(KV_LEADERBOARD_KEY).text().await.ok().flatten()
        && let Ok(resp) = serde_json::from_str::<LeaderboardResponse>(&cached)
    {
        tracing::debug!("leaderboard cache hit");
        return Ok(ApiOk::new(resp));
    }

    // 2. Query D1 for developers with wallets
    let d1 = state.d1.as_ref().ok_or_else(|| {
        event_checkin_domain::models::error::AppError::Internal("D1 database not configured".into())
    })?;

    let developers = crate::db::developers::list_developers_with_wallets(d1)
        .await
        .map_err(event_checkin_domain::models::error::AppError::Internal)?;

    if developers.is_empty() {
        let resp = LeaderboardResponse { entries: vec![] };
        cache_leaderboard(&state, &resp).await;
        return Ok(ApiOk::new(resp));
    }

    // 3. Fetch campaign collection mints for NFT classification
    let campaign_mint_set: HashSet<String> = crate::db::campaigns::campaign_collection_mints(d1)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

    tracing::debug!(
        count = campaign_mint_set.len(),
        "loaded campaign collection mints"
    );

    // 4. Fetch NFTs for each wallet via DAS API
    let config = &state.config;
    let mut entries: Vec<LeaderboardEntry> = Vec::new();

    for dev in &developers {
        match crate::solana::get_assets_by_owner(
            &config.solana.rpc_url,
            &config.solana.api_key,
            &dev.wallet_address,
            1,
            100,
        )
        .await
        {
            Ok(das_response) => {
                let (event_nfts, campaign_nfts) =
                    classify_nfts(&das_response.items, &campaign_mint_set);
                let score = calculate_weighted_score(event_nfts, campaign_nfts);

                entries.push(LeaderboardEntry {
                    wallet_address: dev.wallet_address.clone(),
                    event_nfts,
                    campaign_nfts,
                    weighted_score: score,
                    tier: compute_tier(score),
                });
            }
            Err(e) => {
                tracing::warn!(
                    wallet = %dev.wallet_address,
                    error = %e,
                    "DAS API failed for wallet, skipping in leaderboard"
                );
                // Treat as 0 NFTs — don't fail the entire leaderboard
            }
        }
    }

    // 5. Sort by weighted score descending, take top 50
    entries.sort_by_key(|e| std::cmp::Reverse(e.weighted_score));
    entries.truncate(MAX_LEADERBOARD_ENTRIES);

    let resp = LeaderboardResponse { entries };

    // 6. Cache in KV
    cache_leaderboard(&state, &resp).await;

    Ok(ApiOk::new(resp))
}

/// Cache the leaderboard response in KV with 5-minute TTL.
async fn cache_leaderboard(state: &AppState, resp: &LeaderboardResponse) {
    let kv = match state.events_kv {
        Some(ref kv) => kv,
        None => return,
    };

    let json_str = match serde_json::to_string(resp) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "failed to serialize leaderboard for KV");
            return;
        }
    };

    match kv.put(KV_LEADERBOARD_KEY, &json_str) {
        Ok(builder) => {
            if let Err(e) = builder.expiration_ttl(KV_LEADERBOARD_TTL).execute().await {
                tracing::warn!(error = ?e, "failed to cache leaderboard in KV");
            }
        }
        Err(e) => {
            tracing::warn!(error = ?e, "failed to build leaderboard KV put");
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers for scoring
// ---------------------------------------------------------------------------

/// Compute tier from weighted score.
fn compute_tier(score: i64) -> String {
    if score >= 10 {
        "Legend".to_string()
    } else if score >= 5 {
        "Dedicated".to_string()
    } else if score >= 3 {
        "Collector".to_string()
    } else if score >= 1 {
        "Participant".to_string()
    } else {
        "Newcomer".to_string()
    }
}

/// Classify NFTs into event vs campaign based on collection_mint.
/// NFTs whose `collection_mint` matches an active campaign are counted as campaign NFTs;
/// all others are counted as event NFTs.
fn classify_nfts(
    items: &[crate::solana::DasAsset],
    campaign_mints: &HashSet<String>,
) -> (i64, i64) {
    let mut event_nfts = 0i64;
    let mut campaign_nfts = 0i64;

    for asset in items {
        let collection_mint = asset.grouping.as_ref().and_then(|groups| {
            groups
                .iter()
                .find(|g| g.group_key.as_deref() == Some("collection"))
                .and_then(|g| g.group_value.as_ref())
        });

        match collection_mint {
            Some(mint) if campaign_mints.contains(mint) => campaign_nfts += 1,
            _ => event_nfts += 1,
        }
    }

    (event_nfts, campaign_nfts)
}

/// Calculate weighted score: event NFTs = 1pt each, campaign NFTs = 3pts each.
fn calculate_weighted_score(event_nfts: i64, campaign_nfts: i64) -> i64 {
    event_nfts + (campaign_nfts * 3)
}
