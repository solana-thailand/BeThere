//! Core response types shared across all API domain modules.

use serde::{Deserialize, Serialize};

/// Helper for serde `#[serde(default = "default_true")]` — defaults to `true`.
pub(crate) const fn default_true() -> bool {
    true
}

// ===== Error & Response wrapper =====

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

// ===== Auth response types =====

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

// ===== Attendee response types =====

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

/// Lightweight attendee summary for list views.
/// Omits `claim_token` (only needed for single-attendee detail).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AttendeeListItem {
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
    pub attendees: Vec<AttendeeListItem>,
    #[serde(default)]
    pub stats: StatsResponse,
    /// Cursor for the next page (row_index of last item in current page).
    #[serde(default)]
    pub next_cursor: Option<usize>,
    /// Whether more pages exist beyond this response.
    #[serde(default)]
    pub has_more: bool,
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
