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

use crate::error::ApiOk;

use event_checkin_domain::models::api::{AttendeeResponse, RecentCheckIn, StatsResponse};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use super::ext::{EventIdQuery, resolve_event, resolve_event_with_access, resolve_kv};
use crate::sheets;
use crate::state::AppState;

/// GET /api/attendees
/// List all attendees with optional filtering and statistics.
///
/// Returns attendee list and check-in statistics.
#[worker::send]
pub async fn list_attendees(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
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

    // Compute statistics
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

    let attendee_responses: Vec<AttendeeResponse> = attendees
        .iter()
        .filter(|a| a.is_approved())
        .map(AttendeeResponse::from_attendee)
        .collect();

    let stats = StatsResponse {
        total_approved,
        total_checked_in,
        total_remaining,
        check_in_percentage: (check_in_percentage * 100.0).round() / 100.0,
        recent_check_ins,
    };

    let data = json!({
        "attendees": attendee_responses,
        "stats": stats,
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

    // Also generate a QR code image if the attendee has a QR URL
    let qr_image = attendee
        .qr_code_url
        .as_ref()
        .and_then(|url| event_checkin_domain::qr::generate_qr_base64(url).ok());

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

    // Generate QR code image from the check-in URL
    let qr_image = attendee
        .qr_code_url
        .as_ref()
        .and_then(|url| event_checkin_domain::qr::generate_qr_base64(url).ok());

    // Also try generating QR from the attendee api_id directly if no qr_code_url
    let qr_image = qr_image.or_else(|| event_checkin_domain::qr::generate_qr_base64(&id).ok());

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
