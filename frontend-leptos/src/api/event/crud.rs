//! Event management API functions (admin).

use serde::{Deserialize, Serialize};

use crate::api::types::{ApiError, ApiResponse};
use crate::api::{
    api_delete, api_get, api_post, api_post_blob, api_post_json, api_put_json, fetch::response_json,
};

use super::enums::EscrowStatus;
use super::types::*;

// ===== Event Management API functions (admin) =====

/// GET /api/events — fetch one bounded page.
pub async fn list_events_page(cursor: Option<&str>) -> Result<EventsListData, ApiError> {
    let path = cursor.map_or_else(
        || "/events".to_string(),
        |cursor| format!("/events?cursor={}", urlencoding::encode(cursor)),
    );
    let response = api_get(&path).await?;
    let result: ApiResponse<EventsListData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse events response: {e}"),
            status: 0,
        })?;

    if !result.success {
        return Err(ApiError {
            message: result.error.unwrap_or("Unknown error".to_string()),
            status: 0,
        });
    }

    result.data.ok_or_else(|| ApiError {
        message: "No data in response".to_string(),
        status: 0,
    })
}

/// Fetch every event page for selectors that require the complete set.
pub async fn list_events() -> Result<EventsListData, ApiError> {
    let mut all = Vec::new();
    let mut cursor = None;
    loop {
        let page = list_events_page(cursor.as_deref()).await?;
        all.extend(page.events);
        match page.next_cursor {
            Some(next) if cursor.as_deref() != Some(next.as_str()) => cursor = Some(next),
            Some(_) => {
                return Err(ApiError {
                    message: "Events API returned a repeated cursor".into(),
                    status: 0,
                });
            }
            None => break,
        }
    }
    Ok(EventsListData {
        events: all,
        next_cursor: None,
    })
}

/// GET /api/events/{id} — get full event config.
pub async fn get_event_detail(id: &str) -> Result<EventDetailData, ApiError> {
    let path = format!("/events/{id}");
    let response = api_get(&path).await?;
    let result: ApiResponse<EventDetailData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse event detail response: {e}"),
            status: 0,
        })?;

    if !result.success {
        return Err(ApiError {
            message: result.error.unwrap_or("Unknown error".to_string()),
            status: 0,
        });
    }

    result.data.ok_or_else(|| ApiError {
        message: "No data in response".to_string(),
        status: 0,
    })
}

/// POST /api/events — create a new event.
pub async fn create_event(body: &CreateEventBody) -> Result<EventMutationData, ApiError> {
    api_post_json("/events", body).await
}

/// PUT /api/events/{id} — update an event.
pub async fn update_event(id: &str, body: &UpdateEventBody) -> Result<EventMutationData, ApiError> {
    let path = format!("/events/{id}");
    api_put_json(&path, body).await
}

/// Response shape for poster upload/delete. `poster_url` is the served path
/// (`/api/storage/posters/{event_id}`) — always use this URL as `form.poster_url`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PosterMutationData {
    pub id: String,
    pub poster_url: String,
    #[serde(default)]
    pub updated_at: String,
}

/// POST /api/events/{id}/poster — upload marketing poster (raw image bytes).
/// Caller passes the file's `Blob` and its content-type (e.g. `image/png`).
/// Returns the served URL to store into `form.poster_url`.
pub async fn upload_poster(
    event_id: &str,
    blob: &web_sys::Blob,
    content_type: &str,
) -> Result<PosterMutationData, ApiError> {
    let path = format!("/events/{event_id}/poster");
    let response = api_post_blob(&path, blob, content_type).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Upload failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body
                .error
                .unwrap_or_else(|| format!("HTTP {}", response.status())),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<PosterMutationData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse poster response: {e}"),
            status: response.status(),
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper
            .error
            .unwrap_or_else(|| "No data in response".to_string()),
        status: response.status(),
    })
}

/// DELETE /api/events/{id}/poster — clear `poster_url` and delete the R2 object.
pub async fn delete_poster(event_id: &str) -> Result<PosterMutationData, ApiError> {
    let path = format!("/events/{event_id}/poster");
    let response = api_delete(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Delete failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body
                .error
                .unwrap_or_else(|| format!("HTTP {}", response.status())),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<PosterMutationData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse poster response: {e}"),
            status: response.status(),
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper
            .error
            .unwrap_or_else(|| "No data in response".to_string()),
        status: response.status(),
    })
}

/// POST /api/escrow/init — combined ATA + create_event in one transaction.
pub async fn init_escrow(body: &InitEscrowRequest) -> Result<InitEscrowResponse, ApiError> {
    api_post_json("/escrow/init", body).await
}

/// Request body for POST /api/escrow/confirm-init.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfirmEscrowInitRequest {
    pub event_id: String,
}

/// Response from POST /api/escrow/confirm-init.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfirmEscrowInitResponse {
    pub escrow_address: String,
    pub on_chain_event_id: u64,
    pub escrow_status: EscrowStatus,
}

/// POST /api/escrow/confirm-init — verify escrow exists on-chain & persist state.
///
/// Recovery endpoint: syncs on-chain escrow state to the server-side event config.
/// Idempotent — safe to call multiple times.
pub async fn confirm_escrow_init(
    body: &ConfirmEscrowInitRequest,
) -> Result<ConfirmEscrowInitResponse, ApiError> {
    api_post_json("/escrow/confirm-init", body).await
}

/// DELETE /api/events/{id} — archive an event.
pub async fn archive_event(id: &str) -> Result<EventMutationData, ApiError> {
    let path = format!("/events/{id}");
    let response = api_delete(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Archive failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<EventMutationData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse archive response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// DELETE /api/events/{id}/delete — permanently delete an archived event.
pub async fn hard_delete_event(id: &str, force: bool) -> Result<EventMutationData, ApiError> {
    let path = if force {
        format!("/events/{id}/delete?force=true")
    } else {
        format!("/events/{id}/delete")
    };
    let response = api_delete(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Delete failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<EventMutationData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse delete response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/events/{id}/restore — restore an archived event back to Draft.
pub async fn restore_event(id: &str) -> Result<EventMutationData, ApiError> {
    let path = format!("/events/{id}/restore");
    let response = api_post(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Restore failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<EventMutationData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse restore response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// Response payload for POST /api/events/{id}/duplicate.
///
/// Mirrors `EventMutationData` plus the source event id and a list of
/// non-fatal warnings (e.g. sheet_id collision risk under Decision A1)
/// that the UI should surface as a yellow toast.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct DuplicateEventData {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub status: String,
    /// The id of the event this Draft was copied from.
    #[serde(default)]
    pub source_id: String,
    /// Non-fatal warnings (e.g. shared sheet_id). Empty when no warnings.
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub updated_at: String,
}

/// POST /api/events/{id}/duplicate — copy an event's settings into a new Draft.
///
/// On success, callers should render `data.warnings` (if any) as a yellow
/// toast in addition to the success toast, per Decision A1 in
/// `.issues/055_duplicate_event.md`.
pub async fn duplicate_event(id: &str) -> Result<DuplicateEventData, ApiError> {
    let path = format!("/events/{id}/duplicate");
    let response = api_post(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Duplicate failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<DuplicateEventData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse duplicate response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}
