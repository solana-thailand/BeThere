//! Check-in handler for the Cloudflare Worker.
//!
//! Mirrors `src/handlers/checkin.rs` from the Axum build but uses
//! `crate::sheets` (worker::Fetch) and `crate::auth` (SubtleCrypto JWT)
//! instead of `reqwest` + `jsonwebtoken`.

use axum::{
    Extension,
    extract::{Path, State},
    response::Json,
};
use serde_json::json;
use uuid::Uuid;

use event_checkin_domain::models::api::CheckInResponse;
use event_checkin_domain::models::attendee::Attendee;
use event_checkin_domain::models::auth::Claims;

use crate::sheets;
use crate::state::AppState;

/// POST /api/checkin/:id
/// Check in an attendee by their api_id.
///
/// This endpoint:
/// 1. Looks up the attendee by api_id
/// 2. Verifies the attendee is approved
/// 3. Checks if already checked in
/// 4. Updates columns I (timestamp), J (staff email), L (claim_token) in Google Sheets
/// 5. Generates a UUID v7 claim token for NFT/refund claim link
/// 6. Returns the check-in confirmation with claim URL
#[worker::send]
pub async fn check_in(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    tracing::info!("check-in request for {id} (by: {})", claims.email);

    // Fetch the attendee
    let attendee: Attendee = match sheets::get_attendee_by_id(&id, &state).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            tracing::warn!("check-in failed: attendee {id} not found");
            return Json(json!({
                "success": false,
                "error": format!("attendee with id '{id}' not found"),
            }));
        }
        Err(ref e) => {
            tracing::error!("check-in failed: could not fetch attendee {id}: {e}");
            return Json(json!({
                "success": false,
                "error": format!("failed to look up attendee: {e}"),
            }));
        }
    };

    // Check if attendee is approved
    if !attendee.is_approved() {
        tracing::warn!(
            "check-in denied: attendee {} has status '{}'",
            attendee.api_id,
            attendee.approval_status
        );
        return Json(json!({
            "success": false,
            "error": format!(
                "attendee is not approved (status: {})",
                attendee.approval_status
            ),
        }));
    }

    // Check if attendee is In-Person (not Online)
    if !attendee.is_in_person() {
        tracing::warn!(
            "check-in denied: attendee {} is not In-Person (participation: '{}')",
            attendee.api_id,
            attendee.participation_type
        );
        return Json(json!({
            "success": false,
            "error": format!(
                "attendee is not In-Person (participation type: {})",
                attendee.participation_type
            ),
        }));
    }

    // Check if already checked in
    if attendee.is_checked_in() {
        let checked_in_at = attendee.checked_in_at.as_deref().unwrap_or("unknown time");
        tracing::info!(
            "check-in skipped: attendee {} already checked in at {checked_in_at}",
            attendee.api_id
        );
        return Json(json!({
            "success": false,
            "error": "attendee is already checked in",
            "data": {
                "api_id": attendee.api_id,
                "name": attendee.display_name(),
                "checked_in_at": checked_in_at,
            },
        }));
    }

    // Generate claim token (UUID v7) and build claim URL
    let claim_token = Uuid::now_v7().to_string();
    let claim_url = format!("{}/{}", state.config.claim_base_url, claim_token);

    // Update the Google Sheet (writes timestamp, staff email, and claim_token)
    match sheets::mark_checked_in(attendee.row_index, &claims.email, &claim_token, &state).await {
        Ok(timestamp) => {
            tracing::info!(
                "check-in successful: {} ({}) at {timestamp} by {} claim_url={claim_url}",
                attendee.display_name(),
                attendee.api_id,
                claims.email
            );

            let response = CheckInResponse {
                api_id: attendee.api_id.clone(),
                name: attendee.display_name().to_string(),
                checked_in_at: timestamp,
                checked_in_by: claims.email.clone(),
                claim_url: Some(claim_url),
                message: format!("Successfully checked in {}", attendee.display_name()),
            };

            Json(json!({
                "success": true,
                "data": response,
            }))
        }
        Err(ref e) => {
            tracing::error!(
                "check-in failed: could not update sheet for {}: {e}",
                attendee.api_id
            );
            Json(json!({
                "success": false,
                "error": format!("failed to record check-in: {e}"),
            }))
        }
    }
}
