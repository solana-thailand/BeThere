//! API client for the event check-in backend.
//!
//! Provides typed response structs and authenticated request helpers
//! for all backend endpoints.

use serde::{Deserialize, Serialize};

use crate::auth::{clear_token, get_token};

/// API error type.
#[derive(Debug)]
pub struct ApiError {
    pub message: String,
    pub status: u16,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "API error ({}): {}", self.status, self.message)
    }
}

impl From<gloo::net::Error> for ApiError {
    fn from(err: gloo::net::Error) -> Self {
        Self {
            message: format!("{err}"),
            status: 0,
        }
    }
}

// ===== Response types matching server API =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUrlResponse {
    pub auth_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeResponse {
    pub email: String,
    pub sub: String,
    pub is_staff: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AttendeeResponse {
    #[serde(default)]
    pub api_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub ticket_name: String,
    #[serde(default)]
    pub approval_status: String,
    #[serde(default)]
    pub checked_in_at: Option<String>,
    #[serde(default)]
    pub qr_code_url: Option<String>,
    #[serde(default)]
    pub row_index: usize,
    /// Participation type from Google Sheet column Y (e.g. "In-Person", "Online").
    #[serde(default)]
    pub participation_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentCheckIn {
    pub api_id: String,
    pub name: String,
    pub checked_in_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatsResponse {
    #[serde(default)]
    pub total_approved: usize,
    #[serde(default)]
    pub total_checked_in: usize,
    #[serde(default)]
    pub total_remaining: usize,
    #[serde(default)]
    pub check_in_percentage: f64,
    #[serde(default)]
    pub recent_check_ins: Vec<RecentCheckIn>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AttendeesData {
    #[serde(default)]
    pub attendees: Vec<AttendeeResponse>,
    #[serde(default)]
    pub stats: StatsResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AttendeeData {
    #[serde(default)]
    pub attendee: AttendeeResponse,
    #[serde(default)]
    pub qr_image: Option<String>,
    #[serde(default)]
    pub is_checked_in: bool,
    #[serde(default)]
    pub is_approved: bool,
    /// Whether the attendee is in-person (from backend `is_in_person()`).
    #[serde(default)]
    pub is_in_person: bool,
    /// Raw participation type string from backend.
    #[serde(default)]
    pub participation_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CheckInData {
    #[serde(default)]
    pub api_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub checked_in_at: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QrGenerationDetail {
    #[serde(default)]
    pub api_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub qr_code_url: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GenerateQrData {
    #[serde(default)]
    pub total: usize,
    #[serde(default)]
    pub generated: usize,
    #[serde(default)]
    pub skipped: usize,
    #[serde(default)]
    pub details: Vec<QrGenerationDetail>,
}

/// Generic API response wrapper matching server format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(default)]
    pub data: Option<T>,
    #[serde(default)]
    pub error: Option<String>,
}

// ===== Base URL helper =====

/// Get the API base URL from the current window location.
fn api_base() -> String {
    let window = web_sys::window().expect("no window");
    let location = window.location();
    let origin = location
        .origin()
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    format!("{origin}/api")
}

// ===== Authenticated request helpers =====

/// Make an authenticated GET request to the API.
async fn api_get(path: &str) -> Result<gloo::net::http::Response, ApiError> {
    let url = format!("{}{path}", api_base());
    let token = get_token();

    let mut req = gloo::net::http::Request::get(&url);
    if let Some(ref t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }

    let response = req.send().await?;

    if response.status() == 401 {
        clear_token();
        return Err(ApiError {
            message: "Session expired".to_string(),
            status: 401,
        });
    }

    if response.status() == 403 {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Access denied".to_string()),
        });
        return Err(ApiError {
            message: body.error.unwrap_or("Access denied".to_string()),
            status: 403,
        });
    }

    Ok(response)
}

/// Make an authenticated POST request to the API.
async fn api_post(path: &str) -> Result<gloo::net::http::Response, ApiError> {
    let url = format!("{}{path}", api_base());
    let token = get_token();

    let mut req = gloo::net::http::Request::post(&url);
    if let Some(ref t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    req = req.header("Content-Type", "application/json");

    let response = req.send().await?;

    if response.status() == 401 {
        clear_token();
        return Err(ApiError {
            message: "Session expired".to_string(),
            status: 401,
        });
    }

    Ok(response)
}

// ===== Public API functions =====

/// GET /api/auth/url
/// Returns the Google OAuth 2.0 authorization URL.
pub async fn get_auth_url() -> Result<AuthUrlResponse, ApiError> {
    let url = format!("{}/auth/url", api_base());
    let response = gloo::net::http::Request::get(&url).send().await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Failed to get auth URL".to_string(),
            status: response.status(),
        });
    }

    response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse auth URL response: {e}"),
        status: 0,
    })
}

/// GET /api/auth/me
/// Returns the current authenticated user info.
pub async fn get_me() -> Result<MeResponse, ApiError> {
    let response = api_get("/auth/me").await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Failed to get user info".to_string(),
            status: response.status(),
        });
    }

    response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse user info: {e}"),
        status: 0,
    })
}

/// GET /api/attendees
/// Returns all attendees with stats.
pub async fn get_attendees() -> Result<AttendeesData, ApiError> {
    let response = api_get("/attendees").await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to load attendees".to_string()),
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<AttendeesData> = response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse attendees: {e}"),
        status: 0,
    })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// GET /api/attendee/:id
/// Returns a single attendee by their api_id.
pub async fn get_attendee(id: &str) -> Result<AttendeeData, ApiError> {
    let path = format!("/attendee/{id}");
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Attendee not found".to_string()),
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<AttendeeData> = response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse attendee: {e}"),
        status: 0,
    })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/checkin/:id
/// Check in an attendee by their api_id.
pub async fn check_in(id: &str) -> Result<CheckInData, ApiError> {
    let path = format!("/checkin/{id}");
    let response = api_post(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Check-in failed".to_string()),
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

/// POST /api/generate-qrs?force={force}
/// Bulk generate QR codes for all approved attendees.
///
/// When `force` is true, regenerates QR URLs even for attendees
/// that already have one (overwrites existing).
pub async fn generate_qrs(force: bool) -> Result<GenerateQrData, ApiError> {
    let path = if force {
        "/generate-qrs?force=true"
    } else {
        "/generate-qrs"
    };
    let response = api_post(path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("QR generation failed".to_string()),
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
