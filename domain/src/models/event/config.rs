//! Stored event configuration: index metadata and the full per-event config.

use serde::{Deserialize, Serialize};

use super::defaults::default_true;
use super::enums::{EscrowStatus, EventFormat, EventStatus, EventVisibility, OnlineOpenMode};

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
    /// Marketing poster URL for the event page hero (served path or external URL).
    /// Empty = fall back to `nft_image_url` on the public event page.
    #[serde(default)]
    pub poster_url: String,
    /// Denormalized flag mirroring `events.recap_published` (Plan 008 — Phase 2).
    /// Canonical source is `event_summaries.recap_published_at`; duplicated so
    /// the past-events listing filter works without joining. Set by
    /// `PUT /api/events/{id}/recap`.
    #[serde(default)]
    pub recap_published: bool,
    /// Whether post-event registration (lead capture) is open (Plan 008 — Phase 3).
    /// The public recap page renders a "join the community" CTA when true.
    #[serde(default)]
    pub post_event_registration_open: bool,
    /// Optional deadline (Unix epoch ms) for post-event registration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_event_registration_until_ms: Option<i64>,

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

/// A community/social link shown on event ticket and public event pages.
/// Organizer-configurable. Extensible — new platforms don't require schema changes.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CommunityLink {
    /// Platform identifier: "discord", "telegram", "x", "facebook", "line", "website".
    pub platform: String,
    /// Full URL (e.g. "https://discord.gg/abc123").
    pub url: String,
    /// Optional display label override (e.g. "Solana Thailand Dev Chat").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
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
    /// Marketing poster URL for the event page hero (served path or external URL).
    /// Empty = fall back to `nft_image_url` on the public event page.
    /// Kept separate from `nft_image_url` because that field feeds the on-chain
    /// cNFT mint metadata and must not be overloaded with a marketing image.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub poster_url: String,
    /// Denormalized flag mirroring `events.recap_published` (migration 0020).
    /// Canonical source is `event_summaries.recap_published_at`; this flag is
    /// duplicated onto the events row so `GET /api/public/events/past` can
    /// filter without joining. Set by `PUT /api/events/{id}/recap` (Plan 008 §3.2).
    #[serde(default)]
    pub recap_published: bool,
    /// Whether post-event registration (lead capture) is open (Plan 008 — Phase 3).
    /// Organizer-toggled via `PUT /api/events/{id}/post-event-registration`. When
    /// true, the public recap page shows a "join the community" CTA that posts to
    /// `POST /api/public/event/{slug}/register-post-event`.
    #[serde(default)]
    pub post_event_registration_open: bool,
    /// Optional deadline (Unix epoch ms) after which post-event registration
    /// closes (HTTP 410). `None` = no deadline (open indefinitely until toggled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_event_registration_until_ms: Option<i64>,
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
    /// Google Calendar (or other) embed/subscribe URL for the organization.
    /// When set, ticket page shows "Subscribe to our calendar" link.
    /// Empty = no calendar link shown.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub calendar_subscribe_url: String,

    // ── Community links ─────────────────────────────────────────────
    /// Community/social links shown on ticket + public event pages.
    /// Organizer-configurable. Empty = no community section shown.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub community_links: Vec<CommunityLink>,
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

    // ── Registration form config (Issue #049) ─────────────────────────
    /// Whether developer profile fields are shown on the registration form.
    /// When true, the default "About You" section is displayed.
    /// When a custom form config is stored in KV, this is ignored.
    #[serde(default)]
    pub dev_profile_enabled: bool,
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
            poster_url: self.poster_url.clone(),
            recap_published: self.recap_published,
            post_event_registration_open: self.post_event_registration_open,
            post_event_registration_until_ms: self.post_event_registration_until_ms,
            in_person_capacity: self.in_person_capacity,
            online_capacity: self.online_capacity,
            visibility: self.visibility.clone(),
        }
    }

    /// Whether the post-event registration deadline has passed at `now_ms`.
    ///
    /// `None` = open indefinitely, so it never passes. Plan 008 — Phase 3.
    pub fn post_event_registration_deadline_passed(&self, now_ms: i64) -> bool {
        match self.post_event_registration_until_ms {
            Some(until) => now_ms >= until,
            None => false,
        }
    }

    /// Whether post-event registration actually accepts a submission at `now_ms`
    /// — the organizer's toggle is on **and** the deadline has not passed.
    ///
    /// This is the value public surfaces should gate a "join the community" CTA
    /// on: the raw `post_event_registration_open` flag stays `true` after the
    /// deadline lapses, so rendering the CTA from it invites a visitor to sign
    /// in and fill a form that `POST /api/public/events/{slug}/post-event-register`
    /// answers `410 Gone`. Plan 008 — Phase 3.
    pub fn post_event_registration_accepting(&self, now_ms: i64) -> bool {
        self.post_event_registration_open && !self.post_event_registration_deadline_passed(now_ms)
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
            quiz_enabled: false,
            nft_collection_mint: nft_collection_mint.to_string(),
            nft_metadata_uri: nft_metadata_uri.to_string(),
            nft_image_url: nft_image_url.to_string(),
            poster_url: String::new(),
            recap_published: false,
            post_event_registration_open: false,
            post_event_registration_until_ms: None,
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
            dev_profile_enabled: false,
            community_links: vec![],
            calendar_subscribe_url: String::new(),
        }
    }
}
