#![allow(dead_code)]
//! R2 storage helpers for slip images, refund proofs, badge SVGs, and NFT metadata.
//!
//! The `ASSETS_BUCKET` R2 binding provides zero-egress object storage on Cloudflare's
//! edge network. Key prefixes organize different asset types:
//!
//! | Prefix | Content | Example Key |
//! |--------|---------|-------------|
//! | `slips/` | THB payment slip images | `slips/{event_id}/{attendee_id}.jpg` |
//! | `refunds/` | Refund transfer receipts | `refunds/{event_id}/{attendee_id}.jpg` |
//! | `badges/` | Event badge SVG files | `badges/{event_id}.svg` |
//! | `metadata/` | NFT metadata JSON | `metadata/{event_id}.json` |
//! | `exports/` | Walk-in CSV exports | `exports/{event_id}/{timestamp}.csv` |

use axum::{
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use worker::{Bucket, Result};

use crate::state::AppState;

/// R2 key prefix for THB payment slip images.
pub const PREFIX_SLIPS: &str = "slips/";

/// R2 key prefix for refund transfer receipt images.
pub const PREFIX_REFUNDS: &str = "refunds/";

/// R2 key prefix for event badge SVG files.
pub const PREFIX_BADGES: &str = "badges/";

/// R2 key prefix for NFT metadata JSON files.
pub const PREFIX_METADATA: &str = "metadata/";

/// R2 key prefix for walk-in CSV export files.
pub const PREFIX_EXPORTS: &str = "exports/";

/// Build an R2 key for a THB payment slip image.
pub fn slip_key(event_id: &str, attendee_id: &str) -> String {
    format!("{PREFIX_SLIPS}{event_id}/{attendee_id}")
}

/// Build an R2 key for a refund transfer receipt.
pub fn refund_key(event_id: &str, attendee_id: &str) -> String {
    format!("{PREFIX_REFUNDS}{event_id}/{attendee_id}")
}

/// Build an R2 key for an event badge SVG.
pub fn badge_key(event_id: &str) -> String {
    format!("{PREFIX_BADGES}{event_id}.svg")
}

/// Build an R2 key for an NFT metadata JSON file.
pub fn metadata_key(event_id: &str) -> String {
    format!("{PREFIX_METADATA}{event_id}.json")
}

/// Build an R2 key for a walk-in CSV export.
pub fn export_key(event_id: &str, timestamp: &str) -> String {
    format!("{PREFIX_EXPORTS}{event_id}/{timestamp}.csv")
}

/// Upload bytes to R2 with the given key.
/// Returns the public URL path (not the full URL — Workers serves via route).
pub async fn put_bytes(
    bucket: &Bucket,
    key: &str,
    data: Vec<u8>,
    content_type: &str,
) -> Result<String> {
    let metadata = worker::HttpMetadata {
        content_type: Some(content_type.to_string()),
        ..Default::default()
    };

    bucket
        .put(key, data)
        .http_metadata(metadata)
        .execute()
        .await
        .map_err(|e| worker::Error::RustError(format!("R2 put failed for key '{key}': {e:?}")))?;

    Ok(key.to_string())
}

/// Read bytes from R2 by key. Returns None if the key doesn't exist.
pub async fn get_bytes(bucket: &Bucket, key: &str) -> Result<Option<Vec<u8>>> {
    let object = bucket.get(key).execute().await?;

    match object {
        Some(obj) => {
            let body = obj
                .body()
                .ok_or_else(|| worker::Error::RustError("R2 object has no body".to_string()))?;
            let bytes = body.bytes().await?;
            Ok(Some(bytes))
        }
        None => Ok(None),
    }
}

/// Delete an object from R2 by key.
pub async fn delete(bucket: &Bucket, key: &str) -> Result<()> {
    bucket.delete(key).await
}

/// Check if an object exists in R2 (head only, no body download).
pub async fn exists(bucket: &Bucket, key: &str) -> Result<bool> {
    let object = bucket.head(key).await?;
    Ok(object.is_some())
}

/// Extract file extension from a filename for content-type mapping.
///
/// Used when serving R2 objects to set the correct `Content-Type` header.
pub fn content_type_from_key(key: &str) -> &'static str {
    match key.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        "csv" => "text/csv",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Legacy alias — prefer `content_type_from_key`.
pub fn content_type_from_filename(filename: &str) -> &'static str {
    content_type_from_key(filename)
}

// ---------------------------------------------------------------------------
// R2 serving handlers — specific routes per prefix
// ---------------------------------------------------------------------------

/// GET /api/storage/slips/{event_id}/{attendee_id}
///
/// Serves a THB payment slip image from R2.
#[worker::send]
pub async fn serve_slip(
    State(state): State<AppState>,
    Path((event_id, attendee_id)): Path<(String, String)>,
) -> Response {
    serve_r2_object(&state, &format!("{PREFIX_SLIPS}{event_id}/{attendee_id}")).await
}

/// GET /api/storage/refunds/{event_id}/{attendee_id}
///
/// Serves a refund transfer receipt image from R2.
#[worker::send]
pub async fn serve_refund(
    State(state): State<AppState>,
    Path((event_id, attendee_id)): Path<(String, String)>,
) -> Response {
    serve_r2_object(&state, &format!("{PREFIX_REFUNDS}{event_id}/{attendee_id}")).await
}

/// GET /api/storage/badges/{event_id}
///
/// Serves an event badge SVG from R2.
#[worker::send]
pub async fn serve_badge(State(state): State<AppState>, Path(event_id): Path<String>) -> Response {
    serve_r2_object(&state, &badge_key(&event_id)).await
}

/// Internal: serve an R2 object by key, trying common image extensions if
/// the exact key isn't found.
async fn serve_r2_object(state: &AppState, key: &str) -> Response {
    let Some(bucket) = state.r2.as_ref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "R2 storage not configured").into_response();
    };

    // Try the exact key first, then common image extensions
    let candidates = [
        key.to_string(),
        format!("{key}.jpg"),
        format!("{key}.png"),
        format!("{key}.webp"),
        format!("{key}.svg"),
    ];

    for candidate in &candidates {
        match get_bytes(bucket, candidate).await {
            Ok(Some(bytes)) => {
                let content_type = content_type_from_key(candidate);
                tracing::info!(key = %candidate, content_type = %content_type, size = bytes.len(), "serving R2 object");
                return (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, content_type),
                        (header::CACHE_CONTROL, "public, max-age=86400"),
                    ],
                    bytes,
                )
                    .into_response();
            }
            Ok(None) => continue,
            Err(e) => {
                // R2 errors for non-existent objects shouldn't be 500 to callers.
                // Log the error but return 404 so the frontend can handle gracefully.
                tracing::warn!(key = %candidate, error = ?e, "R2 get_bytes error (returning 404)");
                return (StatusCode::NOT_FOUND, "object not found").into_response();
            }
        }
    }

    (StatusCode::NOT_FOUND, "object not found").into_response()
}
