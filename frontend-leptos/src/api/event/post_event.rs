//! Post-event registration (Plan 008 — Phase 3).

use serde::{Deserialize, Serialize};

use crate::api::types::ApiError;
use crate::api::{
    api_post_json, api_put_json,
};


// ===== Post-Event Registration (Plan 008 — Phase 3) =====

/// Body for `POST /api/public/event/{slug}/register-post-event` — the stripped
/// lead-capture form (no deposit/participation fields).
#[derive(Debug, Clone, Serialize, Default)]
pub struct PostEventRegisterBody {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_handle: Option<String>,
    pub consent_given: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consent_marketing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experience_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech_stack: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interests: Option<String>,
}

/// Success response from post-event registration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PostEventRegisterData {
    pub attendee_id: String,
    pub message: String,
}

/// `POST /api/public/event/{slug}/register-post-event` — submit the lead-capture
/// form. JWT-gated (verified email) on the backend.
pub async fn register_post_event(
    slug: &str,
    body: &PostEventRegisterBody,
) -> Result<PostEventRegisterData, ApiError> {
    let path = format!("/public/event/{slug}/register-post-event");
    api_post_json(&path, body).await
}

/// Body for `PUT /api/events/{id}/post-event-registration` — organizer toggle.
#[derive(Debug, Clone, Serialize)]
pub struct PutPostEventRegistrationBody {
    pub open: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until_ms: Option<i64>,
}

/// Response from the toggle endpoint.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PostEventRegistrationState {
    pub open: bool,
    #[serde(default)]
    pub until_ms: Option<i64>,
}

/// `PUT /api/events/{id}/post-event-registration` — organizer opens/closes
/// post-event lead capture for a completed event.
pub async fn put_post_event_registration(
    id: &str,
    body: &PutPostEventRegistrationBody,
) -> Result<PostEventRegistrationState, ApiError> {
    let path = format!("/events/{id}/post-event-registration");
    api_put_json(&path, body).await
}
