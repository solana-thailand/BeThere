//! Attendee check-in, walk-in registration, QR generation, and cache flush.

use serde::{Deserialize, Serialize};

use super::types::{ApiError, ApiResponse};
use super::{api_get, api_post, cache_invalidate, cached_get};

// ===== Walk-in Registration Types =====

/// Request body for POST /api/walkin/register
#[derive(Debug, Clone, Serialize)]
pub struct WalkinRegisterRequest {
    pub event_id: String,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
}

/// Response from POST /api/walkin/register
#[derive(Debug, Clone, Deserialize, Default)]
pub struct WalkinRegisterResponse {
    pub claim_token: String,
    pub claim_url: String,
}

// ===== Walk-in CSV Export & Sync Types =====

/// Walk-in attendee info from GET /api/walkin/list
#[derive(Debug, Clone, Deserialize)]
pub struct WalkinAttendeeInfo {
    pub event_id: String,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub claim_token: String,
    pub checked_in_at: String,
    pub checked_in_by: String,
    pub wallet_address: Option<String>,
    pub claimed_at: Option<String>,
}

/// Response from GET /api/walkin/list
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WalkinListResponse {
    pub attendees: Vec<WalkinAttendeeInfo>,
    pub count: usize,
}

/// Response from GET /api/walkin/export
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WalkinExportResponse {
    pub csv: String,
    pub filename: String,
    pub count: usize,
}

/// Request body for POST /api/walkin/sync
#[derive(Debug, Clone, Serialize)]
pub struct WalkinSyncRequest {
    pub event_id: String,
}

/// Response from POST /api/walkin/sync
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WalkinSyncResponse {
    pub synced: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
    pub total_walkins: usize,
}

// ===== Attendee API functions =====

use super::types::{AttendeeData, AttendeesData, CheckInData, GenerateQrData};

/// GET /api/attendees
/// Returns attendees with cursor-based pagination and stats.
///
/// Results are cached client-side for 30 seconds (B5).
/// Call `invalidate_attendee_cache()` after mutations (check-in, QR gen).
pub async fn get_attendees(
    event_id: Option<&str>,
    cursor: Option<usize>,
    limit: Option<usize>,
) -> Result<AttendeesData, ApiError> {
    let mut path = "/attendees".to_string();
    let mut params = Vec::new();

    if let Some(id) = event_id {
        if !id.is_empty() {
            params.push(format!("event_id={id}"));
        }
    }
    if let Some(c) = cursor {
        params.push(format!("cursor={c}"));
    }
    if let Some(l) = limit {
        params.push(format!("limit={l}"));
    }

    if !params.is_empty() {
        path = format!("{path}?{}", params.join("&"));
    }

    let json = cached_get(&path).await?;

    let wrapper: ApiResponse<AttendeesData> =
        serde_json::from_str(&json).map_err(|e| ApiError {
            message: format!("Failed to parse attendees: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// Invalidate the client-side attendee cache.
/// Call this after any mutation that changes attendee data
/// (check-in, QR generation, bulk operations).
pub fn invalidate_attendee_cache() {
    cache_invalidate("/attendees");
    cache_invalidate("/attendee/");
}

/// GET /api/attendee/:id
/// Returns a single attendee by their api_id.
///
/// Results are cached client-side for 30 seconds (B5).
pub async fn get_attendee(id: &str, event_id: Option<&str>) -> Result<AttendeeData, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/attendee/{id}?event_id={eid}"),
        _ => format!("/attendee/{id}"),
    };

    let json = cached_get(&path).await?;

    let wrapper: ApiResponse<AttendeeData> =
        serde_json::from_str(&json).map_err(|e| ApiError {
            message: format!("Failed to parse attendee: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// GET /api/public/ticket/:id?event_id=xxx
/// Public — no auth required. Returns attendee ticket data with QR image.
pub async fn get_public_ticket(
    attendee_id: &str,
    event_id: Option<&str>,
) -> Result<AttendeeData, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => {
            format!("/public/ticket/{attendee_id}?event_id={eid}")
        }
        _ => format!("/public/ticket/{attendee_id}"),
    };

    let json = cached_get(&path).await?;

    let wrapper: ApiResponse<AttendeeData> =
        serde_json::from_str(&json).map_err(|e| ApiError {
            message: format!("Failed to parse ticket data: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/checkin/:id
/// Check in an attendee by their api_id.
pub async fn check_in(id: &str, event_id: Option<&str>, online: bool) -> Result<CheckInData, ApiError> {
    let mut path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/checkin/{id}?event_id={eid}"),
        _ => format!("/checkin/{id}"),
    };
    if online {
        path = format!("{path}&online=true");
    }
    let response = api_post(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Check-in failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<CheckInData> = response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse check-in response: {e}"),
        status: 0,
    })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// Undo (revert) a recent check-in for an attendee.
///
/// Calls `POST /api/attendees/{id}/undo-checkin?event_id=...`.
/// Returns `Ok(())` on success. On 404 the backend may not support undo yet —
/// the caller should handle that gracefully.
pub async fn undo_check_in(attendee_id: &str, event_id: Option<&str>) -> Result<(), ApiError> {
    let mut path = format!("/attendees/{attendee_id}/undo-checkin");
    if let Some(eid) = event_id {
        if !eid.is_empty() {
            path = format!("{path}?event_id={eid}");
        }
    }

    let response = api_post(&path).await?;

    if !response.ok() {
        let status = response.status();
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Undo check-in failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status,
        });
    }

    Ok(())
}

// ===== Walk-in API functions =====

/// POST /api/walkin/register
/// Register a walk-in attendee for an event.
pub async fn register_walkin(req: &WalkinRegisterRequest) -> Result<WalkinRegisterResponse, ApiError> {
    let response = super::api_post_json("/walkin/register", req).await?;
    Ok(response)
}

/// GET /api/walkin/list?event_id=xxx
/// List all walk-in attendees for an event.
pub async fn list_walkins(event_id: &str) -> Result<WalkinListResponse, ApiError> {
    let path = format!("/walkin/list?event_id={event_id}");
    let cached = cached_get(&path).await?;
    let response: ApiResponse<WalkinListResponse> = serde_json::from_str(&cached).unwrap_or(ApiResponse {
        success: false,
        data: None,
        error: Some("Failed to parse response".to_string()),
        correlation_id: None,
    });
    if !response.success {
        return Err(ApiError {
            message: response.error.unwrap_or("Unknown error".to_string()),
            status: 0,
        });
    }
    response.data.ok_or_else(|| ApiError {
        message: "No data in response".to_string(),
        status: 0,
    })
}

/// GET /api/walkin/export?event_id=xxx
/// Export walk-in attendees as CSV.
pub async fn export_walkin_csv(event_id: &str) -> Result<WalkinExportResponse, ApiError> {
    let path = format!("/walkin/export?event_id={event_id}");
    let response = api_get(&path).await?;
    let result: ApiResponse<WalkinExportResponse> = response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse response: {e}"),
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

/// POST /api/walkin/sync
/// Sync walk-in attendees to Google Sheet.
pub async fn sync_walkins(event_id: &str) -> Result<WalkinSyncResponse, ApiError> {
    let req = WalkinSyncRequest {
        event_id: event_id.to_string(),
    };
    let response = super::api_post_json("/walkin/sync", &req).await?;
    Ok(response)
}

// ===== QR Generation & Cache Flush =====

/// POST /api/generate-qrs?force={force}
/// Bulk generate QR codes for all approved attendees.
///
/// When `force` is true, regenerates QR URLs even for attendees
/// that already have one (overwrites existing).
pub async fn generate_qrs(force: bool, event_id: Option<&str>) -> Result<GenerateQrData, ApiError> {
    let path = match (force, event_id) {
        (true, Some(eid)) if !eid.is_empty() => format!("/generate-qrs?force=true&event_id={eid}"),
        (false, Some(eid)) if !eid.is_empty() => format!("/generate-qrs?event_id={eid}"),
        (true, None) | (true, Some(_)) => "/generate-qrs?force=true".to_string(),
        _ => "/generate-qrs".to_string(),
    };
    let response = api_post(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("QR generation failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<GenerateQrData> = response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse QR generation response: {e}"),
        status: 0,
    })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// Flush server-side caches (attendee list + column mapping) for an event.
pub async fn flush_cache(event_id: Option<&str>) -> Result<bool, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/admin/flush-cache?event_id={eid}"),
        _ => "/admin/flush-cache".to_string(),
    };
    let response = api_post(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Flush cache failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or("Flush cache failed".to_string()),
            status: response.status(),
        });
    }

    Ok(true)
}
