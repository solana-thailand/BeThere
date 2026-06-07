//! Campaign management API handlers (Issue 049 Phase 3 — Campaigns & Series).
//!
//! Protected endpoints (require staff auth):
//!   GET    /api/campaigns                    — list campaigns
//!   POST   /api/campaigns                    — create campaign
//!   GET    /api/campaigns/{id}               — get campaign detail
//!   PUT    /api/campaigns/{id}               — update campaign
//!   DELETE /api/campaigns/{id}               — delete campaign
//!   PATCH  /api/campaigns/{id}/status        — update campaign status
//!   GET    /api/campaigns/{id}/events        — list campaign events
//!   PUT    /api/campaigns/{id}/events        — set (replace) campaign events
//!   GET    /api/campaigns/{id}/progress      — list developer progress
//!   GET    /api/campaigns/{id}/stats         — campaign completion stats
//!
//! Attendee-authenticated endpoints:
//!   GET    /api/campaigns/my-progress        — current user's campaign progress
//!   POST   /api/campaigns/{id}/claim-reward  — claim completion certificate NFT

use axum::{
    Extension,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};

use crate::error::{ApiOk, WorkerError};
use crate::state::AppState;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateCampaignRequest {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub organization_id: String,
    #[serde(default = "default_completion_criteria")]
    pub completion_criteria: String,
    #[serde(default)]
    pub reward_type: String,
    #[serde(default = "default_reward_config")]
    pub reward_config: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCampaignRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_completion_criteria")]
    pub completion_criteria: String,
    #[serde(default)]
    pub reward_type: String,
    #[serde(default = "default_reward_config")]
    pub reward_config: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStatusRequest {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct SetCampaignEventsRequest {
    pub events: Vec<CampaignEventInput>,
}

#[derive(Debug, Deserialize)]
pub struct CampaignEventInput {
    pub event_id: String,
    #[serde(default)]
    pub sequence_order: i64,
    #[serde(default = "default_true")]
    pub is_required: bool,
}

#[derive(Debug, Deserialize)]
pub struct ListCampaignsParams {
    pub organization_id: Option<String>,
    pub status: Option<String>,
}

#[derive(Serialize)]
pub struct CampaignDetail {
    pub id: String,
    pub title: String,
    pub description: String,
    pub organization_id: String,
    pub status: String,
    pub completion_criteria: String,
    pub reward_type: String,
    pub reward_config: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct CampaignEventItem {
    pub event_id: String,
    pub sequence_order: i64,
    pub is_required: bool,
}

#[derive(Serialize)]
pub struct DeveloperProgressItem {
    pub campaign_id: String,
    pub developer_email: String,
    pub events_completed: i64,
    pub total_required: i64,
    pub is_complete: bool,
    pub completed_at: Option<String>,
    pub reward_claimed_at: Option<String>,
}

#[derive(Serialize)]
pub struct CampaignStatsResponse {
    pub total_enrolled: i64,
    pub total_completed: i64,
    pub completion_rate: f64,
    pub events: Vec<EventDropOffItem>,
}

#[derive(Serialize)]
pub struct EventDropOffItem {
    pub event_id: String,
    pub sequence_order: i64,
    pub attended: i64,
    pub total_in_campaign: i64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn default_completion_criteria() -> String {
    "{}".to_string()
}

fn default_reward_config() -> String {
    "{}".to_string()
}

fn default_true() -> bool {
    true
}

fn require_d1(state: &AppState) -> Result<&worker::D1Database, AppError> {
    state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::NotFound("D1 database not available".to_string()))
}

fn row_to_detail(row: crate::db::campaigns::CampaignRow) -> CampaignDetail {
    CampaignDetail {
        id: row.id,
        title: row.title,
        description: row.description,
        organization_id: row.organization_id,
        status: row.status,
        completion_criteria: row.completion_criteria,
        reward_type: row.reward_type,
        reward_config: row.reward_config,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn validate_campaign_status(status: &str) -> Result<(), AppError> {
    match status {
        "draft" | "active" | "completed" => Ok(()),
        _ => Err(AppError::Validation(format!(
            "invalid campaign status: {status} (expected draft/active/completed)"
        ))),
    }
}

fn validate_reward_type(reward_type: &str) -> Result<(), AppError> {
    match reward_type {
        "none" | "nft_certificate" | "badge" => Ok(()),
        _ => Err(AppError::Validation(format!(
            "invalid reward_type: {reward_type} (expected none/nft_certificate/badge)"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Campaign CRUD handlers
// ---------------------------------------------------------------------------

/// GET /api/campaigns
#[worker::send]
pub async fn list_campaigns(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    axum::extract::Query(params): axum::extract::Query<ListCampaignsParams>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;

    let campaigns = crate::db::campaigns::list_campaigns(
        d1,
        params.organization_id.as_deref(),
        params.status.as_deref(),
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to list campaigns: {e}")))?;

    let items: Vec<CampaignDetail> = campaigns.into_iter().map(row_to_detail).collect();

    Ok(ApiOk::new(serde_json::json!({ "campaigns": items })))
}

/// POST /api/campaigns
#[worker::send]
pub async fn create_campaign(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    axum::Json(body): axum::Json<CreateCampaignRequest>,
) -> Result<ApiOk<CampaignDetail>, WorkerError> {
    let d1 = require_d1(&state)?;

    validate_reward_type(&body.reward_type)?;

    crate::db::campaigns::create_campaign(
        d1,
        &body.id,
        &body.title,
        &body.description,
        &body.organization_id,
        &body.completion_criteria,
        &body.reward_type,
        &body.reward_config,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to create campaign: {e}")))?;

    let campaign = crate::db::campaigns::get_campaign(d1, &body.id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to read campaign: {e}")))?
        .ok_or_else(|| AppError::Internal("campaign created but not found".to_string()))?;

    Ok(ApiOk::new(row_to_detail(campaign)))
}

/// GET /api/campaigns/{id}
#[worker::send]
pub async fn get_campaign(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;

    let campaign = crate::db::campaigns::get_campaign(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to get campaign: {e}")))?
        .ok_or_else(|| AppError::NotFound(format!("campaign not found: {id}")))?;

    let events = crate::db::campaigns::list_campaign_events(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to list campaign events: {e}")))?;

    Ok(ApiOk::new(serde_json::json!({
        "campaign": row_to_detail(campaign),
        "events": events.into_iter().map(|e| CampaignEventItem {
            event_id: e.event_id,
            sequence_order: e.sequence_order,
            is_required: e.is_required == 1,
        }).collect::<Vec<_>>(),
    })))
}

/// PUT /api/campaigns/{id}
#[worker::send]
pub async fn update_campaign(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<String>,
    axum::Json(body): axum::Json<UpdateCampaignRequest>,
) -> Result<ApiOk<CampaignDetail>, WorkerError> {
    let d1 = require_d1(&state)?;

    validate_reward_type(&body.reward_type)?;

    // Verify campaign exists
    let _ = crate::db::campaigns::get_campaign(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to get campaign: {e}")))?
        .ok_or_else(|| AppError::NotFound(format!("campaign not found: {id}")))?;

    crate::db::campaigns::update_campaign(
        d1,
        &id,
        &body.title,
        &body.description,
        &body.completion_criteria,
        &body.reward_type,
        &body.reward_config,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to update campaign: {e}")))?;

    let campaign = crate::db::campaigns::get_campaign(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to read campaign: {e}")))?
        .ok_or_else(|| AppError::Internal("campaign updated but not found".to_string()))?;

    Ok(ApiOk::new(row_to_detail(campaign)))
}

/// DELETE /api/campaigns/{id}
#[worker::send]
pub async fn delete_campaign(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;

    // Cascade delete: events + progress first, then campaign
    crate::db::campaigns::set_campaign_events(d1, &id, &[])
        .await
        .map_err(|e| AppError::Internal(format!("failed to clear campaign events: {e}")))?;

    // Delete progress for this campaign
    let delete_progress_sql =
        format!("DELETE FROM developer_campaign_progress WHERE campaign_id = '{id}'");
    d1.exec(&delete_progress_sql)
        .await
        .map_err(|e| format!("D1 delete campaign progress: {e:?}"))
        .map_err(AppError::Internal)?;

    crate::db::campaigns::delete_campaign(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to delete campaign: {e}")))?;

    Ok(ApiOk::new(serde_json::json!({ "deleted": id })))
}

/// PATCH /api/campaigns/{id}/status
#[worker::send]
pub async fn update_campaign_status(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<String>,
    axum::Json(body): axum::Json<UpdateStatusRequest>,
) -> Result<ApiOk<CampaignDetail>, WorkerError> {
    let d1 = require_d1(&state)?;

    validate_campaign_status(&body.status)?;

    // Verify campaign exists
    let _ = crate::db::campaigns::get_campaign(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to get campaign: {e}")))?
        .ok_or_else(|| AppError::NotFound(format!("campaign not found: {id}")))?;

    crate::db::campaigns::update_campaign_status(d1, &id, &body.status)
        .await
        .map_err(|e| AppError::Internal(format!("failed to update campaign status: {e}")))?;

    let campaign = crate::db::campaigns::get_campaign(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to read campaign: {e}")))?
        .ok_or_else(|| AppError::Internal("campaign not found after status update".to_string()))?;

    Ok(ApiOk::new(row_to_detail(campaign)))
}

// ---------------------------------------------------------------------------
// Campaign Events handlers
// ---------------------------------------------------------------------------

/// GET /api/campaigns/{id}/events
#[worker::send]
pub async fn list_campaign_events(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;

    let events = crate::db::campaigns::list_campaign_events(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to list campaign events: {e}")))?;

    let items: Vec<CampaignEventItem> = events
        .into_iter()
        .map(|e| CampaignEventItem {
            event_id: e.event_id,
            sequence_order: e.sequence_order,
            is_required: e.is_required == 1,
        })
        .collect();

    Ok(ApiOk::new(serde_json::json!({ "events": items })))
}

/// PUT /api/campaigns/{id}/events
/// Full replace: sets the ordered list of events for a campaign.
#[worker::send]
pub async fn set_campaign_events(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<String>,
    axum::Json(body): axum::Json<SetCampaignEventsRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;

    // Verify campaign exists
    let _ = crate::db::campaigns::get_campaign(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to get campaign: {e}")))?
        .ok_or_else(|| AppError::NotFound(format!("campaign not found: {id}")))?;

    let tuples: Vec<(String, i64, i64)> = body
        .events
        .iter()
        .map(|e| {
            (
                e.event_id.clone(),
                e.sequence_order,
                if e.is_required { 1 } else { 0 },
            )
        })
        .collect();

    crate::db::campaigns::set_campaign_events(d1, &id, &tuples)
        .await
        .map_err(|e| AppError::Internal(format!("failed to set campaign events: {e}")))?;

    let total_required = tuples.iter().filter(|(_, _, req)| *req == 1).count() as i64;

    Ok(ApiOk::new(serde_json::json!({
        "campaign_id": id,
        "event_count": tuples.len(),
        "required_count": total_required,
    })))
}

// ---------------------------------------------------------------------------
// Campaign Progress handlers
// ---------------------------------------------------------------------------

/// GET /api/campaigns/{id}/progress
#[worker::send]
pub async fn list_campaign_progress(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;

    let progress = crate::db::campaigns::list_campaign_progress(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to list campaign progress: {e}")))?;

    let items: Vec<DeveloperProgressItem> = progress
        .into_iter()
        .map(|p| DeveloperProgressItem {
            campaign_id: p.campaign_id,
            developer_email: p.developer_email,
            events_completed: p.events_completed,
            total_required: p.total_required,
            is_complete: p.is_complete == 1,
            completed_at: p.completed_at,
            reward_claimed_at: p.reward_claimed_at,
        })
        .collect();

    Ok(ApiOk::new(serde_json::json!({ "progress": items })))
}

/// GET /api/campaigns/{id}/stats
#[worker::send]
pub async fn campaign_stats(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<CampaignStatsResponse>, WorkerError> {
    let d1 = require_d1(&state)?;

    let stats = crate::db::campaigns::campaign_completion_stats(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to get campaign stats: {e}")))?;

    Ok(ApiOk::new(CampaignStatsResponse {
        total_enrolled: stats.total_enrolled,
        total_completed: stats.total_completed,
        completion_rate: stats.completion_rate,
        events: stats
            .events
            .into_iter()
            .map(|e| EventDropOffItem {
                event_id: e.event_id,
                sequence_order: e.sequence_order,
                attended: e.attended,
                total_in_campaign: e.total_in_campaign,
            })
            .collect(),
    }))
}

// ---------------------------------------------------------------------------
// Attendee-facing endpoints
// ---------------------------------------------------------------------------

/// GET /api/campaigns/my-progress
/// Returns the current user's progress across all campaigns they're enrolled in.
#[worker::send]
pub async fn my_campaign_progress(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;

    let email = claims.email.to_lowercase();
    let progress = crate::db::campaigns::list_developer_campaigns(d1, &email)
        .await
        .map_err(|e| AppError::Internal(format!("failed to list developer campaigns: {e}")))?;

    let items: Vec<DeveloperProgressItem> = progress
        .into_iter()
        .map(|p| DeveloperProgressItem {
            campaign_id: p.campaign_id,
            developer_email: p.developer_email,
            events_completed: p.events_completed,
            total_required: p.total_required,
            is_complete: p.is_complete == 1,
            completed_at: p.completed_at,
            reward_claimed_at: p.reward_claimed_at,
        })
        .collect();

    Ok(ApiOk::new(serde_json::json!({ "progress": items })))
}

/// POST /api/campaigns/{id}/claim-reward
/// Marks the completion certificate NFT reward as claimed for the current user.
#[worker::send]
pub async fn claim_campaign_reward(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;

    let email = claims.email.to_lowercase();

    let progress = crate::db::campaigns::get_developer_progress(d1, &id, &email)
        .await
        .map_err(|e| AppError::Internal(format!("failed to get developer progress: {e}")))?
        .ok_or_else(|| AppError::NotFound("not enrolled in this campaign".to_string()))?;

    if progress.is_complete != 1 {
        return Err(AppError::Validation(
            "campaign not yet completed — complete all required events first".to_string(),
        )
        .into());
    }

    if progress.reward_claimed_at.is_some() {
        return Err(
            AppError::Validation("reward already claimed for this campaign".to_string()).into(),
        );
    }

    crate::db::campaigns::mark_reward_claimed(d1, &id, &email)
        .await
        .map_err(|e| AppError::Internal(format!("failed to mark reward claimed: {e}")))?;

    Ok(ApiOk::new(serde_json::json!({
        "campaign_id": id,
        "reward_claimed": true,
    })))
}
