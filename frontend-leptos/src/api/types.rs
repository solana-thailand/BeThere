//! Core response types shared across all API domain modules.

use serde::{Deserialize, Serialize};

/// Helper for serde `#[serde(default = "default_true")]` — defaults to `true`.
pub(crate) const fn default_true() -> bool {
    true
}

// ===== Typed Enums =====

/// Check-in / approval status for an attendee.
/// Mirrors `domain::models::attendee::CheckInStatus`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CheckInStatus {
    #[default]
    PendingApproval,
    Approved,
    Invited,
    CheckedIn,
}

impl CheckInStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PendingApproval => "pending_approval",
            Self::Approved => "approved",
            Self::Invited => "invited",
            Self::CheckedIn => "checked_in",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::PendingApproval => "Pending Approval",
            Self::Approved => "Approved",
            Self::Invited => "Invited",
            Self::CheckedIn => "Checked In",
        }
    }

    pub fn is_approved(&self) -> bool {
        matches!(self, Self::Approved | Self::CheckedIn)
    }
}

/// Deposit payment method.
/// Mirrors `domain::models::deposit::DepositMethod`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DepositMethod {
    #[default]
    Usdc,
    Thb,
    CreditThb,
    CreditUsdc,
}

impl DepositMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Usdc => "usdc",
            Self::Thb => "thb",
            Self::CreditThb => "credit_thb",
            Self::CreditUsdc => "credit_usdc",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Usdc => "USDC (Solana)",
            Self::Thb => "THB (PromptPay)",
            Self::CreditThb => "THB Credit (held deposit)",
            Self::CreditUsdc => "USDC Credit (held deposit)",
        }
    }

    pub fn icon_name(&self) -> &'static str {
        match self {
            Self::Usdc => "coin",
            Self::Thb => "baht",
            Self::CreditThb => "baht",
            Self::CreditUsdc => "coin",
        }
    }
}

/// QR code generation status.
/// Mirrors `domain::models::api::QrGenerationStatus`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum QrGenerationStatus {
    #[default]
    Generated,
    Skipped,
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
    pub approval_status: CheckInStatus,
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
    pub approval_status: CheckInStatus,
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

/// Eligible rollover target event info (returned when attendee has a verified USDC deposit on a past event).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RolloverTargetEvent {
    pub event_id: String,
    pub event_name: String,
    pub event_slug: String,
    pub deposit_amount_usdc: u64,
}

/// Deposit status info (optional — only for events with deposit enabled).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DepositInfo {
    pub method: DepositMethod,
    pub verified: bool,
    pub currency: String,
    /// Whether THB refund has been processed.
    #[serde(default)]
    pub refunded: bool,
    /// URL of the refund transfer receipt (organizer-uploaded proof).
    #[serde(default)]
    pub refund_proof_url: Option<String>,
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
    /// Deposit status info (present when event has deposit enabled).
    #[serde(default)]
    pub deposit_info: Option<DepositInfo>,
    /// Event end timestamp in ms — used for online claim countdown.
    #[serde(default)]
    pub event_end_ms: i64,
    /// Event name.
    #[serde(default)]
    pub event_name: String,
    /// Event start timestamp in ms.
    #[serde(default)]
    pub event_start_ms: i64,
    /// Whether the attendee has already claimed their NFT.
    #[serde(default)]
    pub claimed: bool,
    /// Claimed NFT asset ID (for explorer links).
    #[serde(default)]
    pub claimed_asset_id: Option<String>,
    /// Solana cluster for explorer links (e.g. "devnet", "mainnet-beta").
    #[serde(default)]
    pub cluster: Option<String>,
    /// Event format (in_person, online, hybrid).
    #[serde(default)]
    pub event_format: String,
    /// YouTube/livestream URL for the event.
    #[serde(default)]
    pub video_url: String,
    /// External event page URL (sessions, slides, etc.).
    #[serde(default)]
    pub event_link: String,
    /// Event location (venue name, address, or "Online").
    #[serde(default)]
    pub event_location: String,
    /// Event tagline / subtitle.
    #[serde(default)]
    pub event_tagline: String,
    /// NFT badge image URL.
    #[serde(default)]
    pub nft_image_url: String,
    /// Whether deposit is enabled for this event.
    #[serde(default)]
    pub deposit_enabled: bool,
    /// Deposit deadline in hours after registration.
    #[serde(default)]
    pub deposit_deadline_hours: Option<u32>,
    /// Deposit amount in USDC (smallest unit, e.g. 15000000 = 15 USDC).
    #[serde(default)]
    pub deposit_amount_usdc: u64,
    /// Deposit amount in THB.
    #[serde(default)]
    pub deposit_amount_thb: u64,
    /// Whether the deposit deadline has expired for this attendee.
    #[serde(default)]
    pub deadline_expired: bool,
    /// Whether in-person spots are still available (for reclaim flow).
    #[serde(default)]
    pub in_person_available: Option<bool>,
    /// Event slug for navigation.
    #[serde(default)]
    pub event_slug: String,
    /// Event ID (for rollover and other API calls).
    #[serde(default)]
    pub event_id: String,
    /// Eligible rollover target event (present when attendee has verified USDC deposit on a past event).
    #[serde(default)]
    pub rollover_target_event: Option<RolloverTargetEvent>,
    /// Refund link filled in by organizer (e.g., bank transfer receipt link).
    #[serde(default)]
    pub refund_link: Option<String>,
    /// Escrow status (none, initialized, deactivated, closed, cancelled).
    #[serde(default)]
    pub escrow_status: String,
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
    pub status: QrGenerationStatus,
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
