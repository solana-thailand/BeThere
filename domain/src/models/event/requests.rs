//! API request bodies for the event endpoints.

use serde::{Deserialize, Serialize};

use super::config::CommunityLink;
use super::defaults::default_true;
use super::enums::{EscrowStatus, EventFormat, EventStatus, EventVisibility, OnlineOpenMode};

/// Request body for POST /api/events — create a new event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEventRequest {
    /// Display name (required).
    pub name: String,
    /// URL-friendly slug (required, auto-generated from name if empty).
    #[serde(default)]
    pub slug: String,
    /// Event tagline.
    #[serde(default)]
    pub tagline: String,
    /// External event page URL.
    #[serde(default)]
    pub link: String,
    /// Event start time as Unix epoch milliseconds (required).
    pub event_start_ms: i64,
    /// Event end time as Unix epoch milliseconds (required).
    pub event_end_ms: i64,
    /// Mark event time as TBA. When true, event_start_ms/end_ms are treated as date-only.
    #[serde(default)]
    pub time_tba: bool,
    /// Google Sheets spreadsheet ID (required).
    pub sheet_id: String,
    /// Tab name for attendee data (defaults to "Attendees").
    #[serde(default)]
    pub sheet_name: String,
    /// Tab name for staff allowlist (defaults to "staff").
    #[serde(default)]
    pub staff_sheet_name: String,
    /// Whether quiz is enabled (defaults to false).
    #[serde(default)]
    pub quiz_enabled: bool,
    /// NFT collection mint address.
    #[serde(default)]
    pub nft_collection_mint: String,
    /// NFT metadata URI.
    #[serde(default)]
    pub nft_metadata_uri: String,
    /// NFT badge image URL.
    #[serde(default)]
    pub nft_image_url: String,
    /// Marketing poster URL for the event page hero.
    /// Empty = fall back to `nft_image_url` on the public event page.
    #[serde(default)]
    pub poster_url: String,
    /// NFT name template (supports `{event_name}` placeholder).
    #[serde(default)]
    pub nft_name_template: String,
    /// NFT symbol.
    #[serde(default)]
    pub nft_symbol: String,
    /// NFT description template (supports `{event_name}` placeholder).
    #[serde(default)]
    pub nft_description_template: String,
    /// Merkle tree address for cNFT minting.
    #[serde(default)]
    pub merkle_tree: String,
    /// Organization this event belongs to. Empty = global (no org).
    #[serde(default)]
    pub organization_id: String,
    /// Organizer email addresses.
    #[serde(default)]
    pub organizer_emails: Vec<String>,
    /// Staff email addresses.
    #[serde(default)]
    pub staff_emails: Vec<String>,
    /// Base URL for claim links.
    #[serde(default)]
    pub claim_base_url: String,

    // ── Deposit settings ──────────────────────────────────────────────
    /// Whether deposit is required for this event.
    #[serde(default)]
    pub deposit_enabled: bool,
    /// Deposit amount in USDC smallest unit (6 decimals).
    #[serde(default)]
    pub deposit_amount_usdc: u64,
    /// Deposit amount in Thai Baht (for PromptPay track).
    #[serde(default)]
    pub deposit_amount_thb: u64,
    /// PromptPay ID for THB payments (Thai phone number or national ID).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub promptpay_id: String,
    /// EventEscrow PDA address.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub escrow_address: String,
    /// Organizer's Solana wallet address (base58).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub organizer_wallet: String,
    /// On-chain event ID (u64) for PDA seed derivation. 0 = auto-derive.
    #[serde(default)]
    pub on_chain_event_id: u64,
    /// Hours after event_end for refund deadline (default: 168 = 7 days).
    #[serde(default)]
    pub refund_deadline_hours: u32,
    /// Maximum number of refundable deposits (0 = unlimited).
    #[serde(default)]
    pub max_refundable_deposits: u32,
    /// Event description (markdown or plain text).
    #[serde(default)]
    pub description: String,
    /// Event location (venue name, address, or "Online").
    #[serde(default)]
    pub location: String,
    /// YouTube/live stream/recording URL.
    #[serde(default)]
    pub video_url: String,
    /// Event format — In-Person, Online, or Hybrid.
    #[serde(default)]
    pub event_format: EventFormat,
    /// Whether contact info is required during self-registration (defaults to true).
    #[serde(default = "default_true")]
    pub require_contact_info: bool,
    /// Whether photo/media consent is collected during registration (PDPA).
    #[serde(default)]
    pub require_photo_consent: bool,

    // ── Capacity settings ─────────────────────────────────────────────
    /// Maximum number of in-person attendees. None = unlimited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_person_capacity: Option<u32>,
    /// Maximum number of online attendees. None = unlimited (default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online_capacity: Option<u32>,
    /// Controls when online registration opens for hybrid events.
    #[serde(default)]
    pub online_open_mode: OnlineOpenMode,
    /// Manual toggle for online registration (used when `online_open_mode = Manual`).
    #[serde(default)]
    pub online_registration_open: bool,
    /// Hours after registration to auto-switch from in-person to online track.
    /// None = no deadline (in-person spot held held indefinitely).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deposit_deadline_hours: Option<u32>,
    /// Event visibility — public (shown on landing) or private (auth required).
    #[serde(default)]
    pub visibility: EventVisibility,
    /// Community/social links.
    #[serde(default)]
    pub community_links: Vec<CommunityLink>,
    /// Organization calendar subscribe URL (Google Calendar embed URL).
    #[serde(default)]
    pub calendar_subscribe_url: String,
}

/// Request body for POST /api/events/{id}/duplicate — copy event settings
/// into a new Draft. All fields optional; defaults to copying source verbatim
/// with de-collided slug and "(Copy)" name suffix.
///
/// See `.issues/055_duplicate_event.md` for design (Decisions A1 + B1).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DuplicateEventRequest {
    /// Override the source's sheet_id. If empty, source's sheet_id is reused
    /// (with a UI warning) because sheet_id is a required field downstream.
    #[serde(default)]
    pub new_sheet_id: String,
    /// Override the auto-generated "{name} (Copy)" display name.
    #[serde(default)]
    pub new_name: String,
}

/// Request body for PUT /api/events/{id} — update an existing event.
/// All fields are optional; only provided fields are updated.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateEventRequest {
    /// New display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// New slug.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// New tagline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagline: Option<String>,
    /// New external link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// New status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<EventStatus>,
    /// New start time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_start_ms: Option<i64>,
    /// New end time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_end_ms: Option<i64>,
    /// Update TBA status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_tba: Option<bool>,
    /// New Google Sheets spreadsheet ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet_id: Option<String>,
    /// New attendee tab name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet_name: Option<String>,
    /// New staff tab name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staff_sheet_name: Option<String>,
    /// Toggle quiz feature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quiz_enabled: Option<bool>,
    /// New NFT collection mint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nft_collection_mint: Option<String>,
    /// New NFT metadata URI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nft_metadata_uri: Option<String>,
    /// New NFT image URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nft_image_url: Option<String>,
    /// New marketing poster URL (served path or external URL).
    /// Empty string clears the field, falling the hero back to `nft_image_url`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_url: Option<String>,
    /// New NFT name template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nft_name_template: Option<String>,
    /// New NFT symbol.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nft_symbol: Option<String>,
    /// New NFT description template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nft_description_template: Option<String>,
    /// New Merkle tree address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merkle_tree: Option<String>,
    /// New organization ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
    /// Replace organizer emails.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organizer_emails: Option<Vec<String>>,
    /// Replace staff emails.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staff_emails: Option<Vec<String>>,
    /// New claim base URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_base_url: Option<String>,

    // ── Deposit settings ──────────────────────────────────────────────
    /// Whether deposit is required for this event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_enabled: Option<bool>,
    /// Deposit amount in USDC smallest unit (6 decimals).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_amount_usdc: Option<u64>,
    /// Deposit amount in Thai Baht (for PromptPay track).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_amount_thb: Option<u64>,
    /// PromptPay ID for THB payments (Thai phone number or national ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promptpay_id: Option<String>,
    /// EventEscrow PDA address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escrow_address: Option<String>,
    /// On-chain escrow lifecycle status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escrow_status: Option<EscrowStatus>,
    /// Organizer's Solana wallet address (base58).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organizer_wallet: Option<String>,
    /// On-chain event ID (u64) for PDA seed derivation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_chain_event_id: Option<u64>,
    /// Hours after event_end for refund deadline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_deadline_hours: Option<u32>,
    /// Maximum number of refundable deposits (0 = unlimited).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_refundable_deposits: Option<u32>,
    /// Optimistic concurrency: if provided, update only succeeds when this
    /// matches the stored `updated_at` timestamp. Prevents blind overwrites.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_updated_at: Option<String>,
    /// New event description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// New event location.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// YouTube/live stream/recording URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_url: Option<String>,
    /// New event format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_format: Option<EventFormat>,
    /// Whether contact info is required during self-registration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_contact_info: Option<bool>,
    /// Whether photo/media consent is collected during registration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_photo_consent: Option<bool>,

    // ── Capacity settings ─────────────────────────────────────────────
    /// Maximum number of in-person attendees. None = unlimited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_person_capacity: Option<Option<u32>>,
    /// Maximum number of online attendees. None = unlimited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_capacity: Option<Option<u32>>,
    /// Controls when online registration opens for hybrid events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_open_mode: Option<OnlineOpenMode>,
    /// Manual toggle for online registration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_registration_open: Option<bool>,
    /// Hours after registration to auto-switch from in-person to online.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_deadline_hours: Option<Option<u32>>,
    /// New event visibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<EventVisibility>,
    /// Whether developer profile fields are shown on the registration form.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_profile_enabled: Option<bool>,
    /// Community/social links. Replaces all existing links.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub community_links: Option<Vec<CommunityLink>>,
    /// Organization calendar subscribe URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_subscribe_url: Option<String>,
}
