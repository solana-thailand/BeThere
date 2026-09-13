//! API response bodies for the event endpoints.

use serde::{Deserialize, Serialize};

use super::config::{EventConfig, EventMeta};

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
