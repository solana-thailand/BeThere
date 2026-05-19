//! Event CRUD, escrow init, and event-related types.

use serde::{Deserialize, Serialize};

use super::types::{ApiError, ApiResponse};
use super::{api_delete, api_get, api_post, api_post_json, api_put_json};

// ===== Event Management Types =====

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
    Cancelled,
}

impl EscrowStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Initialized => "initialized",
            Self::Deactivated => "deactivated",
            Self::Closed => "closed",
            Self::Cancelled => "cancelled",
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

/// Controls when online registration opens for hybrid events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum OnlineOpenMode {
    #[default]
    Always,
    AutoOnFull,
    Manual,
}

impl OnlineOpenMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Always => "Always Open",
            Self::AutoOnFull => "Auto (when in-person full)",
            Self::Manual => "Manual Toggle",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::AutoOnFull => "auto_on_full",
            Self::Manual => "manual",
        }
    }
}

/// Default true helper for serde.
fn default_true_fn() -> bool {
    true
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
    pub time_tba: bool,
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
    // Capacity
    #[serde(default)]
    pub in_person_capacity: Option<u32>,
    #[serde(default)]
    pub online_capacity: Option<u32>,
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
    pub time_tba: bool,
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
    pub location: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    // Capacity settings
    #[serde(default)]
    pub in_person_capacity: Option<u32>,
    #[serde(default)]
    pub online_capacity: Option<u32>,
    #[serde(default)]
    pub online_open_mode: OnlineOpenMode,
    #[serde(default)]
    pub online_registration_open: bool,
    #[serde(default)]
    pub deposit_deadline_hours: Option<u32>,
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
    pub time_tba: bool,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    // Capacity settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_person_capacity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_capacity: Option<u32>,
    #[serde(default)]
    pub online_open_mode: OnlineOpenMode,
    #[serde(default)]
    pub online_registration_open: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_deadline_hours: Option<u32>,
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
    pub time_tba: Option<bool>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    // Capacity settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_person_capacity: Option<Option<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_capacity: Option<Option<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_open_mode: Option<OnlineOpenMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_registration_open: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_deadline_hours: Option<Option<u32>>,
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
