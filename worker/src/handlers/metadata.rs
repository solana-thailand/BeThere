//! NFT metadata and badge endpoints.
//!
//! Serves Metaplex-compatible metadata JSON and badge SVG for cNFTs.
//! These are used by wallets and explorers to render NFT details.

use axum::extract::{Path, State};
use axum::response::{Html, Json};
use serde_json::json;

use crate::state::AppState;

/// GET /api/metadata/{event_id}
///
/// Returns Metaplex-compatible metadata JSON for an event's NFT.
/// Wallets and block explorers fetch this URI to display NFT details.
pub async fn get_metadata(
    Path(event_id): Path<String>,
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let _ = event_id; // per-event metadata loading planned

    // Use config defaults for now — per-event override coming soon
    let metadata = json!({
        "name": "BeThere Badge",
        "symbol": "BETHERE",
        "description": "Proof of attendance",
        "image": &state.config.nft_image_url,
        "external_url": &state.config.claim_base_url,
        "attributes": [
            { "trait_type": "Type", "value": "Attendance Badge" },
            { "trait_type": "Platform", "value": "BeThere" },
        ],
        "properties": {
            "category": "image",
            "files": [
                { "uri": &state.config.nft_image_url, "type": "image/svg+xml" }
            ]
        }
    });

    Json(metadata)
}

/// GET /api/badge.svg
///
/// Returns the attendance badge SVG image.
/// Embedded in the WASM binary via `include_str!`.
pub async fn get_badge_svg() -> Html<&'static str> {
    let svg = include_str!("../badge.svg");
    Html(svg)
}
