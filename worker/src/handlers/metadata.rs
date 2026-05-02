//! NFT metadata and badge endpoints.

use axum::extract::{Path, State};
use axum::response::{Html, Json};
use serde_json::json;

use crate::state::AppState;

/// GET /api/metadata/{event_id}
///
/// Returns Metaplex-compatible metadata JSON for an event's NFT.
/// Wallets and block explorers fetch this URI to display NFT details.
///
/// Uses global config defaults. Per-event KV loading is planned but
/// currently blocked by Axum Handler Send bound on wasm32 — the
/// metadata endpoint is called by wallets/explorers which use the
/// per-event fields passed during mint, so this generic fallback
/// is sufficient.
pub async fn get_metadata(
    State(state): State<AppState>,
    Path(event_id): Path<String>,
) -> Json<serde_json::Value> {
    tracing::info!("metadata request for event: {event_id}");

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
pub async fn get_badge_svg() -> Html<&'static str> {
    let svg = include_str!("../badge.svg");
    Html(svg)
}
