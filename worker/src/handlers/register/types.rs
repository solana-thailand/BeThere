//! Request / response types shared across the registration submodules.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterRequest {
    pub slug: String,
    pub name: String,
    /// Kept for backward compatibility — email is now taken from JWT claims.
    #[allow(dead_code)]
    pub email: String,
    /// Optional for InPerson/Online events. Required for Hybrid to choose track.
    /// Defaults based on event format if omitted.
    pub participation_type: Option<String>,
    /// Preferred contact channel (Telegram, Line, Facebook, X (Twitter)).
    /// Required when event has `require_contact_info` enabled.
    pub contact_channel: Option<String>,
    /// Username or profile link for the selected contact channel.
    /// Required when event has `require_contact_info` enabled.
    pub contact_handle: Option<String>,
    /// Whether the attendee agreed to the deposit commitment.
    /// Required when event has `deposit_enabled` enabled.
    pub deposit_agreed: Option<bool>,
    /// Whether the attendee consented to personal data collection (PDPA).
    /// Always required for registration.
    pub consent_given: Option<bool>,
    /// Whether the attendee consented to photo/media capture (PDPA).
    /// Required when event has `require_photo_consent` enabled.
    pub photo_consent_given: Option<bool>,
    /// Whether the attendee wants to receive marketing communications
    /// about future events (optional, PDPA marketing consent).
    pub consent_marketing: Option<bool>,
    /// Developer profile fields (optional — Issue #049 Phase 1, backward compat).
    pub experience_level: Option<String>,
    pub tech_stack: Option<String>,
    pub interests: Option<String>,
    /// Dynamic profile fields from configurable form (Issue #049 Phase 2).
    #[serde(default)]
    pub profile_fields: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NextStep {
    #[serde(rename = "type")]
    pub step_type: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegisterResponse {
    pub attendee_id: String,
    pub name: String,
    pub email: String,
    pub claim_token: String,
    pub next_step: NextStep,
    /// Wallet↔email convergence outcome for wallet-only sessions (Plan 017):
    /// `Some(true)` = wallet was bound to this (brand-new) email; `Some(false)`
    /// = email already had an account, so the wallet was NOT auto-bound (link
    /// it from the profile page instead); `None` = not a wallet session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_linked: Option<bool>,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct MyRegistrationResponse {
    pub attendee_id: String,
    pub name: String,
    pub email: String,
    pub claim_token: String,
    pub participation_type: String,
    pub next_step: NextStep,
}

/// A single registration summary returned by `my_registrations`.
#[derive(Debug, Clone, Serialize)]
pub struct MyRegistrationsItem {
    pub event_id: String,
    pub event_name: String,
    pub event_slug: String,
    pub event_start_ms: i64,
    pub attendee_id: String,
    pub name: String,
    pub participation_type: String,
    /// Human-readable status: "registered", "deposit pending", "deposit confirmed",
    /// "checked in", "nft claimed".
    pub status: String,
    pub next_step: NextStep,
}

/// Request body for `POST /api/public/event/{slug}/register-post-event`.
///
/// A subset of `RegisterRequest` — drops `participation_type`, `deposit_agreed`,
/// `photo_consent_given` (irrelevant: they didn't attend). Keeps the
/// developer-profile fields because capturing the visitor's stack/interests is
/// the primary value of post-event registration.
#[derive(Debug, Clone, Deserialize)]
pub struct PostEventRegisterRequest {
    pub name: String,
    pub contact_channel: Option<String>,
    pub contact_handle: Option<String>,
    /// PDPA consent for data collection (always required).
    pub consent_given: Option<bool>,
    /// Marketing consent for future-event outreach (the point of lead capture).
    #[serde(default)]
    pub consent_marketing: Option<bool>,
    pub experience_level: Option<String>,
    pub tech_stack: Option<String>,
    pub interests: Option<String>,
    #[serde(default)]
    pub profile_fields: Option<std::collections::HashMap<String, String>>,
}

/// Input data for writing developer profile + registration responses to D1.
///
/// Constructed inside the registration handlers and consumed by
/// `write_developer_data`; it therefore spans two submodules and is given
/// `pub(super)` visibility so both can see it.
pub(super) struct DeveloperData<'a> {
    pub(super) d1: &'a worker::D1Database,
    pub(super) email: &'a str,
    pub(super) name: &'a str,
    pub(super) event_id: &'a str,
    pub(super) contact_channel: &'a str,
    pub(super) contact_handle: &'a str,
    pub(super) participation_type: &'a str,
    pub(super) consent_given: bool,
    pub(super) photo_consent_given: bool,
    pub(super) consent_marketing: bool,
    /// Dynamic profile fields (key, value) pairs from form config.
    pub(super) profile_fields: Vec<(String, String)>,
}
