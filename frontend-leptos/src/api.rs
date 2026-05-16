//! API client for the event check-in backend.
//!
//! Provides typed response structs and authenticated request helpers
//! for all backend endpoints.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::auth::{clear_token, get_token};

// ---------------------------------------------------------------------------
// B5: Stale-while-revalidate in-memory API response cache
// ---------------------------------------------------------------------------

/// Cache entry holding serialized JSON and insertion timestamp.
struct CacheEntry {
    json: String,
    inserted_at: Duration,
}

// Global API response cache (keyed by URL path).
thread_local! {
    static API_CACHE: RefCell<HashMap<String, CacheEntry>> =
        RefCell::new(HashMap::new());
}

/// TTL for cached GET responses (30 seconds).
/// Attendee list is cached for 5 minutes on the worker side (KV),
/// so a 30s client-side stale window avoids redundant HTTP round-trips
/// while staying fresh enough for check-in operations.
const CACHE_TTL: Duration = Duration::from_secs(30);

/// Try to get a cached response for the given path.
/// Returns `None` if not cached or expired.
fn cache_get(path: &str) -> Option<String> {
    let now = now_secs();
    API_CACHE.with(|c| {
        let cache = c.borrow();
        cache.get(path).and_then(|entry| {
            if now.saturating_sub(entry.inserted_at) < CACHE_TTL {
                Some(entry.json.clone())
            } else {
                None
            }
        })
    })
}

/// Store a response in the cache.
fn cache_put(path: &str, json: &str) {
    let now = now_secs();
    API_CACHE.with(|c| {
        c.borrow_mut().insert(
            path.to_string(),
            CacheEntry {
                json: json.to_string(),
                inserted_at: now,
            },
        );
    });
}

/// Invalidate a cached response (e.g. after a mutation).
fn cache_invalidate(path_prefix: &str) {
    API_CACHE.with(|c| {
        c.borrow_mut().retain(|k, _| !k.starts_with(path_prefix));
    });
}

/// Current time as Duration from some epoch (performance.now in WASM).
fn now_secs() -> Duration {
    // In WASM, use performance.now() for sub-ms precision.
    // Fall back to 0 if window is unavailable.
    let ms = web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now() as u64)
        .unwrap_or(0);
    Duration::from_millis(ms)
}

/// Perform a cached GET request. Returns cached response if fresh,
/// otherwise fetches from the server.
///
/// Uses stale-while-revalidate: returns stale data immediately,
/// then fetches fresh data in the background.
async fn cached_get(path: &str) -> Result<String, ApiError> {
    // Check cache first
    if let Some(cached_json) = cache_get(path) {
        return Ok(cached_json);
    }

    // Cache miss — fetch from server
    let response = api_get(path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Request failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let json = response.text().await.map_err(|e| ApiError {
        message: format!("Failed to read response: {e}"),
        status: 0,
    })?;

    // Store in cache
    cache_put(path, &json);

    Ok(json)
}

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthUrlResponse {
    pub auth_url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    #[serde(default)]
    pub correlation_id: Option<String>,
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
            correlation_id: None,
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

/// Make an authenticated DELETE request to the API.
async fn api_delete(path: &str) -> Result<gloo::net::http::Response, ApiError> {
    let url = format!("{}{path}", api_base());
    let token = get_token();

    let mut req = gloo::net::http::Request::delete(&url);
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
            correlation_id: None,
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
            correlation_id: None,
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

    let result: ApiResponse<AuthUrlResponse> = response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse auth URL response: {e}"),
        status: 0,
    })?;

    result.data.ok_or_else(|| ApiError {
        message: "No data in auth URL response".to_string(),
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

    let result: ApiResponse<MeResponse> = response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse user info: {e}"),
        status: 0,
    })?;

    result.data.ok_or_else(|| ApiError {
        message: "No data in user info response".to_string(),
        status: 0,
    })
}

/// GET /api/attendees
/// Returns all attendees with stats.
///
/// Results are cached client-side for 30 seconds (B5).
/// Call `invalidate_attendee_cache()` after mutations (check-in, QR gen).
pub async fn get_attendees(event_id: Option<&str>) -> Result<AttendeesData, ApiError> {
    let path = match event_id {
        Some(id) if !id.is_empty() => format!("/attendees?event_id={id}"),
        _ => "/attendees".to_string(),
    };

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

// ===== Walk-in Registration =====

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

/// POST /api/walkin/register
/// Register a walk-in attendee for an event.
pub async fn register_walkin(req: &WalkinRegisterRequest) -> Result<WalkinRegisterResponse, ApiError> {
    let response = api_post_json("/walkin/register", req).await?;
    Ok(response)
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
    /// Total number of attendees checked in for this event.
    #[serde(default)]
    pub total_checked_in: usize,
    /// Total number of attendees who have claimed their NFT.
    #[serde(default)]
    pub total_claimed: usize,
    /// Attendee's API ID (for deposit page link: /deposit/{api_id}).
    #[serde(default)]
    pub api_id: String,
    /// Event ID (for deposit page link query param).
    #[serde(default)]
    pub event_id: String,
    /// Whether deposit is enabled for this event.
    #[serde(default)]
    pub deposit_enabled: bool,
    /// Deposit amount in USDC (smallest unit, e.g. 15000000 = 15 USDC).
    #[serde(default)]
    pub deposit_amount_usdc: u64,
    /// Deposit amount in THB (e.g. 500).
    #[serde(default)]
    pub deposit_amount_thb: u64,
    /// Attendee's participation type ("In-Person", "Online", etc.).
    #[serde(default)]
    pub participation_type: String,
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

/// On-chain escrow lifecycle status (mirrors backend EscrowStatus).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EscrowStatus {
    #[default]
    None,
    Initialized,
    Deactivated,
    Closed,
}

impl EscrowStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Initialized => "initialized",
            Self::Deactivated => "deactivated",
            Self::Closed => "closed",
        }
    }

    /// Whether the escrow is considered "active" (blocking archive/delete).
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Initialized | Self::Deactivated)
    }
}

/// Event format (mirrors backend EventFormat).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventFormat {
    #[default]
    InPerson,
    Online,
    Hybrid,
}

impl EventFormat {
    pub fn label(&self) -> &'static str {
        match self {
            Self::InPerson => "In-Person",
            Self::Online => "Online",
            Self::Hybrid => "Hybrid",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InPerson => "in_person",
            Self::Online => "online",
            Self::Hybrid => "hybrid",
        }
    }

    /// Whether this format includes an in-person track.
    pub fn has_in_person(&self) -> bool {
        matches!(self, Self::InPerson | Self::Hybrid)
    }

    /// Whether this format includes an online track.
    pub fn has_online(&self) -> bool {
        matches!(self, Self::Online | Self::Hybrid)
    }
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
    #[serde(default)]
    pub deposit_enabled: bool,
    #[serde(default)]
    pub escrow_address: String,
    #[serde(default)]
    pub escrow_status: EscrowStatus,
    #[serde(default)]
    pub event_format: EventFormat,
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
    pub deposit_enabled: bool,
    #[serde(default)]
    pub deposit_amount_usdc: u64,
    #[serde(default)]
    pub deposit_amount_thb: u64,
    #[serde(default)]
    pub promptpay_id: String,
    #[serde(default)]
    pub escrow_address: String,
    #[serde(default)]
    pub escrow_status: EscrowStatus,
    #[serde(default)]
    pub organizer_wallet: String,
    #[serde(default)]
    pub on_chain_event_id: u64,
    #[serde(default)]
    pub refund_deadline_hours: u32,
    #[serde(default)]
    pub max_refundable_deposits: u32,
    #[serde(default)]
    pub event_format: EventFormat,
    #[serde(default = "default_true_fn")]
    pub require_contact_info: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

/// Default true helper for serde.
fn default_true_fn() -> bool {
    true
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
    #[serde(default)]
    pub deposit_enabled: bool,
    #[serde(default)]
    pub deposit_amount_usdc: u64,
    #[serde(default)]
    pub deposit_amount_thb: u64,
    #[serde(default)]
    pub promptpay_id: String,
    #[serde(default)]
    pub escrow_address: String,
    #[serde(default)]
    pub organizer_wallet: String,
    #[serde(default)]
    pub on_chain_event_id: u64,
    #[serde(default)]
    pub refund_deadline_hours: u32,
    #[serde(default)]
    pub max_refundable_deposits: u32,
    #[serde(default)]
    pub event_format: EventFormat,
    #[serde(default = "default_true_fn")]
    pub require_contact_info: bool,
}
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_amount_usdc: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_amount_thb: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promptpay_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escrow_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escrow_status: Option<EscrowStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organizer_wallet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_chain_event_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_deadline_hours: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_refundable_deposits: Option<u32>,
    /// Optimistic concurrency: matches server `updated_at` to prevent blind overwrites.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_format: Option<EventFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_contact_info: Option<bool>,
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
/// Results are cached client-side for 30 seconds (B5).
pub async fn get_claim(token: &str) -> Result<ClaimLookupData, ApiError> {
    let path = format!("/claim/{token}");
    let json = cached_get(&path).await?;

    let wrapper: ApiResponse<ClaimLookupData> =
        serde_json::from_str(&json).map_err(|e| ApiError {
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
            correlation_id: None,
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

// ---------------------------------------------------------------------------
// Escrow — init (combined ATA + CreateEvent in one TX)
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/init.
#[derive(Debug, Clone, Serialize)]
pub struct InitEscrowRequest {
    pub event_id: String,
}

/// Response from POST /api/escrow/init.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct InitEscrowResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet signs).
    pub transaction: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
    /// Derived EventEscrow PDA address (base58).
    pub escrow_address: String,
    /// Derived vault ATA address (base58).
    pub vault_address: String,
    /// The on-chain event ID used for PDA derivation.
    pub on_chain_event_id: u64,
}

/// POST /api/escrow/init — combined ATA + create_event in one transaction.
pub async fn init_escrow(body: &InitEscrowRequest) -> Result<InitEscrowResponse, ApiError> {
    api_post_json("/escrow/init", body).await
}


/// DELETE /api/events/{id} — archive an event.
pub async fn archive_event(id: &str) -> Result<EventMutationData, ApiError> {
    let path = format!("/events/{id}");
    let response = api_delete(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
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
        response.json().await.map_err(|e| ApiError {
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
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
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
        response.json().await.map_err(|e| ApiError {
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
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
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
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse restore response: {e}"),
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
            correlation_id: None,
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
            correlation_id: None,
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
            correlation_id: None,
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

// ===== Adventure types =====

/// Adventure status from GET /api/adventure/{token}/status
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AdventureStatusType {
    #[default]
    NotRequired,
    NotStarted,
    InProgress,
    Passed,
}

/// Level score from the adventure API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdventureLevelScore {
    #[serde(default)]
    pub moves: u32,
    #[serde(default)]
    pub puzzles_solved: u32,
    #[serde(default)]
    pub time_seconds: u32,
    #[serde(default)]
    pub stars: u8,
}

/// Adventure progress data from the API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdventureProgressData {
    #[serde(default)]
    pub claim_token: String,
    #[serde(default)]
    pub levels_completed: Vec<String>,
    #[serde(default)]
    pub scores: std::collections::HashMap<String, AdventureLevelScore>,
    #[serde(default)]
    pub total_moves: u32,
    #[serde(default)]
    pub total_time_seconds: u32,
    #[serde(default)]
    pub passed: bool,
    #[serde(default)]
    pub passed_at: Option<String>,
    #[serde(default)]
    pub last_played_at: Option<String>,
}

/// Response from GET /api/adventure/{token}/status
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdventureStatusData {
    #[serde(default)]
    pub status: AdventureStatusType,
    #[serde(default)]
    pub progress: Option<AdventureProgressData>,
}

/// Request body for POST /api/adventure/{token}/save
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdventureSaveBody {
    pub claim_token: String,
    pub level_id: String,
    pub score: AdventureLevelScore,
}

/// GET /api/adventure/{token}/status
/// Get adventure status and progress for a claim token.
///
/// Public endpoint — no authentication required.
pub async fn get_adventure_status(token: &str) -> Result<AdventureStatusData, ApiError> {
    let url = format!("{}/adventure/{token}/status", api_base());
    let response = gloo::net::http::Request::get(&url).send().await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Adventure status fetch failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<AdventureStatusData> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse adventure status response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/adventure/{token}/save
/// Save level completion progress.
///
/// Public endpoint — no authentication required.
pub async fn save_adventure_progress(
    token: &str,
    level_id: &str,
    score: &AdventureLevelScore,
) -> Result<AdventureProgressData, ApiError> {
    let url = format!("{}/adventure/{token}/save", api_base());
    let body = AdventureSaveBody {
        claim_token: token.to_string(),
        level_id: level_id.to_string(),
        score: score.clone(),
    };

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
            error: Some("Adventure save failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    // Response is { success: true, data: { progress: ... } }
    #[derive(Debug, Default, Deserialize)]
    struct SaveResponse {
        #[serde(default)]
        progress: AdventureProgressData,
    }
    let wrapper: ApiResponse<SaveResponse> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse adventure save response: {e}"),
            status: 0,
        })?;

    wrapper
        .data
        .map(|d| d.progress)
        .ok_or_else(|| ApiError {
            message: wrapper.error.unwrap_or("No data".to_string()),
            status: 0,
        })
}

// ===== Adventure Admin API =====

/// Adventure config from GET /api/admin/adventure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdventureConfigData {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub required_level: Option<usize>,
}

/// GET /api/admin/adventure
/// Get adventure config for the active event.
pub async fn get_admin_adventure_config(
    event_id: Option<&str>,
) -> Result<AdventureConfigData, ApiError> {
    let mut url = format!("{}/admin/adventure", api_base());
    if let Some(eid) = event_id {
        url = format!("{url}?event_id={eid}");
    }
    let response = gloo::net::http::Request::get(&url).send().await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to fetch adventure config".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or("Failed to fetch adventure config".to_string()),
            status: response.status(),
        });
    }

    #[derive(Default, Deserialize)]
    struct ConfigResponse {
        #[serde(default)]
        config: Option<AdventureConfigData>,
    }

    let wrapper: ApiResponse<ConfigResponse> = response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse adventure config: {e}"),
        status: 0,
    })?;

    wrapper
        .data
        .and_then(|d| d.config)
        .ok_or_else(|| ApiError {
            message: "No config data".to_string(),
            status: 0,
        })
}

/// PUT /api/admin/adventure
/// Update adventure config for the active event.
pub async fn put_admin_adventure_config(
    config: &AdventureConfigData,
    event_id: Option<&str>,
) -> Result<AdventureConfigData, ApiError> {
    let mut url = format!("{}/admin/adventure", api_base());
    if let Some(eid) = event_id {
        url = format!("{url}?event_id={eid}");
    }
    let response = gloo::net::http::Request::put(&url)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(config).unwrap_or_default())
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
            error: Some("Failed to save adventure config".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or("Failed to save adventure config".to_string()),
            status: response.status(),
        });
    }

    // Return the config we sent (backend echoes it back)
    Ok(config.clone())
}

// ===== Deposit/Refund types =====

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DepositStatusInfo {
    pub attendee_id: String,
    pub event_id: String,
    pub method: String,
    pub amount: u64,
    pub currency: String,
    pub tx_signature: Option<String>,
    pub verified: bool,
    pub deposited_at: String,
    #[serde(default)]
    pub wallet_address: Option<String>,
    /// Deposit order within this event (1-based).
    #[serde(default)]
    pub deposit_order: u32,
    /// Whether this deposit is in the refundable tier.
    #[serde(default = "default_true")]
    pub refundable: bool,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct DepositStatusResponse {
    pub deposit_enabled: bool,
    pub deposit_amount_usdc: u64,
    pub deposit_amount_thb: u64,
    pub promptpay_id: String,
    #[serde(default)]
    pub event_start_ms: i64,
    #[serde(default)]
    pub event_end_ms: i64,
    #[serde(default)]
    pub refund_deadline_hours: u32,
    #[serde(default)]
    pub event_name: String,
    #[serde(default)]
    pub event_tagline: String,
    pub status: Option<DepositStatusInfo>,
    /// Whether the backend is in dev mode (shows Solana wallet options).
    #[serde(default)]
    pub dev_mode: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UsdcDepositRequest {
    pub event_id: String,
    pub attendee_id: String,
    pub wallet_address: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct UsdcDepositResponse {
    pub transaction: String,
    pub solana_pay_url: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ThbSlipUploadRequest {
    pub event_id: String,
    pub attendee_id: String,
    pub slip_url: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VerifySlipRequest {
    pub event_id: String,
    pub attendee_id: String,
    pub approved: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ThbDepositInfo {
    pub attendee_id: String,
    pub event_id: String,
    pub amount_thb: u64,
    pub slip_url: Option<String>,
    pub verified: bool,
    pub verified_by: Option<String>,
    pub verified_at: Option<String>,
    pub uploaded_at: String,
    pub refunded: bool,
    pub refunded_at: Option<String>,
    #[serde(default)]
    pub attendee_name: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PendingSlipResponse {
    pub slips: Vec<ThbDepositInfo>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct RefundQueueResponse {
    pub pending: Vec<ThbDepositInfo>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MarkRefundRequest {
    pub event_id: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ConfirmDepositResponse {
    pub confirmed: bool,
    pub tx_signature: Option<String>,
    pub solana_pay_url: Option<String>,
}

// ===== Deposit/Refund API =====

/// GET /api/deposit/status/{attendee_id}?event_id=xxx
pub async fn get_deposit_status(
    attendee_id: &str,
    event_id: Option<&str>,
) -> Result<DepositStatusResponse, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/deposit/status/{attendee_id}?event_id={eid}"),
        _ => format!("/deposit/status/{attendee_id}"),
    };
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to get deposit status".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<DepositStatusResponse> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse deposit status: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/deposit/usdc
pub async fn deposit_usdc(body: &UsdcDepositRequest) -> Result<UsdcDepositResponse, ApiError> {
    api_post_json("/deposit/usdc", body).await
}

/// POST /api/deposit/usdc/webhook — record TX signature
pub async fn record_deposit_tx(
    event_id: &str,
    attendee_id: &str,
    tx_signature: &str,
) -> Result<serde_json::Value, ApiError> {
    let body = serde_json::json!({
        "event_id": event_id,
        "attendee_id": attendee_id,
        "tx_signature": tx_signature,
    });
    api_post_json("/deposit/usdc/webhook", &body).await
}

/// GET /api/deposit/usdc/confirm?event_id=xxx&attendee_id=xxx
pub async fn confirm_deposit(
    event_id: &str,
    attendee_id: &str,
) -> Result<ConfirmDepositResponse, ApiError> {
    let path = format!("/deposit/usdc/confirm?event_id={event_id}&attendee_id={attendee_id}");
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to check deposit confirmation".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<ConfirmDepositResponse> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse deposit confirmation: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/deposit/thb/upload
pub async fn upload_thb_slip(body: &ThbSlipUploadRequest) -> Result<serde_json::Value, ApiError> {
    api_post_json("/deposit/thb/upload", body).await
}

/// POST /api/deposit/thb/verify (admin)
pub async fn verify_thb_slip(body: &VerifySlipRequest) -> Result<serde_json::Value, ApiError> {
    api_post_json("/deposit/thb/verify", body).await
}

/// GET /api/deposit/thb/pending?event_id=xxx
pub async fn get_pending_slips(event_id: Option<&str>) -> Result<PendingSlipResponse, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/deposit/thb/pending?event_id={eid}"),
        _ => "/deposit/thb/pending".to_string(),
    };
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to get pending slips".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<PendingSlipResponse> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse pending slips: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// GET /api/refund/queue?event_id=xxx
pub async fn get_refund_queue(event_id: Option<&str>) -> Result<RefundQueueResponse, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/refund/queue?event_id={eid}"),
        _ => "/refund/queue".to_string(),
    };
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to get refund queue".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<RefundQueueResponse> =
        response.json().await.map_err(|e| ApiError {
            message: format!("Failed to parse refund queue: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/refund/mark/{attendee_id}
pub async fn mark_refund(
    attendee_id: &str,
    body: &MarkRefundRequest,
) -> Result<serde_json::Value, ApiError> {
    let path = format!("/refund/mark/{attendee_id}");
    api_post_json(&path, body).await
}

// ---------------------------------------------------------------------------
// Escrow Refund
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/refund.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RefundTxRequest {
    pub event_id: String,
    pub attendee_id: String,
    pub wallet_address: String,
}

/// Response from POST /api/escrow/refund.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct RefundTxResponse {
    pub transaction: String,
    pub message: String,
}

/// POST /api/escrow/refund — build refund TX
pub async fn build_refund_tx(body: &RefundTxRequest) -> Result<RefundTxResponse, ApiError> {
    api_post_json("/escrow/refund", body).await
}

// ---------------------------------------------------------------------------
// Escrow: Mark Checked In (admin)
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/mark-checked-in.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MarkCheckedInRequest {
    pub event_id: String,
    /// Attendee API ID — backend resolves wallet from deposit record.
    pub attendee_id: String,
}

/// Response from POST /api/escrow/mark-checked-in.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct MarkCheckedInResponse {
    pub transaction: String,
    pub message: String,
}

/// POST /api/escrow/mark-checked-in — mark attendee checked in
pub async fn mark_checked_in(body: &MarkCheckedInRequest) -> Result<MarkCheckedInResponse, ApiError> {
    api_post_json("/escrow/mark-checked-in", body).await
}

// ---------------------------------------------------------------------------
// Escrow: Deactivate Event (admin)
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/deactivate-event.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeactivateEventRequest {
    pub event_id: String,
}

/// Response from POST /api/escrow/deactivate-event.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct DeactivateEventResponse {
    pub transaction: String,
    pub message: String,
}

/// POST /api/escrow/deactivate-event — build deactivate_event TX
pub async fn deactivate_event(body: &DeactivateEventRequest) -> Result<DeactivateEventResponse, ApiError> {
    api_post_json("/escrow/deactivate-event", body).await
}

// ---------------------------------------------------------------------------
// Escrow: Close Event (admin)
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/close-event.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CloseEventRequest {
    pub event_id: String,
}

/// Response from POST /api/escrow/close-event.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CloseEventResponse {
    pub transaction: String,
    pub message: String,
}

/// POST /api/escrow/close-event — build close_event TX
pub async fn close_event(body: &CloseEventRequest) -> Result<CloseEventResponse, ApiError> {
    api_post_json("/escrow/close-event", body).await
}

// ---------------------------------------------------------------------------
// Escrow: Close Deposit (attendee reclaims rent)
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/close-deposit.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CloseDepositRequest {
    pub event_id: String,
    pub attendee_id: String,
    pub wallet_address: String,
}

/// Response from POST /api/escrow/close-deposit.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CloseDepositResponse {
    pub transaction: String,
    pub message: String,
}

/// POST /api/escrow/close-deposit — build close_deposit TX
pub async fn close_deposit(body: &CloseDepositRequest) -> Result<CloseDepositResponse, ApiError> {
    api_post_json("/escrow/close-deposit", body).await
}

// ---------------------------------------------------------------------------
// Escrow: Claim Forfeited (admin)
// ---------------------------------------------------------------------------

/// Request body for POST /api/escrow/claim-forfeited.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClaimForfeitedRequest {
    pub event_id: String,
}

/// Response from POST /api/escrow/claim-forfeited.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ClaimForfeitedResponse {
    pub transaction: String,
    pub message: String,
}

/// POST /api/escrow/claim-forfeited — build claim_forfeited TX
pub async fn claim_forfeited(body: &ClaimForfeitedRequest) -> Result<ClaimForfeitedResponse, ApiError> {
    api_post_json("/escrow/claim-forfeited", body).await
}

// ---------------------------------------------------------------------------
// Audit Trail
// ---------------------------------------------------------------------------

/// A single audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub description: String,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Response from GET /api/events/{id}/audit.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditResponse {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub entries: Vec<AuditEntry>,
}

/// GET /api/events/{id}/audit — fetch audit trail for an event.
pub async fn get_event_audit(event_id: &str) -> Result<AuditResponse, ApiError> {
    let path = format!("/events/{event_id}/audit");
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to fetch audit trail".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let result: ApiResponse<AuditResponse> = response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse audit response: {e}"),
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

// ---------------------------------------------------------------------------
// On-Chain Escrow Events
// ---------------------------------------------------------------------------

/// On-chain escrow instruction type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EscrowInstruction {
    CreateEvent,
    Deposit,
    MarkCheckedIn,
    Refund,
    ClaimForfeited,
    CloseEvent,
    DeactivateEvent,
    CloseDeposit,
    Unknown,
}

impl EscrowInstruction {
    pub fn label(&self) -> &'static str {
        match self {
            Self::CreateEvent => "Create Event",
            Self::Deposit => "Deposit",
            Self::MarkCheckedIn => "Check In",
            Self::Refund => "Refund",
            Self::ClaimForfeited => "Claim Forfeited",
            Self::CloseEvent => "Close Event",
            Self::DeactivateEvent => "Deactivate",
            Self::CloseDeposit => "Close Deposit",
            Self::Unknown => "Unknown",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            Self::CreateEvent => "#6366f1",    // indigo
            Self::Deposit => "#3b82f6",       // blue
            Self::MarkCheckedIn => "#22c55e",  // green
            Self::Refund => "#eab308",        // yellow
            Self::ClaimForfeited => "#f97316", // orange
            Self::CloseEvent => "#ef4444",     // red
            Self::DeactivateEvent => "#a855f7", // purple
            Self::CloseDeposit => "#64748b",   // slate
            Self::Unknown => "#94a3b8",        // gray
        }
    }
}

/// A single on-chain escrow event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnChainEvent {
    pub signature: String,
    pub slot: u64,
    pub block_time: i64,
    pub instruction: EscrowInstruction,
    pub escrow_address: String,
    #[serde(default)]
    pub organizer: Option<String>,
    #[serde(default)]
    pub attendee: Option<String>,
    #[serde(default)]
    pub amount: Option<u64>,
    pub indexed_at: String,
}

/// Response for GET /api/escrow/events/{event_id}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OnchainEventsResponse {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub escrow_address: String,
    #[serde(default)]
    pub events: Vec<OnChainEvent>,
}

/// GET /api/escrow/events/{event_id} — fetch indexed on-chain events
pub async fn get_onchain_events(event_id: &str) -> Result<OnchainEventsResponse, ApiError> {
    let path = format!("/escrow/events/{event_id}");
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response.json().await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to fetch on-chain events".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let result: ApiResponse<OnchainEventsResponse> = response.json().await.map_err(|e| ApiError {
        message: format!("Failed to parse on-chain events: {e}"),
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
