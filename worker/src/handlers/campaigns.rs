//! Campaign management API handlers (Issue 049 Phase 3 — Campaigns & Series).
//!
//! Protected endpoints (require staff auth):
//!   GET    /api/campaigns                    — list campaigns
//!   POST   /api/campaigns                    — create campaign
//!   GET    /api/campaigns/{id}/exists        — is this campaign id taken?
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
    Extension, Json,
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
    /// Initial status. Omitted by older clients, which is why it defaults to
    /// `draft` — the value this endpoint previously hardcoded.
    #[serde(default = "default_campaign_status")]
    pub status: String,
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
pub struct ClaimRewardRequest {
    pub wallet_address: String,
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
pub struct DeveloperEventAttendance {
    pub event_id: String,
    pub event_name: String,
    pub sequence_order: i64,
    pub is_required: bool,
    pub attended: bool,
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
    /// Per-event check-in breakdown. Populated only by the per-campaign
    /// progress endpoint (`GET /api/campaigns/{id}/progress`). Empty for
    /// `my-progress` (the attendee dashboard only needs the summary counts).
    #[serde(default)]
    pub events: Vec<DeveloperEventAttendance>,
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

/// Status a campaign is created with when the client does not choose one.
/// Matches the value `create_campaign` hardcoded before plan 016 P2.3, so an
/// older client that omits the field keeps its previous behaviour exactly.
fn default_campaign_status() -> String {
    "draft".to_string()
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

/// Statuses a campaign may be *created* with.
///
/// Narrower than [`validate_campaign_status`] by design: `completed` is
/// excluded because nothing has been completed at create time, and a campaign
/// born `completed` could never be progressed through. Transitioning to
/// `completed` later remains available via `update_campaign_status`.
fn validate_create_status(status: &str) -> Result<(), AppError> {
    match status {
        "draft" | "active" => Ok(()),
        _ => Err(AppError::Validation(format!(
            "invalid initial campaign status: {status} (expected draft/active)"
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

/// Maximum length of a campaign id (slug). The create form caps its generated
/// slug at 60; the extra headroom allows hand-typed ids without surprises.
const MAX_CAMPAIGN_ID_LEN: usize = 64;

/// Shape-check a campaign id (slug).
///
/// Two jobs. It is the shape the create form's slugify produces, and — because
/// the D1 helpers in `db::campaigns` interpolate rather than bind — it is what
/// keeps a user-supplied slug out of the SQL string. Restricting to
/// `[A-Za-z0-9_-]` makes quoting impossible.
///
/// Enforced on both create and the availability probe so the two never
/// disagree about what counts as a usable id.
fn validate_campaign_id(id: &str) -> Result<(), AppError> {
    let ok = !id.is_empty()
        && id.len() <= MAX_CAMPAIGN_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    match ok {
        true => Ok(()),
        false => Err(AppError::Validation(format!(
            "invalid campaign id: use letters, numbers, '-' or '_' (max {MAX_CAMPAIGN_ID_LEN} characters)"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Campaign CRUD handlers
// ---------------------------------------------------------------------------

/// Load a campaign's organization_id for the per-org access check (404 if absent).
async fn campaign_org(d1: &worker::D1Database, id: &str) -> Result<String, WorkerError> {
    let c = crate::db::campaigns::get_campaign(d1, id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to get campaign: {e}")))?
        .ok_or_else(|| AppError::NotFound(format!("campaign not found: {id}")))?;
    Ok(c.organization_id)
}

/// GET /api/campaigns
#[worker::send]
pub async fn list_campaigns(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    axum::extract::Query(params): axum::extract::Query<ListCampaignsParams>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;

    // S3: an org filter must be one the caller owns; listing ALL is super-admin.
    match params.organization_id.as_deref() {
        Some(org) => {
            crate::auth::require_org_access(&claims.email, org, &state, "list campaigns").await?
        }
        None => {
            crate::auth::require_super_admin(&claims.email, &state, "list all campaigns").await?
        }
    }

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
    Extension(claims): Extension<Claims>,
    axum::Json(body): axum::Json<CreateCampaignRequest>,
) -> Result<ApiOk<CampaignDetail>, WorkerError> {
    let d1 = require_d1(&state)?;

    // S3: only an owner of the target org (or super-admin) may create in it.
    crate::auth::require_org_access(&claims.email, &body.organization_id, &state, "create campaign")
        .await?;

    validate_campaign_id(&body.id)?;
    validate_create_status(&body.status)?;
    validate_reward_type(&body.reward_type)?;

    crate::db::campaigns::create_campaign(
        d1,
        &body.id,
        &body.title,
        &body.description,
        &body.organization_id,
        &body.status,
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

/// GET /api/campaigns/{id}/exists
///
/// Slug-availability probe for the campaign create form, so an organizer is
/// told "already taken" while typing instead of on a failed save.
///
/// Deliberately NOT org-scoped. Campaign ids are the primary key of
/// `campaigns` and therefore globally unique: a slug held by another org is
/// still unavailable, and an org-scoped check would answer "free" for an id
/// that then fails to insert. The response carries a single boolean — no
/// campaign data, not even the owning org — and the route sits behind the
/// authenticated admin router, so this exposes no more than the create attempt
/// it replaces.
#[worker::send]
pub async fn campaign_id_exists(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;

    validate_campaign_id(&id)?;

    let exists = crate::db::campaigns::campaign_exists(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to check campaign id: {e}")))?;

    Ok(ApiOk::new(serde_json::json!({ "exists": exists })))
}

/// GET /api/campaigns/{id}
#[worker::send]
pub async fn get_campaign(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;
    crate::auth::require_org_access(&claims.email, &campaign_org(d1, &id).await?, &state, "view campaign").await?;

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
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    axum::Json(body): axum::Json<UpdateCampaignRequest>,
) -> Result<ApiOk<CampaignDetail>, WorkerError> {
    let d1 = require_d1(&state)?;
    crate::auth::require_org_access(&claims.email, &campaign_org(d1, &id).await?, &state, "update campaign").await?;

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
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;
    crate::auth::require_org_access(&claims.email, &campaign_org(d1, &id).await?, &state, "delete campaign").await?;

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
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    axum::Json(body): axum::Json<UpdateStatusRequest>,
) -> Result<ApiOk<CampaignDetail>, WorkerError> {
    let d1 = require_d1(&state)?;
    crate::auth::require_org_access(&claims.email, &campaign_org(d1, &id).await?, &state, "update campaign status").await?;

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
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;
    crate::auth::require_org_access(&claims.email, &campaign_org(d1, &id).await?, &state, "view campaign events").await?;

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
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    axum::Json(body): axum::Json<SetCampaignEventsRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;
    crate::auth::require_org_access(&claims.email, &campaign_org(d1, &id).await?, &state, "set campaign events").await?;

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
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;
    crate::auth::require_org_access(&claims.email, &campaign_org(d1, &id).await?, &state, "view campaign progress").await?;

    let progress = crate::db::campaigns::list_campaign_progress(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to list campaign progress: {e}")))?;

    // Per-event attendance breakdown, grouped by developer email. The query
    // orders rows by (developer_email, sequence_order) so the per-developer
    // vectors are already in sequence order.
    let attendance = crate::db::campaigns::list_campaign_attendance(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to list campaign attendance: {e}")))?;

    let mut attendance_by_email: std::collections::HashMap<String, Vec<DeveloperEventAttendance>> =
        std::collections::HashMap::new();
    for row in attendance {
        attendance_by_email
            .entry(row.developer_email.clone())
            .or_default()
            .push(DeveloperEventAttendance {
                event_id: row.event_id,
                event_name: row.event_name.unwrap_or_default(),
                sequence_order: row.sequence_order,
                is_required: row.is_required == 1,
                attended: row.attended == 1,
            });
    }

    let items: Vec<DeveloperProgressItem> = progress
        .into_iter()
        .map(|p| {
            let events = attendance_by_email
                .remove(&p.developer_email)
                .unwrap_or_default();
            DeveloperProgressItem {
                campaign_id: p.campaign_id,
                developer_email: p.developer_email,
                events_completed: p.events_completed,
                total_required: p.total_required,
                is_complete: p.is_complete == 1,
                completed_at: p.completed_at,
                reward_claimed_at: p.reward_claimed_at,
                events,
            }
        })
        .collect();

    Ok(ApiOk::new(serde_json::json!({ "progress": items })))
}

/// GET /api/campaigns/{id}/stats
#[worker::send]
pub async fn campaign_stats(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<ApiOk<CampaignStatsResponse>, WorkerError> {
    let d1 = require_d1(&state)?;
    crate::auth::require_org_access(&claims.email, &campaign_org(d1, &id).await?, &state, "view campaign stats").await?;

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
            events: Vec::new(),
        })
        .collect();

    Ok(ApiOk::new(serde_json::json!({ "progress": items })))
}

/// POST /api/campaigns/{id}/claim-reward
/// Mints a campaign completion cNFT and marks the reward as claimed.
#[worker::send]
pub async fn claim_campaign_reward(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Json(body): Json<ClaimRewardRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    let d1 = require_d1(&state)?;

    let email = claims.email.to_lowercase();

    // Validate wallet address format
    if let Err(e) = crate::solana::validate_wallet_address(&body.wallet_address) {
        return Err(AppError::Validation(e).into());
    }

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

    // Fetch campaign for reward_config metadata
    let campaign = crate::db::campaigns::get_campaign(d1, &id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to get campaign: {e}")))?
        .ok_or_else(|| AppError::NotFound(format!("campaign not found: {id}")))?;

    // Resolve the metadata this reward mints with.
    //
    // Shared with the admin create-form preview card (plan 016 P2.2) so the
    // preview cannot drift from what actually mints. It also fixes a real bug
    // in the code this replaced: the admin form serialises untouched fields as
    // `""` rather than omitting them, and `.and_then(as_str).unwrap_or(default)`
    // reads `""` as a deliberate choice — so a campaign saved with a blank NFT
    // name minted an NFT literally named "", while the form's hint promised the
    // title-based default. `resolve_reward_str` treats blank as unset.
    let reward = event_checkin_domain::models::campaign::resolve_reward_str(
        &campaign.title,
        &campaign.reward_config,
    );

    // Mint campaign cNFT via Crossmint (custodial signer + tree + fees). Campaign
    // rewards mint into the same Crossmint collection as event badges.
    let config = &state.config;
    let campaign_external_url = format!("/campaigns/{id}");
    let mint_req = crate::solana::MintRequest {
        wallet_address: &body.wallet_address,
        host: &config.solana.crossmint_host,
        api_key: &config.solana.crossmint_api_key,
        collection_id: &config.solana.crossmint_collection_id,
        image_url: &reward.image_url,
        nft_name: &reward.name,
        nft_description: &reward.description,
        nft_external_url: &campaign_external_url,
        compressed: true,
        idempotency_key: "",
    };

    let mint_result = match crate::solana::mint_compressed_nft(&mint_req, None).await {
        Ok(result) => result,
        Err(e) => {
            tracing::error!(campaign_id = %id, error = %e, "campaign reward mint failed");
            return Err(AppError::External {
                service: "crossmint".into(),
                status: 502,
                body: e,
            }
            .into());
        }
    };

    // Persist mint details alongside claimed timestamp
    crate::db::campaigns::mark_reward_claimed_with_mint(
        d1,
        &id,
        &email,
        &mint_result.asset_id,
        &mint_result.signature,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to save reward mint details: {e}")))?;

    tracing::info!(
        campaign_id = %id,
        asset_id = %mint_result.asset_id,
        signature = %mint_result.signature,
        "campaign reward cNFT minted"
    );

    Ok(ApiOk::new(serde_json::json!({
        "campaign_id": id,
        "reward_claimed": true,
        "asset_id": mint_result.asset_id,
        "signature": mint_result.signature,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_ok(id: &str) -> bool {
        validate_campaign_id(id).is_ok()
    }

    #[test]
    fn accepts_slugs_the_create_form_produces() {
        assert!(is_ok("solana-hacker-series-2025"));
        assert!(is_ok("campaign"));
        assert!(is_ok("2025"));
        assert!(is_ok("my_campaign-2"));
        // Promote-from-event builds `{event_id}-campaign`.
        assert!(is_ok("devday-bkk-campaign"));
        // A doubled dash is a legal slug, not a SQL comment: `--` can only
        // start a comment outside a string literal, and quoting is impossible.
        assert!(is_ok("x--y"));
    }

    #[test]
    fn rejects_empty_id() {
        assert!(!is_ok(""));
    }

    #[test]
    fn enforces_the_length_cap() {
        let at_cap = "a".repeat(MAX_CAMPAIGN_ID_LEN);
        let over_cap = "a".repeat(MAX_CAMPAIGN_ID_LEN + 1);
        assert!(is_ok(&at_cap));
        assert!(!is_ok(&over_cap));
    }

    /// The validator is the only thing standing between a user-supplied slug
    /// and an interpolated D1 query, so quoting must be impossible.
    #[test]
    fn rejects_sql_metacharacters() {
        assert!(!is_ok("x' OR '1'='1"));
        assert!(!is_ok("x'; DROP TABLE campaigns;--"));
        assert!(!is_ok("x\"y"));
        assert!(!is_ok("x`y"));
        assert!(!is_ok("x;y"));
        assert!(!is_ok("x y"));
        assert!(!is_ok("x\ny"));
    }

    #[test]
    fn create_accepts_only_draft_and_active() {
        assert!(validate_create_status("draft").is_ok());
        assert!(validate_create_status("active").is_ok());
        // Nothing has been completed at create time, and a campaign born
        // `completed` could never be progressed through.
        assert!(validate_create_status("completed").is_err());
        assert!(validate_create_status("").is_err());
        assert!(validate_create_status("Draft").is_err());
        assert!(validate_create_status("archived").is_err());
    }

    /// `completed` stays reachable through the status-transition endpoint —
    /// create is narrower than the general validator on purpose.
    #[test]
    fn transition_validator_still_accepts_completed() {
        assert!(validate_campaign_status("completed").is_ok());
        assert!(validate_campaign_status("draft").is_ok());
        assert!(validate_campaign_status("active").is_ok());
        assert!(validate_campaign_status("nonsense").is_err());
    }

    /// An older client that omits `status` must behave exactly as before
    /// plan 016 P2.3, when the handler hardcoded `'draft'`.
    #[test]
    fn omitted_status_defaults_to_draft() {
        let body: CreateCampaignRequest = serde_json::from_str(
            r#"{"id":"c1","title":"T","organization_id":"org"}"#,
        )
        .expect("request without status should deserialize");
        assert_eq!(body.status, "draft");
        assert!(validate_create_status(&body.status).is_ok());
    }

    #[test]
    fn explicit_status_is_carried_through() {
        let body: CreateCampaignRequest = serde_json::from_str(
            r#"{"id":"c1","title":"T","organization_id":"org","status":"active"}"#,
        )
        .expect("request with status should deserialize");
        assert_eq!(body.status, "active");
    }

    #[test]
    fn rejects_path_and_non_ascii_characters() {
        assert!(!is_ok("a/b"));
        assert!(!is_ok("a%2Fb"));
        assert!(!is_ok("café"));
        assert!(!is_ok("キャンペーン"));
    }
}
