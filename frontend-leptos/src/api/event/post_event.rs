//! Post-event registration (Plan 008 — Phase 3).

use serde::{Deserialize, Serialize};

use crate::api::types::ApiError;
use crate::api::{api_get_json, api_post_json, api_put_json};

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
    /// Free-form per-event answers, keyed by `field_key`.
    ///
    /// The Worker has accepted this since Plan 008 Phase 3; nothing sent it
    /// until the post-event question set (.issues/087). Keys in the `post.`
    /// namespace are stored against the event in `registration_responses` and
    /// deliberately do **not** touch `developer_profiles` — see
    /// `db::developers::is_event_scoped_field` (.issues/082).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_fields: Option<std::collections::HashMap<String, String>>,
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

/// Body and response of `GET` / `PUT /api/events/{id}/meals` (.plans/045
/// R4.11): meals the organizer ordered, `None` for none entered.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MealsOrdered {
    pub ordered: Option<u32>,
}

pub async fn get_meals_ordered(id: &str) -> Result<MealsOrdered, ApiError> {
    api_get_json(&format!("/events/{id}/meals")).await
}

pub async fn put_meals_ordered(id: &str, body: &MealsOrdered) -> Result<MealsOrdered, ApiError> {
    api_put_json(&format!("/events/{id}/meals"), body).await
}

/// `GET /api/events/{id}/regular-rule[?min=N]` (Super GOAT SG1b): the rule and
/// how many registrants qualify (for `min`, a preview, else the saved rule).
pub async fn get_regular_rule(
    id: &str,
    min: Option<u32>,
) -> Result<event_checkin_domain::models::record::RegularRuleAdmin, ApiError> {
    let query = min.map(|n| format!("?min={n}")).unwrap_or_default();
    api_get_json(&format!("/events/{id}/regular-rule{query}")).await
}

/// `PUT /api/events/{id}/regular-rule`: save the rule, or `None` to remove it.
pub async fn put_regular_rule(
    id: &str,
    rule: Option<event_checkin_domain::models::record::RegularRule>,
) -> Result<event_checkin_domain::models::record::RegularRuleAdmin, ApiError> {
    api_put_json(
        &format!("/events/{id}/regular-rule"),
        &serde_json::json!({ "rule": rule }),
    )
    .await
}
