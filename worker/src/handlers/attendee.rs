//! Attendee handlers for the Cloudflare Worker.
//!
//! Mirrors `src/handlers/attendee.rs` from the Axum build but uses
//! `crate::sheets` (worker::Fetch) and `crate::auth` (SubtleCrypto JWT)
//! instead of `reqwest` + `jsonwebtoken`.

use axum::{
    Extension,
    extract::{Path, State},
    response::Json,
};
use serde_json::json;

use event_checkin_domain::models::api::{AttendeeResponse, RecentCheckIn, StatsResponse};
use event_checkin_domain::models::auth::Claims;

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
) -> Json<serde_json::Value> {
    tracing::info!("listing attendees (requested by: {})", claims.email);

    let attendees = match sheets::get_attendees(&state).await {
        Ok(a) => a,
        Err(ref e) => {
            tracing::error!("failed to fetch attendees: {e}");
            return Json(json!({
                "success": false,
                "error": format!("failed to fetch attendees: {e}"),
            }));
        }
    };

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

    Json(json!({
        "success": true,
        "data": {
            "attendees": attendee_responses,
            "stats": stats,
        },
    }))
}

/// GET /api/attendee/:id
/// Get a single attendee by their api_id.
/// Returns full attendee details including check-in status and QR code URL.
#[worker::send]
pub async fn get_attendee(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    tracing::info!("fetching attendee {id} (requested by: {})", claims.email);

    let attendee = match sheets::get_attendee_by_id(&id, &state).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            return Json(json!({
                "success": false,
                "error": format!("attendee with id '{id}' not found"),
            }));
        }
        Err(ref e) => {
            tracing::error!("failed to fetch attendee {id}: {e}");
            return Json(json!({
                "success": false,
                "error": format!("failed to fetch attendee: {e}"),
            }));
        }
    };

    let response = AttendeeResponse::from_attendee(&attendee);

    // Also generate a QR code image if the attendee has a QR URL
    let qr_image = attendee
        .qr_code_url
        .as_ref()
        .and_then(|url| event_checkin_domain::qr::generate_qr_base64(url).ok());

    // Build claim URL from claim_token if attendee has been checked in
    let claim_url = attendee
        .claim_token
        .as_ref()
        .map(|token| format!("{}/{}", state.config.claim_base_url, token));

    Json(json!({
        "success": true,
        "data": {
            "attendee": response,
            "qr_image": qr_image,
            "is_checked_in": attendee.is_checked_in(),
            "is_approved": attendee.is_approved(),
            "is_in_person": attendee.is_in_person(),
            "participation_type": attendee.participation_type,
            "claim_url": claim_url,
        },
    }))
}
