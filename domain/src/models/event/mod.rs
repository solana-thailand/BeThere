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
mod requests;
mod responses;
mod sheet_name;

#[cfg(test)]
mod tests;

pub use config::{CommunityLink, EventConfig, EventIndex, EventMeta};
pub use enums::{EscrowStatus, EventFormat, EventStatus, EventVisibility, OnlineOpenMode};
pub use form::{FormFieldConfig, FormFieldType, RegistrationFormConfig};
pub use requests::{CreateEventRequest, DuplicateEventRequest, UpdateEventRequest};
pub use responses::{
    CreateEventResponse, EventDetailResponse, EventListResponse, UpdateEventResponse,
};
pub use sheet_name::{
    DEFAULT_ATTENDEE_SHEET_NAME, DEFAULT_STAFF_SHEET_NAME, MAX_SHEET_NAME_CHARS,
    normalize_sheet_name,
};
