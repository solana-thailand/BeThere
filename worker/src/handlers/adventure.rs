//! Adventure API handlers for the Rust Adventures feature.
//!
//! Public endpoints:
//!   GET  /api/adventure/config             — public adventure config (enabled, required_level)
//!   GET  /api/adventure/{token}/status     — current adventure progress
//!   POST /api/adventure/{token}/save       — save level completion
//!
//! Admin endpoint (protected):
//!   GET  /api/admin/adventure              — get adventure config
//!   PUT  /api/admin/adventure              — create/update adventure config

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::json;

use crate::error::ApiOk;
use event_checkin_domain::models::attendee::Attendee;

/// Resolve claim_token from D1 when Sheets doesn't have it.
///
/// The attendee from Sheets may have `claim_token: None` because the token was
/// generated and stored in D1 during registration but hasn't synced to Sheets yet.
/// This helper reads D1 to recover the real token.
pub async fn resolve_claim_token_from_d1(
    state: &crate::state::AppState,
    attendee: &Attendee,
) -> Option<String> {
    // Only query D1 if the Sheets attendee doesn't have a valid claim_token
    if attendee.claim_token.as_ref().is_some_and(|t| !t.is_empty()) {
        return attendee.claim_token.clone();
    }

    let d1 = state.d1.as_deref()?;
    let d1_attendee = crate::db::attendees::get_attendee_by_id(d1, &attendee.api_id)
        .await
        .ok()??;

    if d1_attendee
        .claim_token
        .as_ref()
        .is_some_and(|t| !t.is_empty())
    {
        tracing::info!(
            attendee_id = %attendee.api_id,
            claim_token = d1_attendee.claim_token.as_deref().unwrap_or("(none)"),
            "resolved claim_token from D1 (Sheets had none)"
        );
        d1_attendee.claim_token.clone()
    } else {
        None
    }
}

use event_checkin_domain::models::adventure::{
    AdventureConfig, AdventureSaveRequest, AdventureStatus,
};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use super::ext::{EventIdQuery, resolve_event, resolve_event_with_access, resolve_kv};
use crate::adventure;
use crate::error::WorkerError;
use crate::state::AppState;

/// Request body for quest-complete check-in.
#[derive(Debug, Deserialize)]
pub struct QuestCompleteRequest {
    pub event_id: String,
}

/// GET /api/adventure/config
/// Public endpoint: returns enabled + required_level for an event's adventure.
/// Used by the adventure page in casual (no-token) mode to determine quest requirements.
#[worker::send]
pub async fn get_public_adventure_config(
    State(state): State<AppState>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let event = resolve_event(&state, query.event_id.as_deref()).await?;
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    let config = adventure::get_adventure_config(db, &event.id)
        .await
        .map_err(AppError::Internal)?;

    let (enabled, required_level) = match config {
        Some(c) => (c.enabled, c.required_level),
        None => (false, None),
    };

    Ok(ApiOk::new(json!({
        "event_id": event.id,
        "event_slug": event.slug,
        "enabled": enabled,
        "required_level": required_level,
    })))
}

/// GET /api/adventure/{token}/status
/// Get adventure status for a claim token.
#[worker::send]
pub async fn get_adventure_status(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let event = resolve_event(&state, query.event_id.as_deref()).await?;
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    let status = adventure::get_adventure_status(db, &event.id, &token)
        .await
        .map_err(AppError::Internal)?;

    let progress = adventure::get_adventure_progress(db, &event.id, &token)
        .await
        .map_err(AppError::Internal)?;

    Ok(ApiOk::new(json!({
        "status": status,
        "progress": progress,
    })))
}

/// POST /api/adventure/{token}/save
/// Save level completion progress.
///
/// Security: validates that the claim token belongs to a checked-in attendee
/// before saving progress. Mirrors the quiz handler's validation pattern.
#[worker::send]
pub async fn save_adventure_progress(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<EventIdQuery>,
    Json(body): Json<AdventureSaveRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    // Validate token matches body
    if body.claim_token != token {
        return Err(AppError::Validation("token mismatch".to_string()).into());
    }

    let event = resolve_event(&state, query.event_id.as_deref()).await?;

    // Verify claim token belongs to a checked-in attendee
    let sheets_kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());
    match crate::sheets::get_attendee_by_claim_token(
        &token,
        &state,
        &event.sheet_id,
        &event.sheet_name,
        sheets_kv,
    )
    .await
    {
        Ok(Some(_)) => {} // valid checked-in attendee
        Ok(None) => {
            tracing::warn!(
                claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(&token),
                "adventure save: invalid claim token"
            );
            return Err(AppError::NotFound(
                "invalid claim token — you must be checked in first".to_string(),
            )
            .into());
        }
        Err(ref e) => {
            tracing::error!(
                claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(&token),
                error = %e,
                "adventure save: failed to look up claim token"
            );
            return Err(AppError::Internal("failed to verify claim token".to_string()).into());
        }
    }

    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    // Get config to determine required levels
    let config = adventure::get_adventure_config(db, &event.id)
        .await
        .map_err(AppError::Internal)?;

    // Determine required level prefixes from config.
    // required_level is 0-based index: n=2 means levels 01..03 must be completed.
    // If None, all 10 levels must be completed.
    // If adventure is not enabled, pass empty vec to skip passed-check entirely.
    const TOTAL_LEVELS: usize = 10;
    let required_levels: Vec<String> = match &config {
        Some(c) if c.enabled => match c.required_level {
            Some(n) => (1..=(n + 1)).map(|i| format!("{i:02}")).collect(),
            None => (1..=TOTAL_LEVELS).map(|i| format!("{i:02}")).collect(),
        },
        _ => vec![],
    };

    let progress = adventure::save_level_completion(
        db,
        &event.id,
        &token,
        &body.level_id,
        body.score,
        &required_levels,
    )
    .await
    .map_err(AppError::Internal)?;

    Ok(ApiOk::new(json!({
        "progress": progress,
    })))
}

/// GET /api/admin/adventure
/// Get adventure configuration (admin only).
#[worker::send]
pub async fn get_admin_adventure(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!("admin adventure config read by {}", claims.email);

    // S2: per-event access check (was resolve_event — cross-event read IDOR).
    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    let config = adventure::get_adventure_config(db, &event.id)
        .await
        .map_err(AppError::Internal)?;

    Ok(ApiOk::new(json!({
        "event_id": event.id,
        "config": config,
    })))
}

/// PUT /api/admin/adventure
/// Create or update adventure configuration (admin only).
#[worker::send]
pub async fn put_admin_adventure(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
    Json(body): Json<AdventureConfig>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!(
        "admin adventure config update by {} (enabled={})",
        claims.email,
        body.enabled
    );

    // S2: per-event access check (was resolve_event — cross-event write IDOR).
    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    adventure::save_adventure_config(db, &event.id, &body)
        .await
        .map_err(AppError::Internal)?;

    Ok(ApiOk::new(json!({
        "event_id": event.id,
        "enabled": body.enabled,
        "required_level": body.required_level,
    })))
}

/// POST /api/adventure/quest-complete
/// Public endpoint for authenticated users to record virtual check-in after adventure completion.
/// Requires JWT auth. Looks up attendee by email + event_id, verifies adventure is configured
/// and the required levels are completed in D1, then sets checked_in_at.
#[worker::send]
pub async fn quest_complete_checkin(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<QuestCompleteRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let event = resolve_event(&state, Some(&body.event_id))
        .await
        .map_err(|e| AppError::NotFound(e.to_string()))?;

    if !event.event_format.has_online() {
        return Err(AppError::Validation("event does not support online track".into()).into());
    }

    // Find attendee by email
    let kv = resolve_kv(&state);
    let attendees = crate::sheets::get_attendees_for_event(
        &state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
        &event.id,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to fetch attendees: {e}")))?;

    let attendee = attendees
        .iter()
        .find(|a| a.email.eq_ignore_ascii_case(&claims.email))
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "no registration found for {} at this event",
                claims.email
            ))
        })?;

    // Already checked in — idempotent success, not an error: the frontend polls
    // this endpoint and a repeat completion must still hand back the claim token.
    if attendee.is_checked_in() {
        let final_ct = resolve_claim_token_from_d1(&state, attendee).await;
        return Ok(ApiOk::new(json!({
            "status": "already_checked_in",
            "claim_token": final_ct,
            "event_slug": event.slug,
        })));
    }

    // Check if adventure is configured and enabled
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    let config = crate::adventure::get_adventure_config(db, &event.id)
        .await
        .map_err(AppError::Internal)?;

    let Some(config) = config else {
        return Err(
            AppError::Validation("adventure is not configured for this event".into()).into(),
        );
    };

    if !config.enabled {
        return Err(AppError::Validation("adventure is not enabled for this event".into()).into());
    }

    // Resolve the claim token: prefer the one already stored in D1 (generated at
    // registration), fall back to the Sheets attendee value. This prevents
    // overwriting a valid D1 claim_token with an empty string when Sheets hasn't
    // synced yet. Adventure progress is keyed by claim token, so this is also the
    // key the completion gate below reads.
    let resolved_claim_token = resolve_claim_token_from_d1(&state, attendee).await;

    // Quest-completion gate. The claim flow re-verifies completion before minting,
    // so skipping this cannot hand out a badge — but `checked_in_at` is also the
    // attendance signal every dashboard, `db::event_summaries` and the plan 008
    // recap count, so an unstarted adventure must not set it. Progress rows are
    // keyed by claim token; no token means no progress can exist.
    let claim_token = resolved_claim_token.clone().unwrap_or_default();
    let quest_status = adventure::get_adventure_status(db, &event.id, &claim_token)
        .await
        .map_err(AppError::Internal)?;
    if !matches!(quest_status, AdventureStatus::Passed) {
        tracing::warn!(
            attendee_id = %attendee.api_id,
            status = ?quest_status,
            "quest-complete virtual check-in denied: adventure not completed",
        );
        return Err(AppError::Validation(
            "complete the required adventure levels before checking in".into(),
        )
        .into());
    }

    // Single writer of the virtual check-in — gates on approval and the event's
    // online track, writes D1 first, then mirrors to Sheets (plan 022 §2).
    let mapping =
        match crate::sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, kv)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(error = %e, "column mapping fallback to hardcoded");
                event_checkin_domain::models::attendee::ColumnMapping::hardcoded()
            }
        };

    crate::virtual_checkin::commit_virtual_check_in(
        &state,
        attendee,
        &event,
        &mapping,
        kv,
        &claim_token,
    )
    .await?;

    tracing::info!(
        email = %claims.email,
        event_id = %event.id,
        attendee_id = %attendee.api_id,
        "virtual check-in completed via adventure quest-complete endpoint"
    );

    Ok(ApiOk::new(json!({
        "status": "checked_in",
        "claim_token": resolved_claim_token,
        "event_slug": event.slug,
    })))
}
