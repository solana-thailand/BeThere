//! Campaign CRUD and progress tracking API (Issue 049 Phase 3).

use serde::{Deserialize, Serialize};

use super::types::ApiError;
use super::{api_delete, api_get, api_post, api_post_json, api_put_json, fetch::response_json};

// ===== Campaign Types =====

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CampaignStatus {
    #[default]
    Draft,
    Active,
    Completed,
}

impl CampaignStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Completed => "completed",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Active => "Active",
            Self::Completed => "Completed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CampaignDetail {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub organization_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub completion_criteria: String,
    #[serde(default)]
    pub reward_type: String,
    #[serde(default)]
    pub reward_config: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CampaignEventItem {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub sequence_order: i64,
    #[serde(default)]
    pub is_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeveloperProgressItem {
    #[serde(default)]
    pub campaign_id: String,
    #[serde(default)]
    pub developer_email: String,
    #[serde(default)]
    pub events_completed: i64,
    #[serde(default)]
    pub total_required: i64,
    #[serde(default)]
    pub is_complete: bool,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub reward_claimed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventDropOffItem {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub sequence_order: i64,
    #[serde(default)]
    pub attended: i64,
    #[serde(default)]
    pub total_in_campaign: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CampaignStatsResponse {
    #[serde(default)]
    pub total_enrolled: i64,
    #[serde(default)]
    pub total_completed: i64,
    #[serde(default)]
    pub completion_rate: f64,
    #[serde(default)]
    pub events: Vec<EventDropOffItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CampaignListResponse {
    #[serde(default)]
    pub campaigns: Vec<CampaignDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CampaignDetailResponse {
    #[serde(default)]
    pub campaign: CampaignDetail,
    #[serde(default)]
    pub events: Vec<CampaignEventItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CampaignEventsResponse {
    #[serde(default)]
    pub events: Vec<CampaignEventItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CampaignProgressResponse {
    #[serde(default)]
    pub progress: Vec<DeveloperProgressItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCampaignRequest {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub organization_id: String,
    #[serde(default)]
    pub completion_criteria: String,
    #[serde(default)]
    pub reward_type: String,
    #[serde(default)]
    pub reward_config: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCampaignRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub completion_criteria: String,
    #[serde(default)]
    pub reward_type: String,
    #[serde(default)]
    pub reward_config: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatusRequest {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCampaignEventsRequest {
    pub events: Vec<CampaignEventInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignEventInput {
    pub event_id: String,
    #[serde(default)]
    pub sequence_order: i64,
    #[serde(default = "super::types::default_true")]
    pub is_required: bool,
}

// ===== API Functions =====

/// GET /api/campaigns — list all campaigns
pub async fn list_campaigns(
    organization_id: Option<&str>,
    status: Option<&str>,
) -> Result<Vec<CampaignDetail>, ApiError> {
    let mut path = "/campaigns?".to_string();
    if let Some(org) = organization_id {
        path.push_str(&format!("organization_id={org}&"));
    }
    if let Some(s) = status {
        path.push_str(&format!("status={s}&"));
    }

    let response = api_get(&path).await?;
    if !response.ok() {
        return Err(ApiError {
            message: "Failed to list campaigns".to_string(),
            status: response.status(),
        });
    }

    let result: super::types::ApiResponse<CampaignListResponse> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse campaigns: {e}"),
            status: 0,
        })?;

    Ok(result
        .data
        .map(|d| d.campaigns)
        .unwrap_or_default())
}

/// GET /api/campaigns/{id} — get campaign detail with events
pub async fn get_campaign(id: &str) -> Result<CampaignDetailResponse, ApiError> {
    let path = format!("/campaigns/{id}");
    let response = api_get(&path).await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Campaign not found".to_string(),
            status: response.status(),
        });
    }

    response_json(&response).await.map_err(|e| ApiError {
        message: format!("Failed to parse campaign: {e}"),
        status: 0,
    })
    .and_then(|r: super::types::ApiResponse<CampaignDetailResponse>| {
        r.data.ok_or_else(|| ApiError {
            message: "No data in response".to_string(),
            status: 0,
        })
    })
}

/// POST /api/campaigns — create campaign
pub async fn create_campaign(
    req: &CreateCampaignRequest,
) -> Result<CampaignDetail, ApiError> {
    api_post_json("/campaigns", req).await
}

/// PUT /api/campaigns/{id} — update campaign
pub async fn update_campaign(
    id: &str,
    req: &UpdateCampaignRequest,
) -> Result<CampaignDetail, ApiError> {
    let path = format!("/campaigns/{id}");
    api_put_json(&path, req).await
}

/// DELETE /api/campaigns/{id} — delete campaign
pub async fn delete_campaign(id: &str) -> Result<(), ApiError> {
    let path = format!("/campaigns/{id}");
    let response = api_delete(&path).await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Failed to delete campaign".to_string(),
            status: response.status(),
        });
    }

    Ok(())
}

/// PATCH /api/campaigns/{id}/status — update campaign status
pub async fn update_campaign_status(
    id: &str,
    status: &str,
) -> Result<CampaignDetail, ApiError> {
    let path = format!("/campaigns/{id}/status");
    let req = UpdateStatusRequest {
        status: status.to_string(),
    };
    api_post_json(&path, &req).await
}

/// PUT /api/campaigns/{id}/events — set campaign events (full replace)
pub async fn set_campaign_events(
    id: &str,
    events: Vec<CampaignEventInput>,
) -> Result<(), ApiError> {
    let path = format!("/campaigns/{id}/events");
    let req = SetCampaignEventsRequest { events };
    // Backend returns a JSON value; we discard it.
    let _ = api_put_json::<serde_json::Value>(&path, &req).await?;
    Ok(())
}

/// GET /api/campaigns/{id}/progress — list developer progress
pub async fn list_campaign_progress(
    id: &str,
) -> Result<Vec<DeveloperProgressItem>, ApiError> {
    let path = format!("/campaigns/{id}/progress");
    let response = api_get(&path).await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Failed to list progress".to_string(),
            status: response.status(),
        });
    }

    let result: super::types::ApiResponse<CampaignProgressResponse> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse progress: {e}"),
            status: 0,
        })?;

    Ok(result.data.map(|d| d.progress).unwrap_or_default())
}

/// GET /api/campaigns/{id}/stats — campaign completion stats
pub async fn get_campaign_stats(id: &str) -> Result<CampaignStatsResponse, ApiError> {
    let path = format!("/campaigns/{id}/stats");
    let response = api_get(&path).await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Failed to get stats".to_string(),
            status: response.status(),
        });
    }

    response_json(&response).await.map_err(|e| ApiError {
        message: format!("Failed to parse stats: {e}"),
        status: 0,
    })
    .and_then(|r: super::types::ApiResponse<CampaignStatsResponse>| {
        r.data.ok_or_else(|| ApiError {
            message: "No data in response".to_string(),
            status: 0,
        })
    })
}

/// GET /api/campaigns/my-progress — current user's campaign progress
pub async fn my_campaign_progress() -> Result<Vec<DeveloperProgressItem>, ApiError> {
    let response = api_get("/campaigns/my-progress").await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Failed to get my progress".to_string(),
            status: response.status(),
        });
    }

    let result: super::types::ApiResponse<CampaignProgressResponse> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse my progress: {e}"),
            status: 0,
        })?;

    Ok(result.data.map(|d| d.progress).unwrap_or_default())
}

/// POST /api/campaigns/{id}/claim-reward — claim completion reward
pub async fn claim_campaign_reward(id: &str) -> Result<(), ApiError> {
    let path = format!("/campaigns/{id}/claim-reward");
    let response = api_post(&path).await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Failed to claim reward".to_string(),
            status: response.status(),
        });
    }

    Ok(())
}
