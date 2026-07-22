//! Event CRUD, escrow init, and event-related types.

use serde::{Deserialize, Serialize};

use super::types::{ApiError, ApiResponse};
use super::{api_delete, api_get, api_post, api_post_blob, api_post_json, api_put_json, fetch::response_json};

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
#[serde(rename_all = "snake_case")]
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

/// Event visibility (mirrors backend EventVisibility).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventVisibility {
    #[default]
    Public,
    Private,
}

impl EventVisibility {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Public => "Public",
            Self::Private => "Private",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

/// Default true helper for serde.
fn default_true_fn() -> bool {
    true
}

/// Lightweight event metadata from the events list endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
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
    #[serde(default)]
    pub visibility: EventVisibility,
    #[serde(default)]
    pub video_url: String,
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
    /// Marketing poster URL for the event page hero (served path or external URL).
    #[serde(default)]
    pub poster_url: String,
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
    pub require_photo_consent: bool,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub video_url: String,
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
    #[serde(default)]
    pub visibility: EventVisibility,
    /// Community/social links for the event.
    #[serde(default)]
    pub community_links: Vec<super::types::CommunityLink>,
    /// Google Calendar embed URL for the event.
    #[serde(default)]
    pub calendar_subscribe_url: String,
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
    /// Marketing poster URL for the event page hero.
    #[serde(default)]
    pub poster_url: String,
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
    #[serde(default)]
    pub require_photo_consent: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default)]
    pub video_url: String,
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
    #[serde(default)]
    pub visibility: EventVisibility,
    /// Community/social links for the event.
    #[serde(default)]
    pub community_links: Vec<super::types::CommunityLink>,
    /// Google Calendar embed URL for the event.
    #[serde(default)]
    pub calendar_subscribe_url: String,
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
    /// Marketing poster URL (served path or external URL). Empty string clears it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_url: Option<String>,
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
    pub require_photo_consent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_url: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<EventVisibility>,
    /// Community/social links for the event. Replaces all existing links.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub community_links: Option<Vec<super::types::CommunityLink>>,
    /// Google Calendar embed URL for the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_subscribe_url: Option<String>,
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
    let result: ApiResponse<EventsListData> = response_json(&response).await.map_err(|e| ApiError {
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
    let result: ApiResponse<EventDetailData> = response_json(&response).await.map_err(|e| ApiError {
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
            message: body.error.unwrap_or_else(|| format!("HTTP {}", response.status())),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<PosterMutationData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse poster response: {e}"),
            status: response.status(),
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or_else(|| "No data in response".to_string()),
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
            message: body.error.unwrap_or_else(|| format!("HTTP {}", response.status())),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<PosterMutationData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse poster response: {e}"),
            status: response.status(),
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or_else(|| "No data in response".to_string()),
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

// ===== Event Summary (Plan 008 Phase 1) =====

/// Top-level wrapper returned by `/events/{id}/summary` and
/// `/events/{id}/summary/freeze`. The `frozen` flag tells the UI whether the
/// snapshot is permanent (`true`) or a live preview recomputed on each request
/// (`false`). All inner fields are `#[serde(default)]` so a partial backend
/// response degrades gracefully instead of erroring the whole page.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct EventSummaryData {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub frozen: bool,
    #[serde(default)]
    pub summary: EventSummaryPayload,
}

/// Frozen-or-live snapshot of the funnel + financials for one event.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct EventSummaryPayload {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub event_start_ms: i64,
    #[serde(default)]
    pub event_end_ms: i64,
    /// ISO 8601 timestamp the snapshot was frozen at. `None` while the
    /// event is still live (`frozen == false`) — the numbers shown are then
    /// a preview recomputed on each request.
    #[serde(default)]
    pub frozen_at: Option<String>,
    #[serde(default)]
    pub frozen_by: String,
    #[serde(default)]
    pub funnel: FunnelSnapshotData,
    #[serde(default)]
    pub financials: FinancialSnapshotData,
}

/// Funnel counts for the post-event summary. Mirrors the backend
/// `FunnelSnapshot`; every field is `u64` because D1 stores these as
/// `INTEGER` aggregates that never go negative.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct FunnelSnapshotData {
    #[serde(default)]
    pub registered_count: u64,
    #[serde(default)]
    pub deposited_count: u64,
    #[serde(default)]
    pub checked_in_count: u64,
    #[serde(default)]
    pub no_show_count: u64,
    #[serde(default)]
    pub claimed_count: u64,
    #[serde(default)]
    pub refunded_count: u64,
    #[serde(default)]
    pub post_event_reg_count: u64,
    /// In-person registrants — denominator for `no_show_count`.
    #[serde(default)]
    pub in_person_registered_count: u64,
    /// In-person registrants who checked in.
    #[serde(default)]
    pub in_person_checked_in_count: u64,
}

/// Financial totals for the post-event summary. USDC amounts are atomic
/// (1 USDC = 1_000_000); THB amounts are satang (1 THB = 100).
///
/// NOTE: `usdc_refunded_total` is always 0 in v1 — the backend doesn't yet
/// sum USDC refunds. The UI surfaces this honestly rather than pretending
/// the figure is authoritative.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct FinancialSnapshotData {
    #[serde(default)]
    pub usdc_deposited_total: u64,
    #[serde(default)]
    pub usdc_refunded_total: u64,
    #[serde(default)]
    pub thb_deposited_total: u64,
    #[serde(default)]
    pub thb_refunded_total: u64,
}

/// GET /api/events/{id}/summary — fetch the post-event summary snapshot.
///
/// Returns a frozen snapshot if one exists, otherwise a live preview computed
/// from current D1 state (`frozen == false`). Requires a staff JWT
/// (organizer+ role is enforced server-side).
pub async fn get_event_summary(id: &str) -> Result<EventSummaryData, ApiError> {
    let path = format!("/events/{id}/summary");
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to load summary".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<EventSummaryData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse summary response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/events/{id}/summary/freeze — permanently freeze the summary.
///
/// Returns 400 if the event has not ended yet or is still Draft, 403 if the
/// caller is Staff (not organizer+), 404 if the event doesn't exist. On
/// success the returned snapshot has `frozen == true` and `frozen_at` set;
/// subsequent refunds will not change these numbers.
pub async fn freeze_event_summary(id: &str) -> Result<EventSummaryData, ApiError> {
    let path = format!("/events/{id}/summary/freeze");
    let response = api_post(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Freeze failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<EventSummaryData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse freeze response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

// ===== Event Recap (Plan 008 Phase 2) =====

/// Recap content payload returned by `GET /api/events/{id}/recap` (organizer).
///
/// `summary_frozen` tells the UI whether a frozen `event_summaries` row exists.
/// When `false`, the organizer must freeze the summary before authoring a recap
/// (the backend refuses to publish without a freeze).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EventRecapData {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub summary_frozen: bool,
    #[serde(default)]
    pub recap: EventRecapPayload,
}

/// Recap slice of the `event_summaries` row. Mirrors the backend
/// `EventRecap` domain type; all fields `#[serde(default)]` so a draft
/// (empty markdown, no image, no published_at) deserializes cleanly.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EventRecapPayload {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub recap_markdown: String,
    #[serde(default)]
    pub recap_image_url: String,
    #[serde(default)]
    pub recap_published_at: Option<String>,
    #[serde(default)]
    pub frozen_at: Option<String>,
}

/// Request body for `PUT /api/events/{id}/recap`.
///
/// `publish: true` sets `recap_published_at = now` and mirrors
/// `recap_published = 1` onto the events row (visible on `/past-events`).
/// `publish: false` saves as draft (unpublishes a live recap if one exists).
#[derive(Debug, Clone, Serialize)]
pub struct PutRecapRequest {
    pub recap_markdown: String,
    pub recap_image_url: String,
    pub publish: bool,
}

/// `GET /api/events/{id}/recap` — fetch the current draft/published recap.
pub async fn get_event_recap(id: &str) -> Result<EventRecapData, ApiError> {
    let path = format!("/events/{id}/recap");
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to load recap".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<EventRecapData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse recap response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// `PUT /api/events/{id}/recap` — author + publish/unpublish the recap.
///
/// Returns the freshly-persisted recap state. The backend rejects publishing
/// when no frozen summary exists (409-style Validation error surfaced as
/// `ApiError::message`); the caller should render that message to the organizer.
pub async fn put_event_recap(
    id: &str,
    markdown: &str,
    image_url: &str,
    publish: bool,
) -> Result<EventRecapData, ApiError> {
    let path = format!("/events/{id}/recap");
    let body = PutRecapRequest {
        recap_markdown: markdown.to_string(),
        recap_image_url: image_url.to_string(),
        publish,
    };
    api_put_json::<EventRecapData>(&path, &body).await
}

/// One entry in the public past-events feed (`GET /api/public/events/past`).
///
/// Mirrors the sanitized payload from `list_past_events_raw`. Same shape as a
/// public event card, plus `poster_url` (Phase 2 cards prefer the marketing
/// poster over the NFT badge image).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PastEventItem {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub event_start_ms: i64,
    #[serde(default)]
    pub event_end_ms: i64,
    #[serde(default)]
    pub time_tba: bool,
    #[serde(default)]
    pub event_format: String,
    #[serde(default)]
    pub tagline: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub nft_image_url: String,
    #[serde(default)]
    pub poster_url: String,
    #[serde(default)]
    pub created_at: String,
}

/// Wrapper for the past-events feed response.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PastEventsResponse {
    #[serde(default)]
    pub events: Vec<PastEventItem>,
}

/// `GET /api/public/events/past` — list completed events with a published recap.
///
/// Public endpoint (no auth required). Cached 60s server-side.
pub async fn list_past_events() -> Result<PastEventsResponse, ApiError> {
    let response = api_get("/public/events/past").await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Failed to load past events".to_string(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<PastEventsResponse> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse past events response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// Headline funnel counts surfaced on the public recap page. Sensitive
/// financials (refunded totals, no-show) are intentionally excluded by the
/// backend — public recaps celebrate attendance, not accounting.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PublicRecapFunnel {
    #[serde(default)]
    pub registered_count: u64,
    #[serde(default)]
    pub deposited_count: u64,
    #[serde(default)]
    pub checked_in_count: u64,
    #[serde(default)]
    pub claimed_count: u64,
}

/// Sanitized event meta embedded in the public recap response.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PublicRecapEvent {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub tagline: String,
    #[serde(default)]
    pub event_start_ms: i64,
    #[serde(default)]
    pub event_end_ms: i64,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub event_format: String,
    #[serde(default)]
    pub poster_url: String,
    #[serde(default)]
    pub nft_image_url: String,
    /// Whether post-event registration (lead capture) is open (Plan 008 — Phase 3).
    /// Drives the "join the community" CTA on the recap page.
    #[serde(default)]
    pub post_event_registration_open: bool,
}

/// Public recap payload returned by `GET /api/public/event/{slug}/recap`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PublicRecapData {
    #[serde(default)]
    pub event: PublicRecapEvent,
    #[serde(default)]
    pub recap_markdown: String,
    #[serde(default)]
    pub recap_image_url: String,
    #[serde(default)]
    pub recap_published_at: Option<String>,
    #[serde(default)]
    pub frozen_at: Option<String>,
    #[serde(default)]
    pub funnel: PublicRecapFunnel,
}

/// `GET /api/public/event/{slug}/recap` — public recap for a completed event.
///
/// Returns 404 when the event isn't found, isn't `Completed`, has no published
/// recap, or no frozen summary exists. The 404 is indistinguishable from "no
/// recap" by design (don't leak existence of unpublished drafts).
pub async fn get_public_recap(slug: &str) -> Result<PublicRecapData, ApiError> {
    let path = format!("/public/event/{slug}/recap");
    let response = api_get(&path).await?;

    if !response.ok() {
        return Err(ApiError {
            message: if response.status() == 404 {
                "No public recap for this event".to_string()
            } else {
                "Failed to load recap".to_string()
            },
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<PublicRecapData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse public recap response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

// ===== PR Pack (Plan 008 Phase 4) =====

/// Generated marketing copy for one event. Mirrors `domain::pr_pack::PrPack`.
/// All fields are `String` (or `Vec<String>` for organizers) so they drop
/// straight into copy-to-clipboard cards.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PrPack {
    pub headline: String,
    pub short_blurb: String,
    pub social_post: String,
    pub calendar_text: String,
    pub email_snippet: String,
    pub deposit_terms: String,
    #[serde(default)]
    pub organizers: Vec<String>,
}

/// Wrapper returned by `/events/{id}/pr-pack`.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PrPackData {
    pub event_id: String,
    pub pack: PrPack,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub source_config_version: String,
}

/// GET /api/events/{id}/pr-pack — generate the PR pack (deterministic).
///
/// Organizer-gated server-side. Returns 403 for Staff, 404 for unknown events.
pub async fn get_pr_pack(id: &str) -> Result<PrPackData, ApiError> {
    let path = format!("/events/{id}/pr-pack");
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to load PR pack".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<PrPackData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse PR pack response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

// ===== Post-Event Registration (Plan 008 — Phase 3) =====

/// Body for `POST /api/public/event/{slug}/register-post-event` — the stripped
/// lead-capture form (no deposit/participation fields).
#[derive(Debug, Clone, Serialize, Default)]
pub struct PostEventRegisterBody {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_handle: Option<String>,
    pub consent_given: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consent_marketing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experience_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech_stack: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interests: Option<String>,
}

/// Success response from post-event registration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PostEventRegisterData {
    pub attendee_id: String,
    pub message: String,
}

/// `POST /api/public/event/{slug}/register-post-event` — submit the lead-capture
/// form. JWT-gated (verified email) on the backend.
pub async fn register_post_event(
    slug: &str,
    body: &PostEventRegisterBody,
) -> Result<PostEventRegisterData, ApiError> {
    let path = format!("/public/event/{slug}/register-post-event");
    api_post_json(&path, body).await
}

/// Body for `PUT /api/events/{id}/post-event-registration` — organizer toggle.
#[derive(Debug, Clone, Serialize)]
pub struct PutPostEventRegistrationBody {
    pub open: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until_ms: Option<i64>,
}

/// Response from the toggle endpoint.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PostEventRegistrationState {
    pub open: bool,
    #[serde(default)]
    pub until_ms: Option<i64>,
}

/// `PUT /api/events/{id}/post-event-registration` — organizer opens/closes
/// post-event lead capture for a completed event.
pub async fn put_post_event_registration(
    id: &str,
    body: &PutPostEventRegistrationBody,
) -> Result<PostEventRegistrationState, ApiError> {
    let path = format!("/events/{id}/post-event-registration");
    api_put_json(&path, body).await
}
