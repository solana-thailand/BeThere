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
//!
//! `POST /api/events/{id}/poster?kind=og` stores the event's share card
//! instead (`.issues/183` option B): a 1200×630 PNG the organizer's browser
//! draws on save, kept at `og/{event_id}.png` and named in the `og:image` of
//! `/e/{slug}`. It is checked on its bytes (PNG signature + IHDR size, 2 MB
//! cap) and leaves `poster_url` and the event record alone, so it can never
//! race the editor's `expected_updated_at`.

use axum::Extension;
use axum::body::{Bytes, to_bytes};
use axum::extract::{Path, Query, Request, State};
use serde::Deserialize;
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

/// What a `POST /api/events/{id}/poster` stores.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UploadKind {
    /// The marketing poster (`posters/{id}.{ext}`, sets `poster_url`).
    #[default]
    Poster,
    /// The browser-made share card (`og/{id}.png`).
    Og,
}

#[derive(Debug, Default, Deserialize)]
pub struct UploadQuery {
    #[serde(default)]
    kind: UploadKind,
}

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
    Query(query): Query<UploadQuery>,
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

    if query.kind == UploadKind::Og {
        return store_og_card(&state, &claims, &event.id, &headers, bytes).await;
    }

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
    let crate::event_store::SavedEvent {
        config: updated,
        d1_sync,
    } = crate::event_store::update_event(
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
        staff_fingerprint = %state.log_fingerprint(&claims.email),
        "poster uploaded"
    );

    Ok(ApiOk::new(json!({
        "id": updated.id,
        "poster_url": served_url,
        "updated_at": updated.updated_at,
        "warnings": d1_sync.warnings(),
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
    let crate::event_store::SavedEvent {
        config: updated,
        d1_sync,
    } = crate::event_store::update_event(
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
        staff_fingerprint = %state.log_fingerprint(&claims.email),
        "poster cleared"
    );

    Ok(ApiOk::new(json!({
        "id": updated.id,
        "poster_url": updated.poster_url,
        "updated_at": updated.updated_at,
        "warnings": d1_sync.warnings(),
    })))
}

/// Store the share card for `event_id` (the resolved event's own id, never
/// the path segment or a slug). The caller has checked the role.
async fn store_og_card(
    state: &AppState,
    claims: &Claims,
    event_id: &str,
    headers: &axum::http::HeaderMap,
    bytes: Bytes,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if ext_from_content_type(content_type) != Some("png") {
        return Err(AppError::Validation(format!(
            "share card content-type must be image/png (got '{content_type}')"
        ))
        .into());
    }
    event_checkin_domain::og_card::check_og_png(&bytes)
        .map_err(|e| AppError::Validation(e.to_string()))?;
    if !crate::og_meta::is_safe_id(event_id) {
        return Err(AppError::Validation("event id is not usable as a storage key".into()).into());
    }
    let Some(bucket) = state.r2.as_ref() else {
        return Err(AppError::Internal("R2 storage not configured".into()).into());
    };
    let key = storage::og_card_key(event_id);
    let size = bytes.len();
    storage::put_bytes(bucket, &key, bytes.into(), "image/png")
        .await
        .map_err(|e| AppError::Internal(format!("R2 share card put failed: {e:?}")))?;

    tracing::info!(
        event_id = %event_id,
        key = %key,
        size,
        staff_fingerprint = %state.log_fingerprint(&claims.email),
        "share card uploaded"
    );

    Ok(ApiOk::new(json!({
        "id": event_id,
        "og_image_url": format!("{}{event_id}", crate::og_meta::OG_CARD_PREFIX),
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
