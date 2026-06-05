//! Event management types for multi-event / organizer support (Issue 004).
//!
//! Events are stored in Cloudflare KV under the EVENTS namespace:
//!   "events"                    → EventIndex (list of EventMeta summaries)
//!   "event:{id}"                → EventConfig (full per-event configuration)
//!   "event:{id}:quiz:questions" → QuizConfig (per-event quiz)
//!   "event:{id}:quiz:progress:{token}" → QuizProgress (per-event quiz progress)

use serde::{Deserialize, Serialize};

/// Helper for serde default = true.
fn default_true() -> bool {
    true
}

/// Controls when online registration opens for hybrid events.
///
/// - `Always`: Both tracks open from registration start.
/// - `AutoOnFull`: Online opens automatically when in-person capacity is reached.
/// - `Manual`: Organizer flips toggle manually via staff UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnlineOpenMode {
    #[default]
    Always,
    #[serde(alias = "auto")]
    AutoOnFull,
    Manual,
}

impl OnlineOpenMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::AutoOnFull => "auto_on_full",
            Self::Manual => "manual",
        }
    }
}

impl std::fmt::Display for OnlineOpenMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Controls event discoverability — whether the event appears publicly or requires auth.
///
/// - `Public`: Visible on landing page, accessible to anyone via `/e/{slug}`.
/// - `Private`: Hidden from landing page, requires auth + access check via `/e/{slug}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventVisibility {
    #[default]
    Public,
    Private,
}

impl EventVisibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

impl std::fmt::Display for EventVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Event format — controls deposit, check-in, claim, and escrow paths.
///
/// - `InPerson`: Physical event, deposit auto-enabled, physical check-in required.
/// - `Online`: Virtual event, no deposit, quest completion = virtual check-in.
/// - `Hybrid`: Both tracks in one event, one Google Sheet with participation_type column.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum EventFormat {
    #[default]
    InPerson,
    Online,
    Hybrid,
}

impl EventFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InPerson => "in_person",
            Self::Online => "online",
            Self::Hybrid => "hybrid",
        }
    }

    /// Whether this format includes an in-person track (requires deposit, physical check-in).
    pub fn has_in_person(&self) -> bool {
        matches!(self, Self::InPerson | Self::Hybrid)
    }

    /// Whether this format includes an online track (quest-based virtual check-in).
    pub fn has_online(&self) -> bool {
        matches!(self, Self::Online | Self::Hybrid)
    }
}

impl std::fmt::Display for EventFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// On-chain escrow lifecycle status.
/// Tracks the state machine: None → Initialized → Deactivated → Closed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EscrowStatus {
    /// No escrow initialized on-chain (or never set).
    #[default]
    None,
    /// Escrow PDA created on-chain, accepting deposits.
    Initialized,
    /// Escrow deactivated — no new deposits, refunds still allowed.
    Deactivated,
    /// Escrow closed — all on-chain accounts reclaimed, rent refunded.
    Closed,
    /// Event cancelled — refunds in progress (organizer-initiated cancellation).
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

impl std::fmt::Display for EscrowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

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
    /// Whether event time is TBA (To Be Announced). When true, public pages show "TBA" instead of time.
    #[serde(default)]
    pub time_tba: bool,
    /// Google Sheets spreadsheet ID for attendee data.
    #[serde(default)]
    pub sheet_id: String,
    /// ISO 8601 creation timestamp.
    #[serde(default)]
    pub created_at: String,
    /// Organization this event belongs to. Empty string = global (no org).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub organization_id: String,
    /// Emails of users with organizer-level access to this event.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub organizer_emails: Vec<String>,
    /// Whether deposit is enabled for this event.
    #[serde(default)]
    pub deposit_enabled: bool,
    /// Maximum number of refundable deposits (0 = unlimited). Deposits beyond
    /// this count are non-refundable — they pay to attend but get no refund.
    #[serde(default)]
    pub max_refundable_deposits: u32,
    /// On-chain escrow PDA address. Empty string if not yet initialized.
    #[serde(default)]
    pub escrow_address: String,
    /// On-chain escrow lifecycle status (none → initialized → deactivated → closed).
    #[serde(default)]
    pub escrow_status: EscrowStatus,
    /// Event format — controls deposit, check-in, and claim paths.
    #[serde(default)]
    pub event_format: EventFormat,
    /// Event tagline / subtitle.
    #[serde(default)]
    pub tagline: String,
    /// Event venue / location.
    #[serde(default)]
    pub location: String,
    /// YouTube/live stream/recording URL for the event.
    #[serde(default)]
    pub video_url: String,
    /// NFT badge image URL (for event card display).
    #[serde(default)]
    pub nft_image_url: String,

    // ── Capacity settings ─────────────────────────────────────────────
    /// Maximum number of in-person attendees. None = unlimited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_person_capacity: Option<u32>,
    /// Maximum number of online attendees. None = unlimited (default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online_capacity: Option<u32>,
    /// Event visibility — public (shown on landing) or private (auth required).
    #[serde(default)]
    pub visibility: EventVisibility,
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
    /// Whether event time is TBA (To Be Announced).
    #[serde(default)]
    pub time_tba: bool,

    // ── Google Sheets ─────────────────────────────────────────────────
    /// Google Sheets spreadsheet ID (contains attendee + staff tabs).
    pub sheet_id: String,
    /// Tab name for attendee data (e.g. "Attendees").
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
    /// Organization this event belongs to. Empty string = global (no org).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub organization_id: String,
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

    // ── Deposit settings ──────────────────────────────────────────────
    /// Whether deposit is required for this event.
    #[serde(default)]
    pub deposit_enabled: bool,
    /// Deposit amount in USDC smallest unit (6 decimals). e.g., 15_000_000 = $15.
    #[serde(default)]
    pub deposit_amount_usdc: u64,
    /// Deposit amount in Thai Baht (for PromptPay track). e.g., 500.
    #[serde(default)]
    pub deposit_amount_thb: u64,
    /// PromptPay ID for THB payments (Thai phone number or national ID).
    /// e.g., "0812345678" or "1-1001-00000-00-0".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub promptpay_id: String,
    /// EventEscrow PDA address (set after on-chain create_event). Empty if not yet created.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub escrow_address: String,
    /// On-chain escrow lifecycle status (none → initialized → deactivated → closed).
    #[serde(default)]
    pub escrow_status: EscrowStatus,
    /// Organizer's Solana wallet address (base58). Required for PDA derivation.
    /// Set when event is created on-chain via the escrow program.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub organizer_wallet: String,
    /// On-chain event ID (u64) used for PDA seed derivation.
    /// Set when event is created on-chain. If 0, derived from event slug hash.
    #[serde(default)]
    pub on_chain_event_id: u64,
    /// Hours after event_end for refund deadline (default: 168 = 7 days).
    #[serde(default)]
    pub refund_deadline_hours: u32,
    /// Maximum number of refundable deposits (0 = unlimited).
    #[serde(default)]
    pub max_refundable_deposits: u32,

    // ── Public details ───────────────────────────────────────────────
    /// Event description (markdown or plain text).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Event location (venue name, address, or "Online").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub location: String,
    /// YouTube/live stream/recording URL for the event.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub video_url: String,
    /// Event visibility — public (shown on landing) or private (auth required).
    #[serde(default)]
    pub visibility: EventVisibility,

    // ── Event format ───────────────────────────────────────────────────
    /// Event format — In-Person, Online, or Hybrid.
    /// Controls deposit, check-in, escrow, and claim paths.
    #[serde(default)]
    pub event_format: EventFormat,

    // ── Registration settings ────────────────────────────────────────
    /// Whether contact info (channel + handle) is required during self-registration.
    /// Defaults to true. Organizers can disable for events that don't need it.
    #[serde(default = "default_true")]
    pub require_contact_info: bool,
    /// Whether photo/media consent is collected during registration (PDPA).
    /// Defaults to false. Organizers enable it for events with photography.
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
    /// None = no deadline (in-person spot held indefinitely).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deposit_deadline_hours: Option<u32>,

    // ── Timestamps ────────────────────────────────────────────────────
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// ISO 8601 last-update timestamp.
    pub updated_at: String,
    /// Email of the user who last updated this event (set from JWT claims).
    #[serde(default)]
    pub updated_by: String,
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
            time_tba: self.time_tba,
            sheet_id: self.sheet_id.clone(),
            created_at: self.created_at.clone(),
            organization_id: self.organization_id.clone(),
            organizer_emails: self.organizer_emails.clone(),
            deposit_enabled: self.deposit_enabled,
            max_refundable_deposits: self.max_refundable_deposits,
            escrow_address: self.escrow_address.clone(),
            escrow_status: self.escrow_status.clone(),
            event_format: self.event_format.clone(),
            tagline: self.tagline.clone(),
            location: self.location.clone(),
            video_url: self.video_url.clone(),
            nft_image_url: self.nft_image_url.clone(),
            in_person_capacity: self.in_person_capacity,
            online_capacity: self.online_capacity,
            visibility: self.visibility.clone(),
        }
    }

    /// Resolve the NFT name, expanding `{event_name}` placeholder.
    /// Truncates to 32 characters (Bubblegum/Metaplex `MetadataNameTooLong` limit).
    /// Prefers keeping the prefix intact and truncating the event name portion.
    pub fn nft_name(&self) -> String {
        let resolved = if self.nft_name_template.is_empty() {
            format!("BeThere - {}", self.name)
        } else {
            self.nft_name_template.replace("{event_name}", &self.name)
        };
        if resolved.len() <= 32 {
            return resolved;
        }
        // Smart truncation: keep prefix before {event_name}, truncate event name portion
        let prefix = if self.nft_name_template.is_empty() {
            "BeThere - "
        } else {
            match self.nft_name_template.find("{event_name}") {
                Some(idx) => &self.nft_name_template[..idx],
                None => "",
            }
        };
        let budget = 32usize.saturating_sub(prefix.len()).saturating_sub(3); // 3 for "..."
        if budget > 0 {
            let truncated_name: String = self.name.chars().take(budget).collect();
            format!("{prefix}{truncated_name}...")
        } else {
            // Prefix itself is too long — fall back to simple truncation
            let mut truncated: String = resolved.chars().take(29).collect();
            truncated.push_str("...");
            truncated
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

    // ── Domain behavior methods (Phase 1 DDD) ─────────────────────────

    /// Is this event accepting new registrations?
    /// Requires Active status and event must not have started (unless start is 0 = TBA).
    pub fn is_registration_open(&self, now_ms: i64) -> bool {
        self.status == EventStatus::Active
            && (self.event_start_ms == 0 || now_ms < self.event_start_ms)
    }

    /// Is the refund deadline still in the future?
    /// Deadline = event_end_ms + refund_deadline_hours * 3600_000 ms.
    pub fn is_refund_eligible(&self, now_ms: i64) -> bool {
        let deadline = self.event_end_ms + (self.refund_deadline_hours as i64 * 3_600_000);
        now_ms <= deadline
    }

    /// Is in-person capacity still available?
    /// `None` capacity means unlimited.
    pub fn has_in_person_capacity(&self, current_count: u32) -> bool {
        self.in_person_capacity
            .is_none_or(|cap| current_count < cap)
    }

    /// Is online capacity still available?
    /// `None` capacity means unlimited.
    pub fn has_online_capacity(&self, current_count: u32) -> bool {
        self.online_capacity.is_none_or(|cap| current_count < cap)
    }

    /// Are USDC deposits accepted? Only when deposit is enabled and escrow is initialized.
    pub fn accepts_usdc_deposits(&self) -> bool {
        self.deposit_enabled && self.escrow_status == EscrowStatus::Initialized
    }

    /// Has the deposit deadline passed for a given registration date?
    /// Returns `false` if no deadline is configured (`deposit_deadline_hours` is None).
    pub fn deposit_deadline_passed(&self, registration_date_ms: i64, now_ms: i64) -> bool {
        match self.deposit_deadline_hours {
            Some(hours) => {
                let deadline = registration_date_ms + (hours as i64 * 3_600_000);
                now_ms > deadline
            }
            None => false,
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
            time_tba: false,
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
            organization_id: String::new(),
            organizer_emails,
            staff_emails,
            claim_base_url: claim_base_url.to_string(),
            deposit_enabled: false,
            deposit_amount_usdc: 0,
            deposit_amount_thb: 0,
            promptpay_id: String::new(),
            escrow_address: String::new(),
            escrow_status: EscrowStatus::None,
            organizer_wallet: String::new(),
            on_chain_event_id: 0,
            refund_deadline_hours: 168,
            max_refundable_deposits: 0,
            description: String::new(),
            location: String::new(),
            video_url: String::new(),
            event_format: EventFormat::InPerson,
            require_contact_info: true,
            require_photo_consent: false,
            in_person_capacity: None,
            online_capacity: None,
            online_open_mode: OnlineOpenMode::default(),
            online_registration_open: false,
            deposit_deadline_hours: None,
            visibility: EventVisibility::default(),
            created_at: String::new(),
            updated_at: String::new(),
            updated_by: String::new(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event() -> EventConfig {
        EventConfig {
            id: "test-event".to_string(),
            name: "Test Event".to_string(),
            slug: "test-event".to_string(),
            tagline: String::new(),
            link: String::new(),
            status: EventStatus::Active,
            event_start_ms: 2_000_000_000_000, // ~2033
            event_end_ms: 2_000_001_000_000,
            time_tba: false,
            sheet_id: String::new(),
            sheet_name: "Attendees".to_string(),
            staff_sheet_name: "staff".to_string(),
            quiz_enabled: false,
            nft_collection_mint: String::new(),
            nft_metadata_uri: String::new(),
            nft_image_url: String::new(),
            nft_name_template: String::new(),
            nft_symbol: String::new(),
            nft_description_template: String::new(),
            merkle_tree: String::new(),
            organization_id: String::new(),
            organizer_emails: vec![],
            staff_emails: vec![],
            claim_base_url: String::new(),
            deposit_enabled: false,
            deposit_amount_usdc: 0,
            deposit_amount_thb: 0,
            promptpay_id: String::new(),
            escrow_address: String::new(),
            escrow_status: EscrowStatus::None,
            organizer_wallet: String::new(),
            on_chain_event_id: 0,
            refund_deadline_hours: 168, // 7 days
            max_refundable_deposits: 0,
            description: String::new(),
            location: String::new(),
            video_url: String::new(),
            visibility: EventVisibility::Public,
            event_format: EventFormat::InPerson,
            require_contact_info: true,
            require_photo_consent: false,
            in_person_capacity: None,
            online_capacity: None,
            online_open_mode: OnlineOpenMode::Always,
            online_registration_open: false,
            deposit_deadline_hours: None,
            created_at: String::new(),
            updated_at: String::new(),
            updated_by: String::new(),
        }
    }

    // ── is_registration_open ────────────────────────────────────────

    #[test]
    fn test_registration_open_active_before_start() {
        let event = make_event();
        assert!(event.is_registration_open(1_000_000_000_000));
    }

    #[test]
    fn test_registration_closed_after_start() {
        let event = make_event();
        assert!(!event.is_registration_open(2_500_000_000_000));
    }

    #[test]
    fn test_registration_open_when_start_is_zero() {
        let mut event = make_event();
        event.event_start_ms = 0; // TBA
        assert!(event.is_registration_open(2_500_000_000_000));
    }

    #[test]
    fn test_registration_closed_when_draft() {
        let mut event = make_event();
        event.status = EventStatus::Draft;
        assert!(!event.is_registration_open(1_000_000_000_000));
    }

    #[test]
    fn test_registration_closed_when_completed() {
        let mut event = make_event();
        event.status = EventStatus::Completed;
        assert!(!event.is_registration_open(1_000_000_000_000));
    }

    // ── is_refund_eligible ──────────────────────────────────────────

    #[test]
    fn test_refund_eligible_before_deadline() {
        let event = make_event();
        // event_end + 168h * 3600_000 = 2_000_001_000_000 + 604_800_000 = 2_000_605_800_000
        assert!(event.is_refund_eligible(2_000_605_800_000)); // exactly at deadline
    }

    #[test]
    fn test_refund_not_eligible_after_deadline() {
        let event = make_event();
        assert!(!event.is_refund_eligible(2_000_605_800_001));
    }

    #[test]
    fn test_refund_eligible_well_before_deadline() {
        let event = make_event();
        assert!(event.is_refund_eligible(2_000_002_000_000));
    }

    // ── has_in_person_capacity ──────────────────────────────────────

    #[test]
    fn test_in_person_capacity_unlimited() {
        let event = make_event(); // in_person_capacity = None
        assert!(event.has_in_person_capacity(999));
    }

    #[test]
    fn test_in_person_capacity_has_room() {
        let mut event = make_event();
        event.in_person_capacity = Some(100);
        assert!(event.has_in_person_capacity(50));
    }

    #[test]
    fn test_in_person_capacity_at_limit() {
        let mut event = make_event();
        event.in_person_capacity = Some(100);
        assert!(!event.has_in_person_capacity(100));
    }

    #[test]
    fn test_in_person_capacity_over_limit() {
        let mut event = make_event();
        event.in_person_capacity = Some(100);
        assert!(!event.has_in_person_capacity(101));
    }

    // ── has_online_capacity ─────────────────────────────────────────

    #[test]
    fn test_online_capacity_unlimited() {
        let event = make_event(); // online_capacity = None
        assert!(event.has_online_capacity(999));
    }

    #[test]
    fn test_online_capacity_at_limit() {
        let mut event = make_event();
        event.online_capacity = Some(50);
        assert!(!event.has_online_capacity(50));
    }

    #[test]
    fn test_online_capacity_has_room() {
        let mut event = make_event();
        event.online_capacity = Some(50);
        assert!(event.has_online_capacity(49));
    }

    // ── accepts_usdc_deposits ───────────────────────────────────────

    #[test]
    fn test_accepts_usdc_when_enabled_and_initialized() {
        let mut event = make_event();
        event.deposit_enabled = true;
        event.escrow_status = EscrowStatus::Initialized;
        assert!(event.accepts_usdc_deposits());
    }

    #[test]
    fn test_rejects_usdc_when_deposit_disabled() {
        let mut event = make_event();
        event.deposit_enabled = false;
        event.escrow_status = EscrowStatus::Initialized;
        assert!(!event.accepts_usdc_deposits());
    }

    #[test]
    fn test_rejects_usdc_when_escrow_not_initialized() {
        let mut event = make_event();
        event.deposit_enabled = true;
        event.escrow_status = EscrowStatus::None;
        assert!(!event.accepts_usdc_deposits());
    }

    #[test]
    fn test_rejects_usdc_when_escrow_deactivated() {
        let mut event = make_event();
        event.deposit_enabled = true;
        event.escrow_status = EscrowStatus::Deactivated;
        assert!(!event.accepts_usdc_deposits());
    }

    // ── deposit_deadline_passed ─────────────────────────────────────

    #[test]
    fn test_deadline_not_passed_when_no_deadline_configured() {
        let event = make_event(); // deposit_deadline_hours = None
        assert!(!event.deposit_deadline_passed(1_000_000_000_000, 2_000_000_000_000));
    }

    #[test]
    fn test_deadline_passed_when_overdue() {
        let mut event = make_event();
        event.deposit_deadline_hours = Some(24); // 24h after registration
        let reg_ms = 1_000_000_000_000_i64;
        let deadline_ms = reg_ms + (24_i64 * 3_600_000); // +24h
        assert!(!event.deposit_deadline_passed(reg_ms, deadline_ms)); // exactly at deadline
        assert!(event.deposit_deadline_passed(reg_ms, deadline_ms + 1)); // just past
    }

    #[test]
    fn test_deadline_not_passed_when_within_window() {
        let mut event = make_event();
        event.deposit_deadline_hours = Some(48);
        let reg_ms = 1_000_000_000_000_i64;
        let within_ms = reg_ms + (12_i64 * 3_600_000); // +12h
        assert!(!event.deposit_deadline_passed(reg_ms, within_ms));
    }
}
