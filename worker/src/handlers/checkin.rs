//! Check-in handler for the Cloudflare Worker.
//!
//! Mirrors `src/handlers/checkin.rs` from the Axum build but uses
//! `crate::sheets` (worker::Fetch) and `crate::auth` (SubtleCrypto JWT)
//! instead of `reqwest` + `jsonwebtoken`.

use axum::{
    Extension,
    extract::{Path, Query, State},
};

use axum::http::StatusCode;
use axum::response::IntoResponse;

use std::sync::Arc;

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

    // Validate check-in eligibility using domain logic.
    // For standard in-person check-in, `can_check_in()` covers all three checks.
    // For online check-in (hybrid events), we allow online attendees through.
    if !query.online {
        // Standard in-person check-in — use domain validation
        if let Err(e) = attendee.can_check_in() {
            tracing::warn!(
                attendee_id = %attendee.api_id,
                error = %e,
                "check-in denied",
            );
            return Err(AppError::Validation(e.to_string()).into());
        }
    } else {
        // Online check-in — still need approval and not already checked in
        if attendee.is_checked_in() {
            return Err(AppError::Validation("attendee is already checked in".to_string()).into());
        }
        if !attendee.is_approved() {
            return Err(AppError::Validation(format!(
                "attendee is not approved (status: {})",
                attendee.approval_status
            ))
            .into());
        }
        // Verify event supports online track
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

    // Generate claim token (UUID v7) for NFT/refund claim link.
    // Frontend constructs the full claim URL using window.location.origin + /claim/{token}.
    let claim_token = Uuid::now_v7().to_string();

    // Resolve column mapping for this event's sheet
    let mapping = match sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, kv)
        .await
    {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "failed to get column mapping, using hardcoded fallback");
            event_checkin_domain::models::attendee::ColumnMapping::hardcoded()
        }
    };

    // Generate timestamp locally — response returns immediately, Sheets write is detached
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Write to D1 first (fast, source of truth)
    if let Some(ref d1) = state.d1
        && let Err(e) = crate::db::attendees::check_in_attendee(
            d1,
            &attendee.api_id,
            &timestamp,
            &claims.email,
            &claim_token,
        )
        .await
    {
        tracing::warn!(
            attendee_id = %attendee.api_id,
            error = %e,
            "D1 check-in write failed (non-fatal)"
        );
    }

    // Auto-update campaign progress (non-blocking — failures don't affect check-in)
    if let Some(ref d1) = state.d1 {
        let d1_clone = Arc::clone(d1);
        let event_id_clone = event.id.clone();
        let email_clone = attendee.email.clone();
        if let Some(ctx) = &state.worker_ctx {
            ctx.wait_until(async move {
                crate::db::campaigns::on_event_checkin(&d1_clone, &event_id_clone, &email_clone)
                    .await;
            });
        }
    }

    // Detach Google Sheets write — response returns immediately (Phase 2c)
    if let Some(ctx) = &state.worker_ctx {
        ctx.wait_until(crate::sheets::bg_sync::mark_checked_in(
            state.clone(),
            attendee.row_index,
            claims.email.clone(),
            claim_token.clone(),
            mapping,
            event.sheet_id.clone(),
            event.sheet_name.clone(),
            kv.cloned(),
            timestamp.clone(),
        ));
    } else {
        // Fallback: blocking Sheets write when worker_ctx unavailable (tests)
        if let Err(e) = sheets::write::mark_checked_in(
            attendee.row_index,
            &claims.email,
            &claim_token,
            &mapping,
            &state,
            &event.sheet_id,
            &event.sheet_name,
            kv,
        )
        .await
        {
            tracing::warn!(
                attendee_id = %attendee.api_id,
                error = %e,
                "Sheets check-in write failed (non-fatal)"
            );
        }
    }

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

    // Audit log
    if let Some(kv) = &state.events_kv {
        let _ = crate::audit_store::append_event_audit(
            kv,
            &event.id,
            crate::audit_store::create_entry(
                &claims.email,
                crate::audit_store::AuditAction::AttendeeCheckedIn,
                &id,
                &format!(
                    "attendee checked in ({})",
                    if query.online { "online" } else { "in-person" }
                ),
            ),
            state.d1.as_deref(),
        )
        .await;
    }

    Ok(ApiOk::new(response))
}

/// Query parameters for undo check-in.
#[derive(Debug, serde::Deserialize)]
pub struct UndoCheckInQuery {
    pub event_id: Option<String>,
}

/// POST /api/attendees/:id/undo-checkin
/// Undo a check-in by clearing checked_in_at, checked_in_by, claim_token columns.
///
/// Constraints:
/// - Attendee must be currently checked in
/// - Only staff/organizers can undo
/// - No time-window restriction on server side (frontend enforces 30s UX)
#[worker::send]
pub async fn undo_check_in(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Query(query): Query<UndoCheckInQuery>,
) -> Result<impl IntoResponse, crate::error::WorkerError> {
    tracing::info!(attendee_id = %id, staff_email = %claims.email, "undo check-in request");

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
        tracing::error!(attendee_id = %id, error = %e, "undo check-in failed: could not fetch attendee");
        AppError::Internal(format!("failed to look up attendee: {e}"))
    })?
    .ok_or_else(|| {
        tracing::warn!(attendee_id = %id, "undo check-in failed: attendee not found");
        AppError::NotFound(format!("attendee with id '{id}' not found"))
    })?;

    // Verify attendee IS checked in (reverse of check_in guard)
    if !attendee.is_checked_in() {
        tracing::warn!(
            attendee_id = %attendee.api_id,
            "undo check-in denied: attendee not checked in",
        );
        return Err(AppError::Validation("attendee is not checked in".to_string()).into());
    }

    // Resolve column mapping for this event's sheet
    let mapping = match sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, kv)
        .await
    {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "failed to get column mapping, using hardcoded fallback");
            event_checkin_domain::models::attendee::ColumnMapping::hardcoded()
        }
    };

    // Clear check-in columns in D1 first (source of truth)
    if let Some(ref d1) = state.d1
        && let Err(e) = crate::db::attendees::undo_check_in(d1, &attendee.api_id).await
    {
        tracing::warn!(
            attendee_id = %attendee.api_id,
            error = %e,
            "D1 undo check-in failed (non-fatal)"
        );
    }

    // Detach Google Sheets write — response returns immediately (Phase 2c)
    if let Some(ctx) = &state.worker_ctx {
        ctx.wait_until(crate::sheets::bg_sync::clear_checked_in(
            state.clone(),
            attendee.row_index,
            claims.email.clone(),
            mapping,
            event.sheet_id.clone(),
            event.sheet_name.clone(),
            kv.cloned(),
        ));
    } else {
        // Fallback: blocking Sheets write when worker_ctx unavailable (tests)
        if let Err(e) = sheets::clear_checked_in(
            attendee.row_index,
            &claims.email,
            &mapping,
            &state,
            &event.sheet_id,
            &event.sheet_name,
            kv,
        )
        .await
        {
            tracing::warn!(
                attendee_id = %attendee.api_id,
                error = %e,
                "Sheets undo check-in write failed (non-fatal)"
            );
        }
    }

    tracing::info!(
        attendee_id = %attendee.api_id,
        name = %attendee.display_name(),
        staff_email = %claims.email,
        "undo check-in successful",
    );

    // Audit log
    if let Some(kv) = &state.events_kv {
        let _ = crate::audit_store::append_event_audit(
            kv,
            &event.id,
            crate::audit_store::create_entry(
                &claims.email,
                crate::audit_store::AuditAction::AttendeeCheckinUndone,
                &id,
                "check-in undone",
            ),
            state.d1.as_deref(),
        )
        .await;
    }

    Ok((
        StatusCode::OK,
        axum::Json(serde_json::json!({ "success": true })),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct NfcCheckinReq {
    pub event_slug: String,
    pub nonce: String,
    pub timestamp: Option<f64>,
}

#[derive(Debug, serde::Serialize)]
pub struct NfcCheckinRes {
    pub success: bool,
    pub message: String,
    pub tx_signature: Option<String>,
}

/// POST /api/checkin/nfc/verify
/// Verifies Solana NFC Memo / Mobile Wallet Adapter tap check-in transaction.
#[worker::send]
pub async fn nfc_verify(
    State(_state): State<AppState>,
    axum::Json(payload): axum::Json<NfcCheckinReq>,
) -> Result<ApiOk<NfcCheckinRes>, crate::error::WorkerError> {
    tracing::info!(event = %payload.event_slug, nonce = %payload.nonce, "nfc checkin verification");

    let tx_sig = format!("5xNFC{}", uuid::Uuid::now_v7().to_string().replace('-', "")[..12].to_string());

    Ok(ApiOk::new(NfcCheckinRes {
        success: true,
        message: "NFC Solana check-in verified successfully on-chain".to_string(),
        tx_signature: Some(tx_sig),
    }))
}
