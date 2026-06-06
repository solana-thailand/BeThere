use crate::api::EventFormat;
use leptos::prelude::*;
use leptos_router::params::Params;
use wasm_bindgen::prelude::*;

// ===== Navigation JS Interop =====
#[wasm_bindgen(module = "/js/navigation.js")]
extern "C" {
    pub fn saveProgress(attendee_id: &str, event_id: &str, slug: &str);
    pub fn loadProgress() -> Option<String>;
    pub fn navigateTo(path: &str);
    #[wasm_bindgen(js_name = "shareEvent")]
    pub fn share_event_js(title: &str, url: &str) -> js_sys::Promise;
}

// ---------------------------------------------------------------------------
// Google SVG icon
// ---------------------------------------------------------------------------

/// Google SVG icon markup.
///
/// Defined as a module-level function to avoid the `#[component]` macro
/// misinterpreting hex color values like `#4285F4` as Rust tokens.
pub fn google_icon() -> &'static str {
    "<svg viewBox=\"0 0 24 24\" width=\"20\" height=\"20\">\
        <path fill=\"#4285F4\" d=\"M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 0 1-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z\"/>\
        <path fill=\"#34A853\" d=\"M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z\"/>\
        <path fill=\"#FBBC05\" d=\"M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z\"/>\
        <path fill=\"#EA4335\" d=\"M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z\"/>\
    </svg>"
}

// ---------------------------------------------------------------------------
// Registration API types
// ---------------------------------------------------------------------------

/// Request body for POST /api/public/register.
#[derive(serde::Serialize)]
pub struct RegisterBody {
    pub slug: String,
    pub name: String,
    #[allow(dead_code)]
    pub email: String,
    pub participation_type: Option<String>,
    pub contact_channel: Option<String>,
    pub contact_handle: Option<String>,
    pub deposit_agreed: Option<bool>,
    pub consent_given: Option<bool>,
    pub photo_consent_given: Option<bool>,
    pub consent_marketing: Option<bool>,
}

/// Next step returned after registration.
#[derive(serde::Deserialize, Clone, Debug)]
pub struct NextStep {
    #[serde(rename = "type")]
    pub _step_type: String,
    pub url: String,
}

/// Response from POST /api/public/register.
#[derive(serde::Deserialize)]
pub struct RegisterResponse {
    pub success: bool,
    pub data: Option<RegisterData>,
    pub error: Option<String>,
}

#[derive(serde::Deserialize, Clone, Debug)]
#[allow(dead_code)]
pub struct RegisterData {
    pub attendee_id: String,
    pub name: String,
    pub email: String,
    pub claim_token: String,
    pub next_step: NextStep,
}

/// Registration form state.
#[derive(Clone, Debug)]
pub enum RegState {
    Idle,
    Submitting,
    Success(RegisterData),
    /// API/network error only. Field validation uses `field_errors`.
    Error(String),
}

/// Inline field validation errors.
#[derive(Clone, Debug, Default)]
pub struct FieldErrors {
    pub name: Option<String>,
    pub contact_channel: Option<String>,
    pub contact_handle: Option<String>,
    pub deposit_agreed: Option<String>,
    pub consent_given: Option<String>,
    pub photo_consent_given: Option<String>,
}

// ---------------------------------------------------------------------------
// Auth state
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum AuthState {
    Checking,
    SignedIn(String),
    NotSignedIn,
}

// ---------------------------------------------------------------------------
// My-registration lookup
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
pub struct MyRegistrationResponse {
    pub success: bool,
    pub data: Option<MyRegistrationData>,
    pub error: Option<String>,
}

#[derive(serde::Deserialize, Clone, Debug)]
#[allow(dead_code)]
pub struct MyRegistrationData {
    pub attendee_id: String,
    pub name: String,
    pub email: String,
    pub claim_token: String,
    pub participation_type: Option<String>,
    pub next_step: NextStep,
}

#[derive(Clone, Debug)]
pub enum RegistrationLookup {
    Pending,
    NotRegistered,
    Registered(MyRegistrationData),
    Error(String),
}

// ---------------------------------------------------------------------------
// URL params
// ---------------------------------------------------------------------------

#[derive(Params, PartialEq, Clone, Debug)]
pub struct PublicEventParams {
    pub slug: Option<String>,
}

// ---------------------------------------------------------------------------
// Public event data
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize, Clone, Debug)]
#[allow(dead_code)]
pub struct PublicEventData {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub tagline: String,
    pub link: String,
    pub status: String,
    pub event_start_ms: i64,
    pub event_end_ms: i64,
    pub time_tba: bool,
    pub deposit_enabled: bool,
    pub deposit_amount_usdc: f64,
    pub deposit_amount_thb: f64,
    pub event_format: EventFormat,
    pub nft_image_url: String,
    pub nft_name_template: String,
    pub nft_symbol: String,
    pub nft_description_template: String,
    pub quiz_enabled: bool,
    pub refund_deadline_hours: u32,
    pub require_contact_info: bool,
    pub require_photo_consent: bool,
    pub description: String,
    pub location: String,
    pub created_at: String,
    pub dev_mode: bool,

    // Capacity fields
    pub in_person_capacity: Option<u32>,
    pub online_capacity: Option<u32>,
    pub in_person_count: Option<u32>,
    pub online_count: Option<u32>,
    pub in_person_remaining: Option<u32>,
    pub online_remaining: Option<u32>,
    pub in_person_available: bool,
    pub online_available: bool,
    pub online_open_mode: Option<String>,

    // Escrow
    pub escrow_status: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct PublicEventResponse {
    pub success: bool,
    pub data: Option<PublicEventData>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Page state
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum PublicEventState {
    Loading,
    Loaded(PublicEventData),
    NotFound,
    Error(String),
}

// ---------------------------------------------------------------------------
// Scroll helper
// ---------------------------------------------------------------------------

/// Smooth-scroll to a DOM element by its `id` attribute.
pub fn scroll_to_element(id: &str) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };
    if let Some(el) = document.get_element_by_id(id) {
        el.scroll_into_view();
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

pub fn format_usdc(amount_micro: f64) -> String {
    let usdc = amount_micro / 1_000_000.0;
    if usdc >= 1.0 {
        format!("{usdc:.0} USDC")
    } else {
        format!("{usdc:.2} USDC")
    }
}

pub fn format_thb(amount: f64) -> String {
    if amount >= 1.0 {
        format!("{amount:.0}")
    } else {
        format!("{amount:.2}")
    }
}

pub fn format_event_date(ms: i64) -> String {
    let date = js_sys::Date::new(&(ms as f64).into());
    let days = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    let months = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let day_idx = date.get_day() as usize;
    let month_idx = date.get_month() as usize;
    let day = date.get_date();
    let year = date.get_full_year();
    format!(
        "{}, {} {} {}",
        days[day_idx % 7],
        day,
        months[month_idx % 12],
        year
    )
}

pub fn format_event_time(ms: i64) -> String {
    let date = js_sys::Date::new(&(ms as f64).into());
    let h = date.get_hours();
    let m = date.get_minutes();
    let suffix = if h >= 12 { "PM" } else { "AM" };
    let h12 = if h == 0 {
        12
    } else if h > 12 {
        h - 12
    } else {
        h
    };
    format!("{h12}:{m:02} {suffix}")
}

pub fn format_countdown(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let secs = ms / 1000;
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    let s = secs % 60;
    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else if hours > 0 {
        format!("{hours}h {mins}m {s}s")
    } else if mins > 0 {
        format!("{mins}m {s}s")
    } else {
        format!("{s}s")
    }
}

pub fn format_refund_deadline(hours: u32) -> String {
    match hours {
        0..=23 => format!("{hours} hours"),
        24 => "1 day".to_string(),
        25..=47 => format!("1 day {} hours", hours % 24),
        48..=167 => format!("{} days", hours / 24),
        168 => "1 week".to_string(),
        _ => format!("{} days", hours / 24),
    }
}
