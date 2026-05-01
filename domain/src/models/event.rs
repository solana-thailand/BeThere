//! Event management types for multi-event / organizer support (Issue 004).
//!
//! Events are stored in Cloudflare KV under the EVENTS namespace:
//!   "events"                    → EventIndex (list of EventMeta summaries)
//!   "event:{id}"                → EventConfig (full per-event configuration)
//!   "event:{id}:quiz:questions" → QuizConfig (per-event quiz)
//!   "event:{id}:quiz:progress:{token}" → QuizProgress (per-event quiz progress)

use serde::{Deserialize, Serialize};

/// Lifecycle status of an event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum EventStatus {
    /// Event is being configured, not yet visible to attendees.
    #[default]
    Draft,
    /// Event is live — attendees can check in, claim, etc.
    Active,
    /// Event has ended — attendance frozen, claims still possible.
    Completed,
    /// Event is soft-deleted / hidden from listings.
    Archived,
}

impl EventStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Archived => "archived",
        }
    }
}

/// Lightweight event metadata stored in the EventIndex list.
/// Used for event listings / selectors without loading full config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMeta {
    /// Unique event identifier (e.g. "solana-bangkok-2025").
    pub id: String,
    /// Display name.
    pub name: String,
    /// URL-friendly slug (e.g. "solana-bangkok-2025").
    pub slug: String,
    /// Current lifecycle status.
    pub status: EventStatus,
    /// Event start time as Unix epoch milliseconds.
    pub event_start_ms: i64,
    /// Event end time as Unix epoch milliseconds.
    pub event_end_ms: i64,
    /// Google Sheets spreadsheet ID for attendee data.
    pub sheet_id: String,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// Emails of users with organizer-level access to this event.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub organizer_emails: Vec<String>,
}

/// Top-level index of all events, stored under KV key "events".
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventIndex {
    /// All known events (including draft/archived).
    #[serde(default)]
    pub events: Vec<EventMeta>,
}

/// Full per-event configuration, stored under KV key "event:{id}".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventConfig {
    // ── Identity ──────────────────────────────────────────────────────
    /// Unique event identifier.
    pub id: String,
    /// Display name (e.g. "Solana x AI Builders: The Road to Mainnet #1 (Bangkok)").
    pub name: String,
    /// URL-friendly slug (e.g. "solana-bangkok-2025").
    pub slug: String,
    /// Event tagline / subtitle.
    pub tagline: String,
    /// External event page URL.
    pub link: String,
    /// Current lifecycle status.
    pub status: EventStatus,

    // ── Schedule ──────────────────────────────────────────────────────
    /// Event start time as Unix epoch milliseconds.
    pub event_start_ms: i64,
    /// Event end time as Unix epoch milliseconds.
    pub event_end_ms: i64,

    // ── Google Sheets ─────────────────────────────────────────────────
    /// Google Sheets spreadsheet ID (contains attendee + staff tabs).
    pub sheet_id: String,
    /// Tab name for attendee data (e.g. "checkin").
    pub sheet_name: String,
    /// Tab name for staff allowlist (e.g. "staff").
    pub staff_sheet_name: String,

    // ── Quiz settings ─────────────────────────────────────────────────
    /// Whether quiz-gated claiming is enabled for this event.
    #[serde(default)]
    pub quiz_enabled: bool,

    // ── NFT / claim settings ──────────────────────────────────────────
    /// Solana collection mint address for compressed NFTs.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub nft_collection_mint: String,
    /// URI to metadata JSON on Arweave/IPFS.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub nft_metadata_uri: String,
    /// NFT badge image URL.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub nft_image_url: String,
    /// NFT name template (e.g. "BeThere - {event_name}").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub nft_name_template: String,
    /// NFT symbol (e.g. "BETH").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub nft_symbol: String,
    /// NFT description template.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub nft_description_template: String,
    /// Solana Merkle tree address for compressed NFT minting.
    /// When set, the worker mints to this tree via Helius RPC.
    /// When empty, Helius uses its own default tree.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub merkle_tree: String,

    // ── Access control ────────────────────────────────────────────────
    /// Emails with organizer-level access (full event management).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub organizer_emails: Vec<String>,
    /// Emails with staff-level access (scanner only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub staff_emails: Vec<String>,

    // ── Claim ─────────────────────────────────────────────────────────
    /// Base URL for claim links (e.g. "https://bethere.solana-thailand.workers.dev/claim").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub claim_base_url: String,

    // ── Timestamps ────────────────────────────────────────────────────
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// ISO 8601 last-update timestamp.
    pub updated_at: String,
}

impl EventConfig {
    /// Build a lightweight `EventMeta` summary from this config.
    pub fn to_meta(&self) -> EventMeta {
        EventMeta {
            id: self.id.clone(),
            name: self.name.clone(),
            slug: self.slug.clone(),
            status: self.status.clone(),
            event_start_ms: self.event_start_ms,
            event_end_ms: self.event_end_ms,
            sheet_id: self.sheet_id.clone(),
            created_at: self.created_at.clone(),
            organizer_emails: self.organizer_emails.clone(),
        }
    }

    /// Resolve the NFT name, expanding `{event_name}` placeholder.
    pub fn nft_name(&self) -> String {
        if self.nft_name_template.is_empty() {
            format!("BeThere - {}", self.name)
        } else {
            self.nft_name_template.replace("{event_name}", &self.name)
        }
    }

    /// Resolve the NFT description, expanding `{event_name}` placeholder.
    pub fn nft_description(&self) -> String {
        if self.nft_description_template.is_empty() {
            format!("Proof of attendance at {}", self.name)
        } else {
            self.nft_description_template
                .replace("{event_name}", &self.name)
        }
    }

    /// Create an EventConfig from the global AppConfig (legacy fallback).
    ///
    /// Used when EVENTS KV is not configured — builds a synthetic event
    /// from the static env vars so handlers can use the same EventConfig
    /// interface regardless of whether multi-event is enabled.
    #[allow(clippy::too_many_arguments)]
    pub fn from_global_config(
        name: &str,
        tagline: &str,
        link: &str,
        event_start_ms: i64,
        event_end_ms: i64,
        sheet_id: &str,
        sheet_name: &str,
        staff_sheet_name: &str,
        nft_collection_mint: &str,
        nft_metadata_uri: &str,
        nft_image_url: &str,
        nft_symbol: &str,
        organizer_emails: Vec<String>,
        staff_emails: Vec<String>,
        claim_base_url: &str,
        merkle_tree: &str,
    ) -> Self {
        Self {
            id: "default".to_string(),
            name: name.to_string(),
            slug: "default".to_string(),
            tagline: tagline.to_string(),
            link: link.to_string(),
            status: EventStatus::Active,
            event_start_ms,
            event_end_ms,
            sheet_id: sheet_id.to_string(),
            sheet_name: sheet_name.to_string(),
            staff_sheet_name: staff_sheet_name.to_string(),
            quiz_enabled: true,
            nft_collection_mint: nft_collection_mint.to_string(),
            nft_metadata_uri: nft_metadata_uri.to_string(),
            nft_image_url: nft_image_url.to_string(),
            nft_name_template: String::new(),
            nft_symbol: nft_symbol.to_string(),
            nft_description_template: String::new(),
            merkle_tree: merkle_tree.to_string(),
            organizer_emails,
            staff_emails,
            claim_base_url: claim_base_url.to_string(),
            merkle_tree: merkle_tree.to_string(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// API request / response types
// ---------------------------------------------------------------------------

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
    /// Google Sheets spreadsheet ID (required).
    pub sheet_id: String,
    /// Tab name for attendee data (defaults to "checkin").
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
    /// Organizer email addresses.
    #[serde(default)]
    pub organizer_emails: Vec<String>,
    /// Staff email addresses.
    #[serde(default)]
    pub staff_emails: Vec<String>,
    /// Base URL for claim links.
    #[serde(default)]
    pub claim_base_url: String,
}

/// Request body for PUT /api/events/{id} — update an existing event.
/// All fields are optional; only provided fields are updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Replace organizer emails.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organizer_emails: Option<Vec<String>>,
    /// Replace staff emails.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staff_emails: Option<Vec<String>>,
    /// New claim base URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_base_url: Option<String>,
}

/// Response for GET /api/events — list all events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventListResponse {
    pub events: Vec<EventMeta>,
}

/// Response for GET /api/events/{id} — single event details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDetailResponse {
    pub event: EventConfig,
}

/// Response for POST /api/events — event creation confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEventResponse {
    pub id: String,
    pub name: String,
    pub slug: String,
}

/// Response for PUT /api/events/{id} — event update confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEventResponse {
    pub id: String,
    pub updated_at: String,
}
