//! Check-in logic and claim URL helpers.

use leptos::prelude::*;

use crate::api::{self};
use crate::components::{self, ToastType};

use super::interop::*;
use super::state::*;

// ===== Check-In Logic =====

/// Process an attendee ID through the lookup flow.
///
/// Sets the appropriate `CheckInState` based on the attendee's status:
/// - Already checked in → `AlreadyCheckedIn`
/// - Not approved → `NotApproved`
/// - Not In-Person → `NotInPerson`
/// - Approved & In-Person → `Found` (ready to confirm)
pub(super) fn process_attendee_id(
    id: &str,
    set_state: WriteSignal<CheckInState>,
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    set_session_total: WriteSignal<u32>,
) {
    set_session_total.update(|c| *c += 1);
    let attendee_id = id.to_string();
    set_state.set(CheckInState::LookingUp);
    leptos::task::spawn_local(async move {
        match api::get_attendee(&attendee_id, None).await {
            Ok(data) => {
                if data.is_checked_in {
                    feedback_warning_js(); // Already checked in — warning tone
                    set_state.set(CheckInState::AlreadyCheckedIn(Box::new(data)));
                } else if !data.is_approved {
                    feedback_error_js(); // Not approved — error tone
                    set_state.set(CheckInState::NotApproved(Box::new(data)));
                } else if !data.is_in_person {
                    feedback_warning_js(); // Not in-person — warning tone
                    set_state.set(CheckInState::NotInPerson(Box::new(data)));
                } else {
                    // Found — no feedback yet (wait for actual check-in confirmation)
                    set_state.set(CheckInState::Found(Box::new(data)));
                }
            }
            Err(err) => {
                log::warn!("[scanner] attendee lookup failed for id={attendee_id}: {err}");
                feedback_error_js(); // Not found — error tone
                set_state.set(CheckInState::NotFound);
                components::show_toast(&set_toast, "Attendee not found", ToastType::Error);
            }
        }
    });
}

/// Extract attendee ID from a QR code value or manual input.
///
/// Handles multiple formats:
/// - Raw API ID: `gst-abc123`
/// - URL with `?scan=`: `https://server/staff/?scan=gst-abc123`
/// - URL with `?id=`: `https://server/staff/?id=gst-abc123`
pub(super) fn extract_attendee_id(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Try URL parameter extraction
    if trimmed.starts_with("http")
        && let Ok(url) = web_sys::Url::new(trimmed)
    {
        if let Some(scan) = url.search_params().get("scan") {
            return Some(scan);
        }
        if let Some(id_param) = url.search_params().get("id") {
            return Some(id_param);
        }
    }

    // Return as-is (gst- prefix or raw ID)
    Some(trimmed.to_string())
}

// ===== Claim URL Helpers =====

/// Build the full claim URL from a claim token using the current window origin.
///
/// This makes the QR code dynamic — works correctly on both localhost:8787
/// (local testing) and the production domain without backend config changes.
pub(super) fn build_claim_url(token: &str) -> String {
    let window = web_sys::window().expect("no window");
    let origin = window
        .location()
        .origin()
        .unwrap_or_else(|_| "http://localhost:8787".to_string());
    format!("{origin}/claim/{token}")
}
