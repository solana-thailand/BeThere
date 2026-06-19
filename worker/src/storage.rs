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
//! | `posters/` | Event marketing posters | `posters/{event_id}.{ext}` |
//! | `metadata/` | NFT metadata JSON | `metadata/{event_id}.json` |
//! | `exports/` | Walk-in CSV exports | `exports/{event_id}/{timestamp}.csv` |

use axum::{
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use worker::{Bucket, Result};

// `JsString` for building JS string args when calling the raw R2 binding directly.
use js_sys::JsString;

// `Digest` trait is needed for `Md5::digest` (R2 put checksum workaround).
use md5::Digest;

use crate::state::AppState;

/// R2 key prefix for THB payment slip images.
pub const PREFIX_SLIPS: &str = "slips/";

/// R2 key prefix for refund transfer receipt images.
pub const PREFIX_REFUNDS: &str = "refunds/";

/// R2 key prefix for event badge SVG files.
pub const PREFIX_BADGES: &str = "badges/";

/// R2 key prefix for event marketing poster images.
pub const PREFIX_POSTERS: &str = "posters/";

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

/// Build an R2 key for an event marketing poster.
/// Extension is derived from the uploaded content-type (png/jpg/webp/svg).
pub fn poster_key(event_id: &str, ext: &str) -> String {
    format!("{PREFIX_POSTERS}{event_id}.{ext}")
}

/// Build an R2 key for an NFT metadata JSON file.
pub fn metadata_key(event_id: &str) -> String {
    format!("{PREFIX_METADATA}{event_id}.json")
}

/// Build an R2 key for a walk-in CSV export.
pub fn export_key(event_id: &str, timestamp: &str) -> String {
    format!("{PREFIX_EXPORTS}{event_id}/{timestamp}.csv")
}

/// Upload bytes to R2 with the given key. Returns the public URL path
/// (not the full URL — Workers serves via route).
///
/// NOTE: `content_type` is intentionally NOT set as R2 `http_metadata`, and
/// an MD5 checksum IS provided via `.md5(...)`. Both work around defects in
/// `worker` 0.8.x:
///   - The default `HttpMetadata.cache_expiry` (`Option::None`) serializes to
///     JS `null`, which Cloudflare's R2 binding rejects ("cacheExpiry ... not
///     of type 'date'"). Serving (`serve_r2_object`) derives Content-Type
///     from the key extension via `content_type_from_key`, so the stored
///     metadata is unused anyway.
///   - `PutOptionsBuilder` unconditionally sends an `md5` field (the default
///     `checksum_algorithm` is "md5") whose value is `null` when no checksum
///     is set — also rejected ("md5 ... not of type 'JsBufferSource or
///     string''). We compute the real MD5 so the field is valid.
///
/// The `content_type` arg is kept for logging/tracing only.
pub async fn put_bytes(
    bucket: &Bucket,
    key: &str,
    data: Vec<u8>,
    content_type: &str,
) -> Result<String> {
    // R2 expects an MD5 digest here (matches the builder's default algorithm).
    let digest = md5::Md5::digest(&data).to_vec();

    bucket
        .put(key, data)
        .md5(digest)
        .execute()
        .await
        .map_err(|e| worker::Error::RustError(format!("R2 put failed for key '{key}': {e:?}")))?;

    tracing::debug!(key = %key, content_type = %content_type, "R2 object written");
    Ok(key.to_string())
}

/// Read bytes from R2 by key. Returns `None` if the key doesn't exist.
///
/// Calls the R2 `get` binding **directly** via the raw JS handle instead of
/// `worker::Bucket::get`, because the `worker` 0.8.x get builder always sends
/// `{ onlyIf: null, range: null }`, and the `range: null` makes R2 throw
/// internal error 10001 on every get (even for missing keys). Calling
/// `bucket.get(key)` with no options avoids the bug. The body stream is
/// materialized via `new Response(body).arrayBuffer()`.
pub async fn get_bytes(bucket: &js_sys::Object, key: &str) -> Result<Option<Vec<u8>>> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    // bucket.get(key) — call the JS method with no options object.
    let get_fn = js_sys::Reflect::get(bucket.as_ref(), &JsString::from("get"))
        .map_err(|e| worker::Error::RustError(format!("R2 get lookup failed: {e:?}")))?;
    let get_fn: js_sys::Function = get_fn
        .dyn_into()
        .map_err(|_| worker::Error::RustError("R2 bucket.get is not a function".into()))?;

    let promise = get_fn
        .call1(bucket.as_ref(), &JsString::from(key))
        .map_err(|e| worker::Error::RustError(format!("R2 get(key) threw: {e:?}")))?;
    let promise: js_sys::Promise = promise.into();

    let value = JsFuture::from(promise).await?;

    // Missing key → R2 returns null.
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }

    let r2_obj = js_sys::Object::from(value);

    // body is a ReadableStream (or undefined if the object has no body).
    let body = js_sys::Reflect::get(&r2_obj, &JsString::from("body"))
        .map_err(|e| worker::Error::RustError(format!("R2 object.body access failed: {e:?}")))?;
    if body.is_null() || body.is_undefined() {
        return Err(worker::Error::RustError(
            "R2 object has no body (onlyIf precondition may have failed)".into(),
        ));
    }

    // Materialize the stream via Response.arrayBuffer().
    let response = web_sys::Response::new_with_opt_readable_stream(Some(body.unchecked_ref()))
        .map_err(|e| worker::Error::RustError(format!("new Response(stream) failed: {e:?}")))?;
    let ab_promise = response
        .array_buffer()
        .map_err(|e| worker::Error::RustError(format!("Response.arrayBuffer() failed: {e:?}")))?;
    let ab = JsFuture::from(ab_promise).await?;

    let arr = js_sys::Uint8Array::new(&ab);
    let mut out = vec![0u8; arr.length() as usize];
    arr.copy_to(&mut out);
    Ok(Some(out))
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

/// GET /api/storage/posters/{event_id}
///
/// Serves an event marketing poster from R2. `serve_r2_object` tries common
/// image extensions (.png/.jpg/.webp/.svg) so the served path is extension-agnostic,
/// letting organizers re-upload in a different format without changing the stored URL.
#[worker::send]
pub async fn serve_poster(State(state): State<AppState>, Path(event_id): Path<String>) -> Response {
    serve_r2_object(&state, &format!("{PREFIX_POSTERS}{event_id}")).await
}

/// Internal: serve an R2 object by key, trying common image extensions if
/// the exact key isn't found.
///
/// Distinguishes "key not found" (HTTP 404) from an actual R2 error
/// (HTTP 500). Previously *any* R2 error was masked as 404, which hid
/// failures like the `worker` 0.8.x get-options `null`-serialization bug.
async fn serve_r2_object(state: &AppState, key: &str) -> Response {
    // Reads use the RAW R2 handle to bypass the worker 0.8.x get-options bug
    // (see get_bytes). Head/exists use the typed Bucket, which is unaffected.
    let Some(bucket_raw) = state.r2_raw.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "R2 storage not configured (raw binding missing)",
        )
            .into_response();
    };
    let bucket_typed = state.r2.as_ref();

    // Try the exact key first, then common image extensions.
    let candidates = [
        key.to_string(),
        format!("{key}.jpg"),
        format!("{key}.png"),
        format!("{key}.webp"),
        format!("{key}.svg"),
    ];

    // Track the most recent R2 error separately from "not found" so we can
    // return 500 (R2 broken) vs 404 (genuinely missing).
    let mut last_error: Option<String> = None;

    for candidate in &candidates {
        match get_bytes(bucket_raw, candidate).await {
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
            Ok(None) => {
                tracing::debug!(key = %candidate, "R2 object not found, trying next extension");
                continue;
            }
            Err(e) => {
                // Record the error but keep trying other extensions — a
                // transient/serialization error on one key shouldn't mask a
                // valid object stored under a different extension.
                tracing::warn!(key = %candidate, error = ?e, "R2 get_bytes error");
                last_error = Some(format!("{e:?}"));
                continue;
            }
        }
    }

    if let Some(err) = last_error {
        // An R2-level error occurred (not merely "missing"). Surface it as 500
        // with the underlying error so it is diagnosable instead of masked.
        tracing::error!(base_key = %key, error = %err, "R2 serve failed after trying all extensions");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("R2 read failed: {err}"),
        )
            .into_response();
    }

    tracing::info!(base_key = %key, "R2 object not found under any extension");

    // Independent existence probe via `head` (uses a different code path than
    // `get` — no options object — so it helps distinguish "object genuinely
    // missing" from a broken `get` builder).
    if let Some(bucket_typed) = bucket_typed {
        for probe_ext in ["png", "jpg", "webp", "svg"] {
            let probe_key = format!("{key}.{probe_ext}");
            match exists(bucket_typed, &probe_key).await {
                Ok(true) => {
                    tracing::error!(probe_key = %probe_key, "HEAD says object EXISTS but get returned nothing — get builder is broken");
                }
                Ok(false) => {
                    tracing::debug!(probe_key = %probe_key, "HEAD confirms object absent");
                }
                Err(e) => {
                    tracing::warn!(probe_key = %probe_key, error = ?e, "HEAD probe failed");
                }
            }
        }
    }

    (StatusCode::NOT_FOUND, "object not found").into_response()
}
