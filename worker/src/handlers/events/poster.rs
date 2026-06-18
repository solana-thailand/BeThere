//! POST /api/events/{id}/poster   — upload a marketing poster to R2 (Organizer+).
//! DELETE /api/events/{id}/poster — clear the poster field + delete the R2 object.
//!
//! The poster is stored in the existing `ASSETS_BUCKET` R2 under `posters/{event_id}.{ext}`
//! (mirrors `badges/`). The served path `/api/storage/posters/{event_id}` is
//! extension-agnostic — `serve_r2_object` tries `.png/.jpg/.webp/.svg`, so the
//! stored `poster_url` never changes even if the format does on re-upload.
//!
//! Body convention: raw image bytes with a `Content-Type: image/*` header
//! (e.g. `image/png`). This avoids fragile multipart parsing in WASM and lets
//! the frontend upload with a single `fetch(url, { method: 'POST', body: blob })`.
//! A 5 MB cap is enforced before the R2 put to protect worker memory.

use axum::Extension;
use axum::body::{Bytes, to_bytes};
use axum::extract::{Path, Request, State};
use serde_json::json;

use crate::error::ApiOk;
use crate::state::AppState;
use crate::storage;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::UpdateEventRequest;

/// Maximum poster upload size (5 MB). Guards worker memory — multipart/raw
/// bodies are held in memory before the R2 put.
const MAX_POSTER_BYTES: usize = 5 * 1024 * 1024;

/// POST /api/events/{id}/poster — upload a marketing poster to R2.
///
/// Organizer-gated. Body is raw image bytes; `Content-Type` selects the
/// extension (`image/png` → `.png`, etc.). On success, persists the served
/// path `/api/storage/posters/{event_id}` to `EventConfig.poster_url`.
///
/// Re-upload with a different extension deletes the prior object (best-effort)
/// to avoid R2 orphans, then writes the new one.
#[worker::send]
pub async fn upload_poster(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<String>,
    req: Request,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let kv = state.events_kv.as_ref();

    // ── 1. Resolve event (KV first, D1 fallback) ────────────────────────────
    let event =
        crate::event_store::get_event_config_with_fallback(kv, state.d1.as_deref(), &event_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::NotFound(format!("event '{event_id}' not found")))?;

    // ── 2. Role check (Organizer+ for this event) ───────────────────────────
    let role = crate::auth::resolve_user_role(&claims.email, &state, Some(&event)).await;
    if role < crate::auth::UserRole::Organizer {
        return Err(AppError::Forbidden(
            "only super admins or organizers can upload event posters".into(),
        )
        .into());
    }

    // ── 3. Parse body bytes + detect extension from Content-Type ────────────
    let headers = req.headers().clone();
    let bytes = collect_body_bytes(req).await?;

    if bytes.len() > MAX_POSTER_BYTES {
        return Err(AppError::Validation(format!(
            "poster exceeds {} MB limit (got {} bytes)",
            MAX_POSTER_BYTES / (1024 * 1024),
            bytes.len()
        ))
        .into());
    }
    if bytes.is_empty() {
        return Err(AppError::Validation("poster body is empty".into()).into());
    }

    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let ext = ext_from_content_type(content_type).ok_or_else(|| {
        AppError::Validation(format!(
            "unsupported poster content-type '{content_type}' (expected image/png, image/jpeg, image/webp, or image/svg+xml)"
        ))
    })?;

    let Some(bucket) = state.r2.as_ref() else {
        return Err(AppError::Internal("R2 storage not configured".into()).into());
    };

    // ── 4. Best-effort delete of any prior poster object (other extensions) ─
    // serve_r2_object tries multiple extensions for reads, but stale uploads
    // with different extensions would otherwise accumulate as orphans.
    for stale_ext in ["png", "jpg", "webp", "svg"] {
        if stale_ext == ext {
            continue;
        }
        let stale_key = storage::poster_key(&event_id, stale_ext);
        if storage::exists(bucket, &stale_key).await.unwrap_or(false) {
            let _ = storage::delete(bucket, &stale_key).await;
        }
    }

    // ── 5. Put new object + persist served path ─────────────────────────────
    let key = storage::poster_key(&event_id, ext);
    let image_ct = content_type_to_store(content_type);
    storage::put_bytes(bucket, &key, bytes.into(), image_ct)
        .await
        .map_err(|e| AppError::Internal(format!("R2 poster put failed: {e:?}")))?;

    let served_url = format!("/api/storage/posters/{event_id}");
    let update_req = UpdateEventRequest {
        poster_url: Some(served_url.clone()),
        ..Default::default()
    };
    let updated = crate::event_store::update_event(
        kv,
        state.d1.as_deref(),
        &event_id,
        &update_req,
        &claims.email,
    )
    .await
    .map_err(AppError::Internal)?;

    tracing::info!(
        event_id = %event_id,
        key = %key,
        content_type = %image_ct,
        staff_email = %claims.email,
        "poster uploaded"
    );

    Ok(ApiOk::new(json!({
        "id": updated.id,
        "poster_url": served_url,
        "updated_at": updated.updated_at,
    })))
}

/// DELETE /api/events/{id}/poster — clear the poster field + remove R2 object.
///
/// Organizer-gated. Clears `EventConfig.poster_url` to empty string (hero
/// falls back to `nft_image_url`) and best-effort deletes any poster objects
/// in R2 across the known extensions.
#[worker::send]
pub async fn delete_poster(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let kv = state.events_kv.as_ref();

    let event =
        crate::event_store::get_event_config_with_fallback(kv, state.d1.as_deref(), &event_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::NotFound(format!("event '{event_id}' not found")))?;

    let role = crate::auth::resolve_user_role(&claims.email, &state, Some(&event)).await;
    if role < crate::auth::UserRole::Organizer {
        return Err(AppError::Forbidden(
            "only super admins or organizers can delete event posters".into(),
        )
        .into());
    }

    // Best-effort delete of any poster object across extensions.
    if let Some(bucket) = state.r2.as_ref() {
        for ext in ["png", "jpg", "webp", "svg"] {
            let key = storage::poster_key(&event_id, ext);
            if storage::exists(bucket, &key).await.unwrap_or(false) {
                let _ = storage::delete(bucket, &key).await;
            }
        }
    }

    // Clear the field via update_event (empty string = fall back to nft_image_url).
    let update_req = UpdateEventRequest {
        poster_url: Some(String::new()),
        ..Default::default()
    };
    let updated = crate::event_store::update_event(
        kv,
        state.d1.as_deref(),
        &event_id,
        &update_req,
        &claims.email,
    )
    .await
    .map_err(AppError::Internal)?;

    tracing::info!(
        event_id = %event_id,
        staff_email = %claims.email,
        "poster cleared"
    );

    Ok(ApiOk::new(json!({
        "id": updated.id,
        "poster_url": updated.poster_url,
        "updated_at": updated.updated_at,
    })))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Collect the full request body into bytes, enforcing the size cap at collect
/// time to avoid OOM on oversized uploads.
async fn collect_body_bytes(req: Request) -> Result<Bytes, AppError> {
    to_bytes(req.into_body(), MAX_POSTER_BYTES)
        .await
        .map_err(|e| AppError::Validation(format!("failed to read upload body: {e}")))
}

/// Map a `Content-Type` header value to a file extension for R2 key + serving.
fn ext_from_content_type(ct: &str) -> Option<&'static str> {
    // Strip parameters like "; charset=utf-8" — image types don't use these,
    // but be defensive in case a proxy adds them.
    let base = ct.split(';').next().unwrap_or("").trim().to_lowercase();
    match base.as_str() {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    }
}

/// Canonical content-type to store in R2 metadata (matches `ext_from_content_type`).
fn content_type_to_store(ct: &str) -> &'static str {
    match ext_from_content_type(ct) {
        Some("png") => "image/png",
        Some("jpg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}
