//! Attendee handlers for the Cloudflare Worker.
//!
//! Mirrors `src/handlers/attendee.rs` from the Axum build but uses
//! `crate::sheets` (worker::Fetch) and `crate::auth` (SubtleCrypto JWT)
//! instead of `reqwest` + `jsonwebtoken`.

use axum::{
    Extension,
    extract::{Path, Query, State},
};
use serde_json::json;

use worker::KvStore;

use crate::error::ApiOk;

use event_checkin_domain::models::api::{
    AttendeeListItem, AttendeeResponse, RecentCheckIn, StatsResponse,
};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use super::ext::{
    AttendeesQuery, EventIdQuery, resolve_event, resolve_event_with_access, resolve_kv,
};
use crate::sheets;
use crate::state::AppState;

/// GET /api/attendees
/// List attendees with cursor-based pagination and statistics.
///
/// Stats are computed over ALL attendees regardless of pagination.
/// Attendees are sorted by `row_index` ascending for deterministic pagination.
/// Use `cursor` (row_index of last item) and `limit` (page size) for pagination.
#[worker::send]
pub async fn list_attendees(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<AttendeesQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!("listing attendees (requested by: {})", claims.email);

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;

    let kv = resolve_kv(&state);
    let attendees = sheets::get_attendees(&state, &event.sheet_id, &event.sheet_name, kv)
        .await
        .map_err(|e| {
            tracing::error!("failed to fetch attendees: {e}");
            AppError::Internal(format!("failed to fetch attendees: {e}"))
        })?;

    // Compute statistics over ALL attendees (not paginated)
    let total_approved: usize = attendees.iter().filter(|a| a.is_approved()).count();

    let total_checked_in: usize = attendees.iter().filter(|a| a.is_checked_in()).count();

    let total_remaining: usize = total_approved.saturating_sub(total_checked_in);

    let check_in_percentage: f64 = if total_approved > 0 {
        (total_checked_in as f64 / total_approved as f64) * 100.0
    } else {
        0.0
    };

    let recent_check_ins: Vec<RecentCheckIn> = attendees
        .iter()
        .filter(|a| a.is_checked_in())
        .filter_map(|a| {
            a.checked_in_at.as_ref().map(|ts| RecentCheckIn {
                api_id: a.api_id.clone(),
                name: a.display_name().to_string(),
                checked_in_at: ts.clone(),
                checked_in_by: a.checked_in_by.clone(),
            })
        })
        .collect();

    let stats = StatsResponse {
        total_approved,
        total_checked_in,
        total_remaining,
        check_in_percentage: (check_in_percentage * 100.0).round() / 100.0,
        recent_check_ins,
    };

    // Cursor-based pagination: sort approved attendees by row_index,
    // filter by cursor, then take up to `page_limit`.
    let page_limit = query.limit.unwrap_or(200).min(200);

    let mut approved: Vec<_> = attendees.iter().filter(|a| a.is_approved()).collect();
    approved.sort_by_key(|a| a.row_index);

    let filtered: Vec<_> = match query.cursor {
        Some(cursor) => approved
            .into_iter()
            .filter(|a| a.row_index > cursor)
            .collect(),
        None => approved,
    };

    let has_more = filtered.len() > page_limit;
    let page: Vec<_> = filtered.into_iter().take(page_limit).collect();

    let next_cursor = if has_more {
        page.last().map(|a| a.row_index)
    } else {
        None
    };

    let attendee_responses: Vec<AttendeeListItem> = page
        .iter()
        .map(|a| AttendeeListItem::from_attendee(a))
        .collect();

    let data = json!({
        "attendees": attendee_responses,
        "stats": stats,
        "next_cursor": next_cursor,
        "has_more": has_more,
    });
    Ok(ApiOk::new(data))
}

/// GET /api/attendee/:id
/// Get a single attendee by their api_id.
/// Returns full attendee details including check-in status and QR code URL.
#[worker::send]
pub async fn get_attendee(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!("fetching attendee {id} (requested by: {})", claims.email);

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;

    let kv = resolve_kv(&state);
    let attendee = sheets::get_attendee_by_id(&id, &state, &event.sheet_id, &event.sheet_name, kv)
        .await
        .map_err(|e| {
            tracing::error!("failed to fetch attendee {id}: {e}");
            AppError::Internal(format!("failed to fetch attendee: {e}"))
        })?
        .ok_or_else(|| AppError::NotFound(format!("attendee with id '{id}' not found")))?;

    let response = AttendeeResponse::from_attendee(&attendee);

    // Generate a QR code image (cached in KV) if the attendee has a QR URL
    let qr_image = match attendee.qr_code_url.as_ref() {
        Some(url) => get_cached_qr_image(kv, &attendee.api_id, url).await,
        None => None,
    };

    let data = json!({
        "attendee": response,
        "qr_image": qr_image,
        "is_checked_in": attendee.is_checked_in(),
        "is_approved": attendee.is_approved(),
        "is_in_person": attendee.is_in_person(),
        "participation_type": attendee.participation_type,
    });
    Ok(ApiOk::new(data))
}

/// GET /api/public/ticket/:id
/// Public — no auth required. Returns attendee ticket data with QR image.
/// Masks email for privacy (e.g. "j***@example.com").
#[worker::send]
pub async fn get_public_ticket(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(attendee_id = %id, "public ticket requested");

    let event = resolve_event(&state, query.event_id.as_deref()).await?;

    let kv = resolve_kv(&state);
    let attendee = sheets::get_attendee_by_id(&id, &state, &event.sheet_id, &event.sheet_name, kv)
        .await
        .map_err(|e| {
            tracing::error!(attendee_id = %id, error = %e, "failed to fetch attendee for public ticket");
            AppError::Internal(format!("failed to fetch attendee: {e}"))
        })?
        .ok_or_else(|| AppError::NotFound(format!("attendee with id '{id}' not found")))?;

    let mut response = AttendeeResponse::from_attendee(&attendee);

    // Mask email for privacy: "john@example.com" → "j***@example.com"
    response.email = mask_email(&response.email);

    // Generate QR code image (cached in KV) if the attendee has a QR URL
    let qr_image = match attendee.qr_code_url.as_ref() {
        Some(url) => get_cached_qr_image(kv, &attendee.api_id, url).await,
        None => None,
    };

    let data = json!({
        "attendee": response,
        "qr_image": qr_image,
        "is_checked_in": attendee.is_checked_in(),
        "is_approved": attendee.is_approved(),
        "is_in_person": attendee.is_in_person(),
        "participation_type": attendee.participation_type,
    });
    Ok(ApiOk::new(data))
}

/// Mask an email address for privacy: "john@example.com" → "j***@example.com".
fn mask_email(email: &str) -> String {
    let Some((local, domain)) = email.split_once('@') else {
        return "***".to_string();
    };
    if local.is_empty() {
        return format!("***@{domain}");
    }
    let first = local.chars().next().unwrap_or('*');
    format!("{first}***@{domain}")
}

/// QR image cache TTL in seconds (1 hour).
const QR_IMAGE_CACHE_TTL_SECS: u64 = 3600;

/// Generate a QR base64 image, cached in KV.
///
/// Key: `qr:{api_id}`, TTL: 1 hour.
/// Falls back to uncached generation if KV is unavailable.
#[allow(clippy::collapsible_if)]
async fn get_cached_qr_image(
    kv: Option<&KvStore>,
    api_id: &str,
    qr_code_url: &str,
) -> Option<String> {
    let cache_key = format!("qr:{api_id}");

    // Try KV cache first
    if let Some(kv) = kv
        && let Ok(Some(cached)) = kv.get(&cache_key).text().await
    {
        return Some(cached);
    }

    // Generate fresh
    let image = event_checkin_domain::qr::generate_qr_base64(qr_code_url).ok()?;

    // Store in KV (best-effort, don't block on failure)
    if let Some(kv) = kv
        && let Ok(builder) = kv.put(&cache_key, image.clone())
        && let Err(e) = builder
            .expiration_ttl(QR_IMAGE_CACHE_TTL_SECS)
            .execute()
            .await
    {
        tracing::debug!(key = %cache_key, error = %e, "failed to cache QR image in KV");
    }

    Some(image)
}

/// POST /api/admin/flush-cache
/// Flush all server-side caches (attendee list + column mapping) for an event.
/// Use after changing sheet structure or headers.
#[worker::send]
pub async fn flush_cache(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!("flushing caches (requested by: {})", claims.email);

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;
    let kv = resolve_kv(&state);

    sheets::flush_caches(&state, &event.sheet_id, &event.sheet_name, kv).await;

    Ok(ApiOk::new(json!({
        "flushed": true,
        "event_id": event.id,
        "sheet_id": event.sheet_id,
    })))
}
