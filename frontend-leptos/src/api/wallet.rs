//! Wallet API client — on-chain NFT verification and leaderboard.

use serde::Deserialize;

use super::types::ApiError;
use super::{api_get, fetch::response_json};

// ===== Types =====

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WalletNftsResponse {
    #[serde(default)]
    pub wallet_address: String,
    #[serde(default)]
    pub total_nfts: i64,
    #[serde(default)]
    pub campaign_mints: Vec<String>,
    #[serde(default)]
    pub nfts: Vec<NftItem>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct NftItem {
    #[serde(default)]
    pub asset_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub image_uri: Option<String>,
    #[serde(default)]
    pub collection_mint: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LeaderboardResponse {
    #[serde(default)]
    pub entries: Vec<LeaderboardEntry>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LeaderboardEntry {
    #[serde(default)]
    pub wallet_address: String,
    #[serde(default)]
    pub event_nfts: i64,
    #[serde(default)]
    pub campaign_nfts: i64,
    #[serde(default)]
    pub weighted_score: i64,
    #[serde(default)]
    pub tier: String,
}

// ===== Tier helpers =====

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Newcomer,
    Participant,
    Collector,
    Dedicated,
    Legend,
}

impl Tier {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Newcomer => "Newcomer",
            Self::Participant => "Participant",
            Self::Collector => "Collector",
            Self::Dedicated => "Dedicated",
            Self::Legend => "Legend",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Newcomer => "🌱",
            Self::Participant => "⭐",
            Self::Collector => "🏆",
            Self::Dedicated => "💎",
            Self::Legend => "👑",
        }
    }

    pub fn score_threshold(&self) -> i64 {
        match self {
            Self::Newcomer => 0,
            Self::Participant => 1,
            Self::Collector => 3,
            Self::Dedicated => 5,
            Self::Legend => 10,
        }
    }
}

/// Compute tier from weighted score.
pub fn compute_tier(score: i64) -> Tier {
    if score >= 10 {
        Tier::Legend
    } else if score >= 5 {
        Tier::Dedicated
    } else if score >= 3 {
        Tier::Collector
    } else if score >= 1 {
        Tier::Participant
    } else {
        Tier::Newcomer
    }
}

/// Calculate weighted score: event NFTs = 1pt each, campaign NFTs = 3pts each.
pub fn calculate_weighted_score(event_nfts: i64, campaign_nfts: i64) -> i64 {
    event_nfts + (campaign_nfts * 3)
}

// ===== API Functions =====

/// GET /api/wallet/{address}/nfts — fetch all compressed NFTs owned by wallet
pub async fn get_wallet_nfts(address: &str) -> Result<WalletNftsResponse, ApiError> {
    let path = format!("/wallet/{address}/nfts");
    let response = api_get(&path).await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Failed to fetch wallet NFTs".to_string(),
            status: response.status(),
        });
    }

    let result: super::types::ApiResponse<WalletNftsResponse> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse wallet NFTs: {e}"),
            status: 0,
        })?;

    result.data.ok_or_else(|| ApiError {
        message: "No data in wallet NFTs response".to_string(),
        status: 0,
    })
}

/// GET /api/wallet/leaderboard — global NFT ranking
pub async fn get_leaderboard() -> Result<LeaderboardResponse, ApiError> {
    let response = api_get("/wallet/leaderboard").await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Failed to fetch leaderboard".to_string(),
            status: response.status(),
        });
    }

    let result: super::types::ApiResponse<LeaderboardResponse> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse leaderboard: {e}"),
            status: 0,
        })?;

    result.data.ok_or_else(|| ApiError {
        message: "No data in leaderboard response".to_string(),
        status: 0,
    })
}
