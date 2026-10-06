//! Form state and helpers: `EventForm`, its defaults and the conversions.

use event_checkin_domain::models::event::{
    DEFAULT_ATTENDEE_SHEET_NAME, DEFAULT_STAFF_SHEET_NAME, Sponsor,
};
use std::sync::Arc;

use crate::api;

/// Maximum number of shared event links, covering community, logistics, and
/// learning resources without separate storage or editing workflows.
pub(super) const MAX_COMMUNITY_LINKS: usize = 10;

// ===== Form State =====

/// Form state for creating/editing events.
#[derive(Debug, Clone, Default)]
pub struct EventForm {
    pub name: String,
    pub slug: String,
    pub tagline: String,
    /// Long-form public description (agenda, schedule, details).
    pub description: String,
    pub link: String,
    pub event_start: String,
    pub event_end: String,
    pub time_tba: bool,
    pub sheet_id: String,
    pub sheet_name: String,
    pub staff_sheet_name: String,
    pub quiz_enabled: bool,
    pub nft_collection_mint: String,
    pub nft_metadata_uri: String,
    pub nft_image_url: String,
    pub poster_url: String,
    pub nft_name_template: String,
    pub nft_symbol: String,
    pub nft_description_template: String,
    pub merkle_tree: String,
    pub claim_base_url: String,
    pub organizer_emails: String,
    pub staff_emails: String,
    pub status: api::EventStatus,
    pub event_format: api::EventFormat,
    pub deposit_enabled: bool,
    pub deposit_amount_usdc: String,
    pub deposit_amount_thb: String,
    pub require_contact_info: bool,
    pub require_photo_consent: bool,
    pub promptpay_id: String,
    pub escrow_address: String,
    pub escrow_status: api::EscrowStatus,
    pub organizer_wallet: String,
    pub on_chain_event_id: String,
    pub refund_deadline_hours: String,
    pub max_refundable_deposits: String,
    pub location: String,
    pub location_map_url: String,
    /// Deposit-waived emails, as typed: one per line or comma-separated. Parsed
    /// on submit; normalised server-side so casing here does not matter.
    pub comp_emails: String,
    pub video_url: String,
    pub in_person_capacity: String,
    pub online_capacity: String,
    pub online_open_mode: api::OnlineOpenMode,
    pub online_registration_open: bool,
    pub deposit_deadline_hours: String,
    pub visibility: api::EventVisibility,
    pub updated_at: String,
    pub community_links: Vec<crate::api::CommunityLink>,
    /// Sponsors (migration 0057); edited row by row in `SponsorsSection`.
    pub sponsors: Vec<Sponsor>,
    pub calendar_subscribe_url: String,
    /// Ticket-page announcement shown to in-person attendees (migration 0049).
    pub ticket_note_in_person: String,
    /// Ticket-page announcement shown to online attendees (migration 0049).
    pub ticket_note_online: String,
    /// Postponed notice (migration 0053). Empty = not postponed.
    pub postponed_note: String,
}

// ===== Helpers =====

/// Auto-generate a URL-safe slug from a name: the Worker's rule, so the
/// preview is the id the event gets (an all-Thai name → `event-{hash}`).
pub(super) fn generate_slug(name: &str) -> String {
    event_checkin_domain::slug::Slug::from_text(name).or_prefixed("event")
}

/// Parse an ISO date string or epoch ms string to epoch milliseconds.
pub(super) fn parse_date_to_ms(date_str: &str) -> Option<i64> {
    if date_str.is_empty() {
        return None;
    }
    // Try parsing as epoch ms first
    if let Ok(ms) = date_str.parse::<i64>()
        && ms > 1_000_000_000_000
    {
        return Some(ms);
    }
    // Parse as ISO datetime-local format (YYYY-MM-DDTHH:MM)
    let cleaned = date_str.replace('T', " ");
    let parsed = js_sys::Date::parse(&cleaned);
    if parsed.is_nan() {
        // Fallback: try original string
        let parsed2 = js_sys::Date::parse(date_str);
        if parsed2.is_nan() {
            return None;
        }
        Some(parsed2 as i64)
    } else {
        Some(parsed as i64)
    }
}

/// Format epoch milliseconds to a short readable date string.
pub fn format_date_display(ms: i64) -> String {
    if ms == 0 {
        return "\u{2014}".to_string();
    }
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    let year = date.get_full_year();
    let month = date.get_month() + 1; // 0-indexed
    let day = date.get_date();
    let hours = date.get_hours();
    let minutes = date.get_minutes();
    let seconds = date.get_seconds();
    let tz_offset_min = date.get_timezone_offset(); // minutes behind UTC (positive = west)
    let tz_sign = if tz_offset_min >= 0.0 { '-' } else { '+' };
    let tz_abs = tz_offset_min.abs() as i32;
    let tz_h = tz_abs / 60;
    let tz_m = tz_abs % 60;
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC{}{:02}:{:02}",
        year, month, day, hours, minutes, seconds, tz_sign, tz_h, tz_m
    )
}

/// Format epoch milliseconds to datetime-local input format (YYYY-MM-DDTHH:MM).
pub(super) fn format_datetime_local(ms: i64) -> String {
    if ms == 0 {
        return String::new();
    }
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    let year = date.get_full_year();
    let month = date.get_month() + 1; // 0-indexed
    let day = date.get_date();
    let hours = date.get_hours();
    let minutes = date.get_minutes();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}",
        year, month, day, hours, minutes
    )
}

/// Build self-hosted NFT badge URLs. No Arweave/IPFS needed.
pub(super) fn get_self_hosted_nft_urls() -> (String, String) {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_else(|| "https://bethere.solana-thailand.workers.dev".to_string());
    let image_url = format!("{origin}/api/badge-hd.svg");
    let metadata_uri_prefix = format!("{origin}/api/metadata/");
    (image_url, metadata_uri_prefix)
}

/// Create a default form state with sensible defaults.
pub fn default_form() -> EventForm {
    EventForm {
        name: String::new(),
        slug: String::new(),
        tagline: String::new(),
        description: String::new(),
        link: String::new(),
        event_start: String::new(),
        event_end: String::new(),
        time_tba: false,
        sheet_id: String::new(),
        sheet_name: DEFAULT_ATTENDEE_SHEET_NAME.to_string(),
        staff_sheet_name: DEFAULT_STAFF_SHEET_NAME.to_string(),
        quiz_enabled: false,
        nft_collection_mint: String::new(),
        nft_metadata_uri: String::new(),
        nft_image_url: String::new(),
        poster_url: String::new(),
        nft_name_template: String::new(),
        nft_symbol: String::new(),
        nft_description_template: String::new(),
        merkle_tree: String::new(),
        claim_base_url: String::new(),
        organizer_emails: String::new(),
        staff_emails: String::new(),
        status: api::EventStatus::Draft,
        event_format: api::EventFormat::InPerson,
        deposit_enabled: true,
        deposit_amount_usdc: String::new(),
        deposit_amount_thb: String::new(),
        require_contact_info: true,
        require_photo_consent: false,
        promptpay_id: String::new(),
        escrow_address: String::new(),
        escrow_status: api::EscrowStatus::None,
        organizer_wallet: String::new(),
        on_chain_event_id: String::new(),
        refund_deadline_hours: String::new(),
        max_refundable_deposits: String::new(),
        location: String::new(),
        location_map_url: String::new(),
        comp_emails: String::new(),
        video_url: String::new(),
        in_person_capacity: String::new(),
        online_capacity: String::new(),
        online_open_mode: api::OnlineOpenMode::default(),
        online_registration_open: false,
        deposit_deadline_hours: String::new(),
        visibility: api::EventVisibility::default(),
        updated_at: String::new(),
        community_links: vec![],
        sponsors: vec![],
        calendar_subscribe_url: String::new(),
        ticket_note_in_person: String::new(),
        ticket_note_online: String::new(),
        postponed_note: String::new(),
    }
}

/// Populate form state from an event detail API response.
pub fn form_from_detail(detail: &api::EventDetail) -> EventForm {
    EventForm {
        name: detail.name.clone(),
        slug: detail.slug.clone(),
        tagline: detail.tagline.clone(),
        description: detail.description.clone(),
        link: detail.link.clone(),
        event_start: if detail.event_start_ms > 0 {
            format_datetime_local(detail.event_start_ms)
        } else {
            String::new()
        },
        event_end: if detail.event_end_ms > 0 {
            format_datetime_local(detail.event_end_ms)
        } else {
            String::new()
        },
        time_tba: detail.time_tba,
        sheet_id: detail.sheet_id.clone(),
        sheet_name: if detail.sheet_name.is_empty() {
            DEFAULT_ATTENDEE_SHEET_NAME.to_string()
        } else {
            detail.sheet_name.clone()
        },
        staff_sheet_name: if detail.staff_sheet_name.is_empty() {
            DEFAULT_STAFF_SHEET_NAME.to_string()
        } else {
            detail.staff_sheet_name.clone()
        },
        quiz_enabled: detail.quiz_enabled,
        nft_collection_mint: detail.nft_collection_mint.clone(),
        nft_metadata_uri: detail.nft_metadata_uri.clone(),
        nft_image_url: detail.nft_image_url.clone(),
        poster_url: detail.poster_url.clone(),
        nft_name_template: detail.nft_name_template.clone(),
        nft_symbol: detail.nft_symbol.clone(),
        nft_description_template: detail.nft_description_template.clone(),
        merkle_tree: detail.merkle_tree.clone(),
        claim_base_url: detail.claim_base_url.clone(),
        organizer_emails: detail.organizer_emails.join(", "),
        staff_emails: detail.staff_emails.join(", "),
        status: detail.status.clone(),
        event_format: detail.event_format.clone(),
        deposit_enabled: detail.deposit_enabled,
        deposit_amount_usdc: if detail.deposit_amount_usdc > 0 {
            format!("{:.6}", detail.deposit_amount_usdc as f64 / 1_000_000.0)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        } else {
            String::new()
        },
        deposit_amount_thb: if detail.deposit_amount_thb > 0 {
            detail.deposit_amount_thb.to_string()
        } else {
            String::new()
        },
        require_contact_info: detail.require_contact_info,
        require_photo_consent: detail.require_photo_consent,
        promptpay_id: detail.promptpay_id.clone(),
        escrow_address: detail.escrow_address.clone(),
        escrow_status: detail.escrow_status.clone(),
        organizer_wallet: detail.organizer_wallet.clone(),
        on_chain_event_id: if detail.on_chain_event_id > 0 {
            format!("{}", detail.on_chain_event_id)
        } else {
            String::new()
        },
        refund_deadline_hours: if detail.refund_deadline_hours > 0 {
            format!("{}", detail.refund_deadline_hours)
        } else {
            String::new()
        },
        max_refundable_deposits: if detail.max_refundable_deposits > 0 {
            format!("{}", detail.max_refundable_deposits)
        } else {
            String::new()
        },
        location: detail.location.clone(),
        location_map_url: detail.location_map_url.clone(),
        comp_emails: detail.comp_emails.join("\n"),
        video_url: detail.video_url.clone(),
        in_person_capacity: detail
            .in_person_capacity
            .map(|v| v.to_string())
            .unwrap_or_default(),
        online_capacity: detail
            .online_capacity
            .map(|v| v.to_string())
            .unwrap_or_default(),
        online_open_mode: detail.online_open_mode.clone(),
        online_registration_open: detail.online_registration_open,
        deposit_deadline_hours: detail
            .deposit_deadline_hours
            .map(|h| h.to_string())
            .unwrap_or_default(),
        visibility: detail.visibility.clone(),
        updated_at: detail.updated_at.clone(),
        community_links: detail.community_links.clone(),
        sponsors: detail.sponsors.clone(),
        ticket_note_in_person: detail.ticket_note_in_person.clone(),
        ticket_note_online: detail.ticket_note_online.clone(),
        postponed_note: detail.postponed_note.clone(),
        calendar_subscribe_url: detail.calendar_subscribe_url.clone(),
    }
}

/// Parse comma-separated emails into a Vec.
pub(super) fn parse_emails(s: &str) -> Vec<String> {
    s.split(',')
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty())
        .collect()
}

/// CSS class for event status badge.
pub fn status_badge_class(status: &api::EventStatus) -> &'static str {
    match status {
        api::EventStatus::Active => "badge badge-success",
        api::EventStatus::Draft => "badge badge-warning",
        api::EventStatus::Completed => "badge badge-completed",
        api::EventStatus::Archived => "badge badge-archived",
    }
}

/// Human-readable status label.
pub fn status_label(status: &api::EventStatus) -> &'static str {
    match status {
        api::EventStatus::Active => "Active",
        api::EventStatus::Draft => "Draft",
        api::EventStatus::Completed => "Completed",
        api::EventStatus::Archived => "Archived",
    }
}

/// Callback type for when the form is done (save success or cancel).
pub type OnDone = Arc<dyn Fn() + Send + Sync + 'static>;
