//! NFT metadata and badge endpoints.
//!
//! Serves Metaplex-compatible metadata JSON and badge SVG for cNFTs.
//! These are used by wallets and explorers to render NFT details.

use axum::{
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Json},
};
use serde_json::json;

use crate::state::AppState;

/// GET /api/metadata/{event_id}
/// Serve Metaplex-compatible NFT metadata JSON for a given event.
/// This is the `uri` field stored on-chain in the compressed NFT leaf.
#[worker::send]
pub async fn get_metadata(
    State(state): State<AppState>,
    Path(_event_id): Path<String>,
) -> Json<serde_json::Value> {
    let config = &state.config;
    let event_name = &config.event_name;
    let image_url = if !config.nft_image_url.is_empty() {
        config.nft_image_url.clone()
    } else {
        format!("{}/api/badge.svg", config.server_url)
    };

    Json(json!({
        "name": format!("BeThere - {}", event_name),
        "symbol": "BETHERE",
        "description": format!("Proof of attendance at {}", event_name),
        "image": image_url,
        "external_url": config.server_url,
        "attributes": [
            { "trait_type": "Type", "value": "Attendance Badge" },
            { "trait_type": "Platform", "value": "BeThere" },
            { "trait_type": "Network", "value": "Solana" },
            { "trait_type": "Format", "value": "Compressed NFT" }
        ],
        "properties": {
            "category": "image",
            "files": [
                { "uri": image_url, "type": "image/svg+xml" }
            ]
        }
    }))
}

/// GET /api/badge.svg
/// Serve the BeThere attendance badge SVG.
/// Used as the NFT image by wallets and explorers.
pub async fn get_badge_svg() -> impl IntoResponse {
    let svg = include_str!("../badge.svg");
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        svg,
    )
}
