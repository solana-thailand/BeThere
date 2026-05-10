//! NFT metadata and badge endpoints.

use axum::extract::{Path, State};
use axum::response::{Html, Json};
use serde_json::json;

use crate::error::WorkerError;
use crate::event_store;
use crate::state::AppState;

/// GET /api/metadata/{event_id}
///
/// Returns Metaplex-compatible metadata JSON for an event's NFT.
/// Wallets and block explorers fetch this URI to display NFT details.
///
/// Loads per-event NFT fields from KV, falling back to global config
/// when the event is not found or a field is empty.
#[worker::send]
pub async fn get_metadata(
    State(state): State<AppState>,
    Path(event_id): Path<String>,
) -> Result<Json<serde_json::Value>, WorkerError> {
    tracing::info!(event_id = %event_id, "metadata requested");

    let event = match state.events_kv.as_ref() {
        Some(kv) => match event_store::get_event_config(kv, &event_id).await {
            Ok(Some(e)) => Some(e),
            Ok(None) => {
                tracing::warn!(event_id = %event_id, "metadata: event not found in KV");
                None
            }
            Err(err) => {
                tracing::warn!(event_id = %event_id, error = %err, "metadata: failed to load event from KV");
                None
            }
        },
        None => {
            tracing::info!("metadata: EVENTS KV not configured, using global config");
            None
        }
    };

    let name = event
        .as_ref()
        .map(|e| e.nft_name())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "BeThere Badge".to_string());

    let symbol = event
        .as_ref()
        .and_then(|e| {
            let s = &e.nft_symbol;
            if s.is_empty() { None } else { Some(s.clone()) }
        })
        .unwrap_or_else(|| "BETHERE".to_string());

    let description = event
        .as_ref()
        .map(|e| e.nft_description())
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "Proof of attendance".to_string());

    let image = event
        .as_ref()
        .and_then(|e| {
            let u = &e.nft_image_url;
            if u.is_empty() { None } else { Some(u.clone()) }
        })
        .unwrap_or_else(|| state.config.nft.image_url.clone());

    let external_url = event
        .as_ref()
        .and_then(|e| {
            let u = &e.claim_base_url;
            if u.is_empty() { None } else { Some(u.clone()) }
        })
        .unwrap_or_else(|| state.config.server.claim_base_url.clone());

    // Build attributes: include event-specific data when available
    let mut attributes = vec![
        json!({ "trait_type": "Type", "value": "Attendance Badge" }),
        json!({ "trait_type": "Platform", "value": "BeThere" }),
    ];

    if let Some(ref e) = event {
        if !e.name.is_empty() {
            attributes.push(json!({ "trait_type": "Event", "value": &e.name }));
        }
        if e.event_start_ms > 0 {
            attributes.push(json!({ "trait_type": "Event Date", "value": e.event_start_ms }));
        }
    }

    let metadata = json!({
        "name": name,
        "symbol": symbol,
        "description": description,
        "image": image,
        "external_url": external_url,
        "attributes": attributes,
        "properties": {
            "category": "image",
            "files": [
                { "uri": image, "type": "image/svg+xml" }
            ]
        }
    });

    Ok(Json(metadata))
}

/// GET /api/badge.svg
///
/// Simple 200x200 badge for thumbnails and quick previews.
pub async fn get_badge_svg() -> Html<&'static str> {
    let svg = include_str!("../badge.svg");
    Html(svg)
}

/// GET /api/badge-hd.svg
///
/// Production 1000x1000 badge for NFT display.
/// Use this URL as `nft_image_url` in the admin UI for best quality.
pub async fn get_badge_hd_svg() -> Html<&'static str> {
    let svg = include_str!("../badge_production.svg");
    Html(svg)
}
