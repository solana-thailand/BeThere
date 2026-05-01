//! API client for the event check-in backend.
//!
//! Provides typed response structs and authenticated request helpers
//! for all backend endpoints.

use serde::{Deserialize, Serialize};

use crate::auth::{clear_token, get_token};

/// Helper for serde `#[serde(default = "default_true")]` — defaults to `true`.
const fn default_true() -> bool {
    true
}

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
    /// Role: "super_admin" (full access), "organizer" (event management), or "staff" (scanner only).
    #[serde(default)]
    pub role: String,
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
    pub checked_in_by: Option<String>,
    #[serde(default)]
    pub qr_code_url: Option<String>,
    /// Claim token for NFT/refund claim link (set after check-in).
    #[serde(default)]
    pub claim_token: Option<String>,
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
    #[serde(default)]
    pub checked_in_by: Option<String>,
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
    pub checked_in_by: String,
    #[serde(default)]
    pub claim_token: Option<String>,
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
        .unwrap_or_else(|_| "http://localhost:8787".to_string());
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

/// Make an authenticated POST request with JSON body to the API.
async fn api_post_json<T: serde::de::DeserializeOwned + Default>(
    path: &str,
    body: &impl serde::Serialize,
) -> Result<T, ApiError> {
    let url = format!("{}{path}", api_base());
    let token = get_token();

    let json_body = serde_json::to_string(body).map_err(|e| ApiError {
        message: format!("Failed to serialize request: {e}"),
        status: 0,
    })?;

    let mut req = gloo::net::http::Request::post(&url);
    if let Some(ref t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    req = req.header("Content-Type", "application/json");

    let response = req
        .body(&json_body)
        .map_err(|e| ApiError {
            message: format!("Failed to set request body: {e:?}"),
            status: 0,
        })?
        .send()
        .await?;

    if response.status() == 401 {
        clear_token();
        return Err(ApiError {
            message: "Session expired".to_string(),
            status: 401,
        });
    }

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Request failed".to_string()),
        });
        return Err(ApiError {
            message: body
                .error
                .unwrap_or_else(|| format!("HTTP {}", response.status())),
            status: response.status(),
        });
    }

    let result: ApiResponse<T> = response.json().await.map_err(|e| ApiError {
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

/// Make an authenticated PUT request with JSON body to the API.
async fn api_put_json<T: serde::de::DeserializeOwned + Default>(
    path: &str,
    body: &impl serde::Serialize,
) -> Result<T, ApiError> {
    let url = format!("{}{path}", api_base());
    let token = get_token();

    let json_body = serde_json::to_string(body).map_err(|e| ApiError {
        message: format!("Failed to serialize request: {e}"),
        status: 0,
    })?;

    let mut req = gloo::net::http::Request::put(&url);
    if let Some(ref t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    req = req.header("Content-Type", "application/json");

    let response = req
        .body(&json_body)
        .map_err(|e| ApiError {
            message: format!("Failed to set request body: {e:?}"),
            status: 0,
        })?
        .send()
        .await?;

    if response.status() == 401 {
        clear_token();
        return Err(ApiError {
            message: "Session expired".to_string(),
            status: 401,
        });
    }

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Request failed".to_string()),
        });
        return Err(ApiError {
            message: body
                .error
                .unwrap_or_else(|| format!("HTTP {}", response.status())),
            status: response.status(),
        });
    }

    let result: ApiResponse<T> = response.json().await.map_err(|e| ApiError {
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
pub async fn get_attendees(event_id: Option<&str>) -> Result<AttendeesData, ApiError> {
    let path = match event_id {
        Some(id) if !id.is_empty() => format!("/attendees?event_id={id}"),
        _ => "/attendees".to_string(),
    };
    let response = api_get(&path).await?;

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
pub async fn get_attendee(id: &str, event_id: Option<&str>) -> Result<AttendeeData, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/attendee/{id}?event_id={eid}"),
        _ => format!("/attendee/{id}"),
    };
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
pub async fn check_in(id: &str, event_id: Option<&str>) -> Result<CheckInData, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/checkin/{id}?event_id={eid}"),
        _ => format!("/checkin/{id}"),
    };
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

// ===== Claim API types (public — no auth required) =====

/// Dynamic event metadata served from backend config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventConfig {
    #[serde(default)]
    pub event_name: String,
    #[serde(default)]
    pub event_tagline: String,
    #[serde(default)]
    pub event_link: String,
    /// Event start time as Unix epoch milliseconds.
    #[serde(default)]
    pub event_start_ms: i64,
    /// Event end time as Unix epoch milliseconds.
    #[serde(default)]
    pub event_end_ms: i64,
}

/// Quiz requirement status for a claim.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QuizStatus {
    #[default]
    NotRequired,
    NotStarted,
    InProgress,
    Passed,
}

/// Response data for GET /api/claim/{token} — attendee claim lookup.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClaimLookupData {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub checked_in_at: String,
    #[serde(default)]
    pub claim_token: String,
    #[serde(default)]
    pub claimed: bool,
    #[serde(default)]
    pub claimed_at: Option<String>,
    /// Whether NFT minting is configured on the backend.
    #[serde(default = "default_true")]
    pub nft_available: bool,
    /// Pre-registered wallet address from column P.
    /// When present, the claim is locked to this wallet — any other address is rejected.
    #[serde(default)]
    pub locked_wallet: Option<String>,
    /// Dynamic event metadata (name, tagline, link, timestamps).
    #[serde(default)]
    pub event: EventConfig,
    /// Quiz requirement status for this attendee's claim.
    #[serde(default)]
    pub quiz_status: QuizStatus,
}

/// Response data for POST /api/claim/{token} — NFT mint result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClaimMintData {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub asset_id: String,
    #[serde(default)]
    pub signature: String,
    #[serde(default)]
    pub wallet_address: String,
    #[serde(default)]
    pub claimed_at: String,
    /// Solana cluster for explorer links (e.g. "devnet", "mainnet-beta").
    #[serde(default)]
    pub cluster: String,
}

// ===== Quiz API types (public — no auth required) =====

/// A single quiz question as served to the frontend (no correct answer).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuizQuestionPublic {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub options: Vec<String>,
}

/// Response data for GET /api/quiz — quiz questions and config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuizQuestionsData {
    #[serde(default)]
    pub configured: bool,
    #[serde(default)]
    pub questions: Vec<QuizQuestionPublic>,
    #[serde(default)]
    pub passing_score_percent: u8,
    #[serde(default)]
    pub max_attempts: u8,
    #[serde(default)]
    pub time_limit_seconds: Option<u16>,
}

/// A single answer in a quiz submission (text-based, not index).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizAnswer {
    pub question_id: String,
    pub selected_text: String,
}

/// Per-question feedback after submission.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuestionExplanation {
    #[serde(default)]
    pub question_id: String,
    #[serde(default)]
    pub correct: bool,
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Response data for POST /api/quiz/{token}/submit — scored result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuizSubmitData {
    #[serde(default)]
    pub attempt_number: u8,
    #[serde(default)]
    pub score_percent: u8,
    #[serde(default)]
    pub passed: bool,
    #[serde(default)]
    pub correct_count: usize,
    #[serde(default)]
    pub total_questions: usize,
    #[serde(default)]
    pub remaining_attempts: u8,
    #[serde(default)]
    pub explanations: Vec<QuestionExplanation>,
}

/// Response data for GET /api/quiz/{token}/status — quiz progress.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuizStatusData {
    #[serde(default)]
    pub configured: bool,
    #[serde(default)]
    pub quiz_status: String,
    #[serde(default)]
    pub attempts: u8,
    #[serde(default)]
    pub max_attempts: u8,
    #[serde(default)]
    pub best_score_percent: u8,
    #[serde(default)]
    pub passed: bool,
    #[serde(default)]
    pub passing_threshold_percent: u8,
}

// ===== Admin Quiz Types =====

/// A quiz question as stored in the admin config (includes correct answer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizQuestionAdmin {
    pub id: String,
    pub text: String,
    pub options: Vec<String>,
    pub correct_index: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

/// Full quiz config for admin management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizConfigAdmin {
    pub questions: Vec<QuizQuestionAdmin>,
    pub passing_score_percent: u8,
    pub max_attempts: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_limit_seconds: Option<u16>,
}

/// Response from GET /api/admin/quiz.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AdminQuizData {
    #[serde(default)]
    pub configured: bool,
    #[serde(default)]
    pub questions: Vec<QuizQuestionAdmin>,
    #[serde(default)]
    pub passing_score_percent: u8,
    #[serde(default)]
    pub max_attempts: u8,
    #[serde(default)]
    pub time_limit_seconds: Option<u16>,
}

/// Response from POST /api/admin/quiz.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AdminQuizSaveData {
    pub questions_count: usize,
    pub passing_score_percent: u8,
    pub max_attempts: u8,
}

// ===== Event Management API Types =====

/// Event status (mirrors backend EventStatus).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    #[default]
    Draft,
    Active,
    Completed,
    Archived,
}

/// Lightweight event metadata from the events list endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventMeta {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub status: EventStatus,
    #[serde(default)]
    pub event_start_ms: i64,
    #[serde(default)]
    pub event_end_ms: i64,
    #[serde(default)]
    pub sheet_id: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub organizer_emails: Vec<String>,
}

/// Full event configuration (from GET /api/events/{id}).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventDetail {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub tagline: String,
    #[serde(default)]
    pub link: String,
    #[serde(default)]
    pub status: EventStatus,
    #[serde(default)]
    pub event_start_ms: i64,
    #[serde(default)]
    pub event_end_ms: i64,
    #[serde(default)]
    pub sheet_id: String,
    #[serde(default)]
    pub sheet_name: String,
    #[serde(default)]
    pub staff_sheet_name: String,
    #[serde(default)]
    pub quiz_enabled: bool,
    #[serde(default)]
    pub nft_collection_mint: String,
    #[serde(default)]
    pub nft_metadata_uri: String,
    #[serde(default)]
    pub nft_image_url: String,
    #[serde(default)]
    pub nft_name_template: String,
    #[serde(default)]
    pub nft_symbol: String,
    #[serde(default)]
    pub nft_description_template: String,
    #[serde(default)]
    pub merkle_tree: String,
    #[serde(default)]
    pub organizer_emails: Vec<String>,
    #[serde(default)]
    pub staff_emails: Vec<String>,
    #[serde(default)]
    pub claim_base_url: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

/// Response for GET /api/events — list all events.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct EventsListData {
    #[serde(default)]
    pub events: Vec<EventMeta>,
}

/// Response for GET /api/events/{id} — single event detail.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct EventDetailData {
    pub event: EventDetail,
}

/// Request body for POST /api/events — create event.
#[derive(Debug, Clone, Serialize, Default)]
pub struct CreateEventBody {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub tagline: String,
    #[serde(default)]
    pub link: String,
    #[serde(default)]
    pub event_start_ms: i64,
    #[serde(default)]
    pub event_end_ms: i64,
    #[serde(default)]
    pub sheet_id: String,
    #[serde(default)]
    pub sheet_name: String,
    #[serde(default)]
    pub staff_sheet_name: String,
    #[serde(default)]
    pub quiz_enabled: bool,
    #[serde(default)]
    pub nft_collection_mint: String,
    #[serde(default)]
    pub nft_metadata_uri: String,
    #[serde(default)]
    pub nft_image_url: String,
    #[serde(default)]
    pub nft_name_template: String,
    #[serde(default)]
    pub nft_symbol: String,
    #[serde(default)]
    pub nft_description_template: String,
    #[serde(default)]
    pub merkle_tree: String,
    #[serde(default)]
    pub organizer_emails: Vec<String>,
    #[serde(default)]
    pub staff_emails: Vec<String>,
    #[serde(default)]
    pub claim_base_url: String,
}

/// Request body for updating an existing event.
/// Request body for PUT /api/events/{id} — update event.
/// All fields optional for partial update.
#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateEventBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<EventStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_start_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_end_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staff_sheet_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quiz_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nft_collection_mint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nft_metadata_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nft_image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nft_name_template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nft_symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nft_description_template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merkle_tree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organizer_emails: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staff_emails: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_base_url: Option<String>,
}

/// Response from event create/update (partial data).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct EventMutationData {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub updated_at: String,
}

// ===== Claim API functions (public — no auth) =====

/// GET /api/claim/{token}
/// Look up an attendee's claim status by their claim token.
///
/// Public endpoint — no authentication required.
/// Returns attendee name, check-in time, and whether already claimed.
pub async fn get_claim(token: &str) -> Result<ClaimLookupData, ApiError> {
    let url = format!("{}/claim/{token}", api_base());
    let response = gloo::net::http::Request::get(&url).send().await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Claim lookup failed".to_string()),
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<ClaimLookupData> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse claim response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

// ===== Quiz API functions (public — no auth) =====

/// GET /api/quiz
/// Fetch quiz questions for the frontend (no correct answers).
///
/// Public endpoint — no authentication required.
pub async fn get_quiz() -> Result<QuizQuestionsData, ApiError> {
    let url = format!("{}/quiz", api_base());
    let response = gloo::net::http::Request::get(&url).send().await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Quiz fetch failed".to_string()),
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<QuizQuestionsData> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse quiz response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

// ===== Event Management API functions (admin) =====

/// GET /api/events — list all events.
pub async fn list_events() -> Result<EventsListData, ApiError> {
    let response = api_get("/events").await?;
    let result: ApiResponse<EventsListData> = response.json().await.map_err(|e| ApiError {
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

/// GET /api/events/{id} — get full event config.
pub async fn get_event_detail(id: &str) -> Result<EventDetailData, ApiError> {
    let path = format!("/events/{id}");
    let response = api_get(&path).await?;
    let result: ApiResponse<EventDetailData> = response.json().await.map_err(|e| ApiError {
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

/// DELETE /api/events/{id} — archive an event.
pub async fn archive_event(id: &str) -> Result<EventMutationData, ApiError> {
    let path = format!("/events/{id}");
    let response = api_post(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Archive failed".to_string()),
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<EventMutationData> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse archive response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

// ===== Admin Quiz Management =====

/// Get the full quiz configuration (admin only, includes correct answers).
pub async fn get_admin_quiz(event_id: Option<&str>) -> Result<AdminQuizData, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/admin/quiz?event_id={eid}"),
        _ => "/admin/quiz".to_string(),
    };
    let response = api_get(&path).await?;
    let result: ApiResponse<AdminQuizData> = response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse admin quiz response: {e}"),
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

/// Save quiz configuration (admin only).
pub async fn put_admin_quiz(config: &QuizConfigAdmin, event_id: Option<&str>) -> Result<AdminQuizSaveData, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/admin/quiz?event_id={eid}"),
        _ => "/admin/quiz".to_string(),
    };
    api_post_json(&path, config).await
}

/// POST /api/quiz/{token}/submit
/// Submit quiz answers for scoring.
///
/// Public endpoint — no authentication required.
/// The attendee must be checked in (valid claim token).
pub async fn submit_quiz(
    token: &str,
    answers: &[QuizAnswer],
) -> Result<QuizSubmitData, ApiError> {
    let url = format!("{}/quiz/{token}/submit", api_base());
    let body = serde_json::json!({ "answers": answers });

    let response = gloo::net::http::Request::post(&url)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body).unwrap_or_default())
        .map_err(|e| ApiError {
            message: format!("Failed to build request: {e}"),
            status: 0,
        })?
        .send()
        .await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Quiz submit failed".to_string()),
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<QuizSubmitData> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse quiz submit response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// GET /api/quiz/{token}/status
/// Get quiz progress for an attendee.
///
/// Public endpoint — no authentication required.
pub async fn get_quiz_status(token: &str) -> Result<QuizStatusData, ApiError> {
    let url = format!("{}/quiz/{token}/status", api_base());
    let response = gloo::net::http::Request::get(&url).send().await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Quiz status fetch failed".to_string()),
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<QuizStatusData> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse quiz status response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/claim/{token}
/// Mint a compressed NFT to the given wallet address.
///
/// Public endpoint — no authentication required.
/// The attendee must be checked in and not already claimed.
pub async fn post_claim(token: &str, wallet_address: &str) -> Result<ClaimMintData, ApiError> {
    let url = format!("{}/claim/{token}", api_base());
    let body = serde_json::json!({ "wallet_address": wallet_address });

    let response = gloo::net::http::Request::post(&url)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body).unwrap_or_default())
        .map_err(|e| ApiError {
            message: format!("Failed to build request: {e}"),
            status: 0,
        })?
        .send()
        .await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Claim mint failed".to_string()),
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<ClaimMintData> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse mint response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}
