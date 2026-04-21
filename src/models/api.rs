use serde::{Deserialize, Serialize};

/// Generic API response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[allow(dead_code)]
impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

/// Response for a single attendee or attendee in a list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttendeeResponse {
    pub api_id: String,
    pub name: String,
    pub email: String,
    pub ticket_name: String,
    pub approval_status: String,
    pub checked_in_at: Option<String>,
    pub qr_code_url: Option<String>,
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
            qr_code_url: attendee.qr_code_url.clone(),
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
}
