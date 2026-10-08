//! Check-in logic and claim URL helpers.

use leptos::prelude::*;

use event_checkin_domain::models::attendee::DisplayCode;

use crate::api::{self};
use crate::components::{self, ToastType};
use crate::i18n::{Locale, td_string};

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
        lookup_and_classify(attendee_id, None, set_state, set_toast).await;
    });
}

/// The shared tail of every lookup: fetch the attendee and pick the state.
async fn lookup_and_classify(
    attendee_id: String,
    event_id: Option<String>,
    set_state: WriteSignal<CheckInState>,
    set_toast: WriteSignal<Option<components::ToastMessage>>,
) {
    match api::get_attendee(&attendee_id, event_id.as_deref()).await {
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
}

/// What staff typed into the manual box (.issues/178).
pub(super) enum ManualEntry {
    /// A ticket's booking code (`Nº 7KQ2XM`, any case, spaces/dashes ok).
    Code(DisplayCode),
    /// An attendee id or a scanned-QR URL.
    AttendeeId(String),
}

/// Read the manual box: a well-formed booking code wins; anything else goes
/// down the attendee-id path as before. Ids are UUIDs or `gst-…`, which never
/// parse as a 6-character code from the code alphabet.
pub(super) fn classify_manual_entry(text: &str) -> Option<ManualEntry> {
    match DisplayCode::parse(text) {
        Ok(code) => Some(ManualEntry::Code(code)),
        Err(_) => extract_attendee_id(text).map(ManualEntry::AttendeeId),
    }
}

/// Resolve a booking code within the selected event, then run the same
/// lookup + state flow as a scan. The code is scoped server-side to the
/// event staff are authorized for; it never unlocks anything by itself.
///
/// `locale` is read by the caller: the toasts fire inside a spawned task,
/// where there is no reactive owner to look the i18n context up from.
pub(super) fn process_display_code(
    code: DisplayCode,
    event_id: Option<String>,
    locale: Locale,
    set_state: WriteSignal<CheckInState>,
    set_toast: WriteSignal<Option<components::ToastMessage>>,
    set_session_total: WriteSignal<u32>,
) {
    let Some(event_id) = event_id.filter(|e| !e.is_empty()) else {
        components::show_toast(
            &set_toast,
            td_string!(locale, ticket.code.scanner_pick_event),
            ToastType::Warning,
        );
        return;
    };
    set_session_total.update(|c| *c += 1);
    set_state.set(CheckInState::LookingUp);
    leptos::task::spawn_local(async move {
        match api::lookup_attendee_id_by_code(&code, Some(&event_id)).await {
            Ok(attendee_id) => {
                lookup_and_classify(attendee_id, Some(event_id), set_state, set_toast).await
            }
            Err(err) => {
                log::warn!("[scanner] ticket code lookup failed: {err}");
                feedback_error_js();
                set_state.set(CheckInState::NotFound);
                components::show_toast(
                    &set_toast,
                    td_string!(locale, ticket.code.scanner_not_found),
                    ToastType::Error,
                );
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
