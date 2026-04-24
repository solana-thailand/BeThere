use serde::{Deserialize, Serialize};

/// Response for a single attendee or attendee in a list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttendeeResponse {
    pub api_id: String,
    pub name: String,
    pub email: String,
    pub ticket_name: String,
    pub approval_status: String,
    pub checked_in_at: Option<String>,
    pub checked_in_by: Option<String>,
    pub qr_code_url: Option<String>,
    /// Claim token for NFT/refund claim link (set after check-in).
    pub claim_token: Option<String>,
    pub participation_type: String,
    pub row_index: usize,
}

impl AttendeeResponse {
    pub fn from_attendee(attendee: &crate::models::attendee::Attendee) -> Self {
        Self {
            api_id: attendee.api_id.clone(),
            name: attendee.display_name().to_string(),
            email: attendee.email.clone(),
            ticket_name: attendee.ticket_name.clone(),
            approval_status: attendee.approval_status.to_string(),
            checked_in_at: attendee.checked_in_at.clone(),
            checked_in_by: attendee.checked_in_by.clone(),
            qr_code_url: attendee.qr_code_url.clone(),
            claim_token: attendee.claim_token.clone(),
            participation_type: attendee.participation_type.clone(),
            row_index: attendee.row_index,
        }
    }
}

/// Response after checking in an attendee.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckInResponse {
    pub api_id: String,
    pub name: String,
    pub checked_in_at: String,
    pub checked_in_by: String,
    /// Claim token for NFT/refund claim link (UUID v7).
    /// Frontend constructs the full claim URL using `window.location.origin + /claim/{token}`.
    pub claim_token: Option<String>,
    pub message: String,
}

/// Response after bulk generating QR codes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateQrResponse {
    pub total: usize,
    pub generated: usize,
    pub skipped: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<QrGenerationDetail>,
}

/// Detail about a single QR code generation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrGenerationDetail {
    pub api_id: String,
    pub name: String,
    pub qr_code_url: String,
    pub status: QrGenerationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QrGenerationStatus {
    Generated,
    Skipped,
}

/// Check-in statistics for the admin dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResponse {
    pub total_approved: usize,
    pub total_checked_in: usize,
    pub total_remaining: usize,
    pub check_in_percentage: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_check_ins: Vec<RecentCheckIn>,
}

/// A recent check-in entry for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentCheckIn {
    pub api_id: String,
    pub name: String,
    pub checked_in_at: String,
    pub checked_in_by: Option<String>,
}

/// Response for GET /api/claim/{token} — look up an attendee by claim token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimLookupResponse {
    pub name: String,
    pub checked_in_at: String,
    pub claim_token: String,
    pub claimed: bool,
    pub claimed_at: Option<String>,
}

/// Response for POST /api/claim/{token} — mint cNFT and mark as claimed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimResponse {
    pub name: String,
    pub asset_id: String,
    pub signature: String,
    pub wallet_address: String,
    pub claimed_at: String,
}
