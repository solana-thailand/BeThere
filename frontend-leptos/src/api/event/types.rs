//! Event metadata, detail and request/response body types.

use serde::{Deserialize, Serialize};

use super::enums::{
    EscrowStatus, EventFormat, EventStatus, EventVisibility, OnlineOpenMode, default_true_fn,
};

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
    /// Long-form public description shown in "About this Event".
    #[serde(default)]
    pub description: String,
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
    pub community_links: Vec<crate::api::types::CommunityLink>,
    /// Google Calendar embed URL for the event.
    #[serde(default)]
    pub calendar_subscribe_url: String,
}

/// Response for GET /api/events — list all events.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct EventsListData {
    #[serde(default)]
    pub events: Vec<EventMeta>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct QuizReadinessProblem {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub event_name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub blocked_attendees: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CoreReadinessData {
    #[serde(default)]
    pub quiz_problems: Vec<QuizReadinessProblem>,
    #[serde(default)]
    pub blocked_attendees: u64,
}

/// Response for GET /api/events/{id} — single event detail.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct EventDetailData {
    pub event: EventDetail,
}

/// Request body for POST /api/events — create event.
#[derive(Debug, Clone, Serialize, Default)]
pub struct CreateEventBody {
    /// Long-form public description. Line breaks are preserved on the public
    /// page (`white-space: pre-line`), so an agenda pastes in readably.
    #[serde(default)]
    pub description: String,
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
    pub community_links: Vec<crate::api::types::CommunityLink>,
    /// Google Calendar embed URL for the event.
    #[serde(default)]
    pub calendar_subscribe_url: String,
}

/// Request body for PUT /api/events/{id} — update event.
/// All fields optional for partial update.
#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateEventBody {
    /// New long-form public description. `None` leaves it untouched.
    #[serde(default)]
    pub description: Option<String>,
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
    pub community_links: Option<Vec<crate::api::types::CommunityLink>>,
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
