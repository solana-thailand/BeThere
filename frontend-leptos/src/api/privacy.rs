//! PDPA Privacy API — data deletion and marketing unsubscribe.

use serde::Deserialize;

use super::types::{ApiError, ApiResponse};
use super::fetch::response_json;

/// Response from POST /api/privacy/delete-request
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DeleteRequestResponse {
    pub status: String,
    pub email: String,
    #[serde(default)]
    pub events_affected: usize,
    #[serde(default)]
    pub events_blocked: usize,
    #[serde(default)]
    pub blocked_events: Vec<BlockedEvent>,
    #[serde(default)]
    pub d1_attendees_cleared: usize,
    #[serde(default)]
    pub kv_keys_deleted: usize,
    #[serde(default)]
    pub r2_objects_deleted: usize,
    #[serde(default)]
    pub on_chain_note: Option<String>,
    #[serde(default)]
    pub time_gate_note: Option<String>,
}

/// An event that blocks deletion.
#[derive(Debug, Clone, Deserialize)]
pub struct BlockedEvent {
    pub event_id: String,
    pub event_name: String,
    pub event_end_ms: i64,
}

/// Response from POST /api/privacy/unsubscribe-marketing
#[derive(Debug, Clone, Deserialize, Default)]
pub struct UnsubscribeMarketingResponse {
    pub status: String,
    pub email: String,
    #[serde(default)]
    pub rows_updated: usize,
}

/// POST /api/privacy/delete-request
/// Self-service PDPA data deletion. Requires attendee JWT.
pub async fn request_data_deletion(
    event_id: Option<&str>,
) -> Result<DeleteRequestResponse, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/privacy/delete-request?event_id={eid}"),
        _ => "/privacy/delete-request".to_string(),
    };
    let response = super::api_post(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Deletion request failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<DeleteRequestResponse> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse deletion response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/privacy/unsubscribe-marketing
/// Self-service marketing opt-out. Requires attendee JWT.
pub async fn unsubscribe_marketing() -> Result<UnsubscribeMarketingResponse, ApiError> {
    let response = super::api_post("/privacy/unsubscribe-marketing").await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Unsubscribe failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<UnsubscribeMarketingResponse> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse unsubscribe response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}
