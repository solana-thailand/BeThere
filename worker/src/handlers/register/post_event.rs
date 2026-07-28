//! Post-event registration (Plan 008 — Phase 3 §3.3.2)
//!
//! Lead capture for completed events: a visitor who missed the event signs in
//! with Google (JWT) and submits a stripped registration (name + contact +
//! developer-profile fields). No deposit, no check-in, no NFT — they become a
//! `post_event_registered` attendee row + a `contacts` / `developer_profiles`
//! entry for future outreach.

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use uuid::Uuid;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::EventStatus;

use crate::error::ApiOk;
use crate::state::AppState;

use super::contact::write_developer_data;
use super::types::{DeveloperData, PostEventRegisterRequest};

/// `POST /api/public/event/{slug}/register-post-event` — public lead capture.
///
/// JWT-required (same `require_identity` middleware as normal registration) so
/// every lead has a verified Google email. Validates the event is Completed with
/// post-event registration open and the deadline (if any) not expired, then
/// writes a `post_event_registered` attendee row + contact + developer profile.
#[worker::send]
pub async fn register_post_event(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(slug): Path<String>,
    Json(body): Json<PostEventRegisterRequest>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    // 1. Validate input
    let name = body.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::Validation("name is required (max 100 chars)".to_string()).into());
    }

    // PDPA consent is always required.
    if body.consent_given != Some(true) {
        return Err(AppError::Validation(
            "you must consent to data collection to register".to_string(),
        )
        .into());
    }

    let email = claims.email.trim().to_lowercase();

    let slug = slug.trim();
    if slug.is_empty() {
        return Err(AppError::Validation("event slug is required".to_string()).into());
    }

    // 2. Resolve event by slug (KV → D1 fallback)
    let kv = state.events_kv.as_ref();
    let config = crate::event_store::resolve_event_by_slug(kv, slug, state.d1.as_deref())
        .await
        .map_err(AppError::NotFound)?;
    let event_id = config.id.clone();

    // 3. Validate lifecycle: must be Completed (active events use normal reg).
    if config.status != EventStatus::Completed {
        return Err(AppError::Conflict(format!(
            "post-event registration is only available for completed events (current: {})",
            config.status.as_str()
        ))
        .into());
    }

    // 4. Validate the organizer has opened post-event registration.
    if !config.post_event_registration_open {
        return Err(AppError::Conflict(
            "post-event registration is not open for this event".to_string(),
        )
        .into());
    }

    // 5. Validate the deadline (if set) has not passed.
    if let Some(until) = config.post_event_registration_until_ms {
        let now_ms = chrono::Utc::now().timestamp_millis();
        if now_ms >= until {
            return Err(AppError::Gone(
                "post-event registration for this event has closed".to_string(),
            )
            .into());
        }
    }

    let contact_channel = body
        .contact_channel
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let contact_handle = body
        .contact_handle
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    // 6. Write to D1 (source of truth). Post-event registrants are leads, not
    //    attendees: `post_event_registered` status + `online` placeholder keeps
    //    them out of capacity / check-in / claim queries without a separate table.
    if let Some(ref d1) = state.d1 {
        let api_id = Uuid::now_v7().to_string();

        let attendee_fut = crate::db::attendees::upsert_post_event_attendee(
            d1,
            &api_id,
            &event_id,
            &email,
            name,
            "online", // placeholder — not used for capacity
            contact_channel.unwrap_or(""),
            contact_handle.unwrap_or(""),
            body.consent_marketing,
        );

        let events_joined = event_id.clone();
        let contact_fut = crate::db::contacts::upsert_contact(
            d1,
            &email,
            name,
            &events_joined,
            1,
            contact_channel.unwrap_or(""),
            contact_handle.unwrap_or(""),
        );

        let (attendee_result, contact_result) = futures_util::join!(attendee_fut, contact_fut);

        if let Err(e) = attendee_result {
            tracing::warn!(%api_id, %email, error = %e, "D1 post-event attendee upsert failed (non-fatal)");
        }
        if let Err(e) = contact_result {
            tracing::warn!(%email, error = %e, "D1 post-event contact upsert failed (non-fatal)");
        }

        // Developer profile + registration responses (the primary value of lead capture).
        let mut profile_fields: Vec<(String, String)> = Vec::new();
        if let Some(ref v) = body.experience_level {
            profile_fields.push(("experience_level".to_string(), v.clone()));
        }
        if let Some(ref v) = body.tech_stack {
            profile_fields.push(("tech_stack".to_string(), v.clone()));
        }
        if let Some(ref v) = body.interests {
            profile_fields.push(("interests".to_string(), v.clone()));
        }
        if let Some(ref fields) = body.profile_fields {
            for (key, value) in fields {
                if !value.is_empty() && !profile_fields.iter().any(|(k, _)| k == key) {
                    profile_fields.push((key.clone(), value.clone()));
                }
            }
        }

        write_developer_data(&DeveloperData {
            d1,
            email: &email,
            name,
            event_id: &event_id,
            contact_channel: contact_channel.unwrap_or(""),
            contact_handle: contact_handle.unwrap_or(""),
            participation_type: "online",
            consent_given: body.consent_given.unwrap_or(false),
            photo_consent_given: false,
            consent_marketing: body.consent_marketing.unwrap_or(false),
            profile_fields,
        })
        .await;

        tracing::info!(%email, %event_id, %api_id, "post-event registration captured");

        return Ok(ApiOk::new(serde_json::json!({
            "attendee_id": api_id,
            "message": "Thanks! We'll notify you about future events.",
        })));
    }

    // D1 unavailable — cannot persist a lead without the canonical store.
    Err(AppError::Internal("D1 is required for post-event registration".to_string()).into())
}
