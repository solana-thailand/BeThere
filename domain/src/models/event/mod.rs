//! Event management types for multi-event / organizer support (Issue 004).
//!
//! Events are stored in Cloudflare KV under the EVENTS namespace:
//!   "events"                    → EventIndex (list of EventMeta summaries)
//!   "event:{id}"                → EventConfig (full per-event configuration)
//!   "event:{id}:quiz:questions" → QuizConfig (per-event quiz)
//!   "event:{id}:quiz:progress:{token}" → QuizProgress (per-event quiz progress)

mod config;
mod defaults;
mod enums;
mod form;
mod map_url;
mod requests;
mod responses;
mod sheet_name;
mod slug;
mod ticket_note;

#[cfg(test)]
mod tests;

pub use config::{CommunityLink, EventConfig, EventIndex, EventMeta};
pub use enums::{EscrowStatus, EventFormat, EventStatus, EventVisibility, OnlineOpenMode};
pub use form::{FormFieldConfig, FormFieldType, RegistrationFormConfig};
pub use map_url::{MAX_MAP_URL_CHARS, normalize_map_url, safe_map_url};
pub use requests::{CreateEventRequest, DuplicateEventRequest, UpdateEventRequest};
pub use responses::{
    CreateEventResponse, EventDetailResponse, EventListResponse, UpdateEventResponse,
};
pub use sheet_name::{
    DEFAULT_ATTENDEE_SHEET_NAME, DEFAULT_STAFF_SHEET_NAME, MAX_SHEET_NAME_CHARS,
    normalize_sheet_name,
};
pub use slug::slug_taken_by_other;
pub use ticket_note::{MAX_TICKET_NOTE_CHARS, normalize_ticket_note};
