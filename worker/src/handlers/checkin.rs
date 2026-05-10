//! Check-in handler for the Cloudflare Worker.
//!
//! Mirrors `src/handlers/checkin.rs` from the Axum build but uses
//! `crate::sheets` (worker::Fetch) and `crate::auth` (SubtleCrypto JWT)
//! instead of `reqwest` + `jsonwebtoken`.

use axum::{
    Extension,
    extract::{Path, Query, State},
};

use crate::error::ApiOk;
use uuid::Uuid;

use event_checkin_domain::models::api::CheckInResponse;
use event_checkin_domain::models::attendee::Attendee;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use super::ext::{resolve_event_with_access, resolve_kv};
use crate::sheets;
use crate::state::AppState;

/// Query parameters for check-in.
#[derive(Debug, serde::Deserialize)]
pub struct CheckInQuery {
    pub event_id: Option<String>,
    /// Allow checking in online attendees (virtual check-in for hybrid events).
    /// Default: false — only In-Person attendees can be checked in.
    #[serde(default)]
    pub online: bool,
}

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
    Query(query): Query<CheckInQuery>,
) -> Result<ApiOk<CheckInResponse>, crate::error::WorkerError> {
    tracing::info!(attendee_id = %id, staff_email = %claims.email, online = query.online, "check-in request");

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;

    // Fetch the attendee
    let kv = resolve_kv(&state);
    let attendee: Attendee = sheets::get_attendee_by_id(
        &id,
        &state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
    )
    .await
    .map_err(|e| {
        tracing::error!(attendee_id = %id, error = %e, "check-in failed: could not fetch attendee");
        AppError::Internal(format!("failed to look up attendee: {e}"))
    })?
    .ok_or_else(|| {
        tracing::warn!(attendee_id = %id, "check-in failed: attendee not found");
        AppError::NotFound(format!("attendee with id '{id}' not found"))
    })?;

    // Check if attendee is approved
    if !attendee.is_approved() {
        tracing::warn!(
            attendee_id = %attendee.api_id,
            status = %attendee.approval_status,
            "check-in denied: attendee not approved",
        );
        return Err(AppError::Validation(format!(
            "attendee is not approved (status: {})",
            attendee.approval_status
        ))
        .into());
    }

    // Check if attendee is In-Person (not Online).
    // Online attendees can only be checked in if `?online=true` and the event is Hybrid.
    if !attendee.is_in_person() {
        if !query.online {
            tracing::warn!(
                attendee_id = %attendee.api_id,
                participation_type = %attendee.participation_type,
                "check-in denied: attendee not In-Person",
            );
            return Err(AppError::Validation(format!(
                "attendee is not In-Person (participation type: {})",
                attendee.participation_type
            ))
            .into());
        }
        // online=true: verify event supports online track
        if !event.event_format.has_online() {
            tracing::warn!(
                attendee_id = %attendee.api_id,
                event_format = %event.event_format,
                "online check-in denied: event has no online track",
            );
            return Err(AppError::Validation(
                "online check-in not available for this event format".to_string(),
            )
            .into());
        }
        tracing::info!(
            attendee_id = %attendee.api_id,
            participation_type = %attendee.participation_type,
            "virtual check-in for online attendee",
        );
    }

    // Check if already checked in
    if attendee.is_checked_in() {
        let checked_in_at = attendee.checked_in_at.as_deref().unwrap_or("unknown time");
        tracing::info!(
            attendee_id = %attendee.api_id,
            checked_in_at = %checked_in_at,
            "check-in skipped: already checked in",
        );
        return Err(AppError::Validation("attendee is already checked in".to_string()).into());
    }

    // Generate claim token (UUID v7) for NFT/refund claim link.
    // Frontend constructs the full claim URL using window.location.origin + /claim/{token}.
    let claim_token = Uuid::now_v7().to_string();

    // Update the Google Sheet (writes timestamp, staff email, and claim_token)
    let timestamp = sheets::mark_checked_in(
        attendee.row_index,
        &claims.email,
        &claim_token,
        &state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
    )
    .await
    .map_err(|e| {
        tracing::error!(
            attendee_id = %attendee.api_id,
            error = %e,
            "check-in failed: could not update sheet",
        );
        AppError::Internal(format!("failed to record check-in: {e}"))
    })?;

    tracing::info!(
        attendee_id = %attendee.api_id,
        name = %attendee.display_name(),
        staff_email = %claims.email,
        claim_token = %claim_token,
        checked_in_at = %timestamp,
        "check-in successful",
    );

    let response = CheckInResponse {
        api_id: attendee.api_id.clone(),
        name: attendee.display_name().to_string(),
        checked_in_at: timestamp,
        checked_in_by: claims.email.clone(),
        claim_token: Some(claim_token),
        message: format!("Successfully checked in {}", attendee.display_name()),
    };

    Ok(ApiOk::new(response))
}
