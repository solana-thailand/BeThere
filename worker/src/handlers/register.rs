//! Self-registration handler for public event sign-up.
//
//! POST /api/public/register — allows attendees to register from the public event page.
//! GET /api/my-registration/:slug — returns attendee info for the authenticated user.
//!
//! Validates input, checks for duplicates, appends to Google Sheet, returns next step.

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::{EventFormat, EventStatus};

use crate::error::ApiOk;
use crate::sheets;
use crate::state::AppState;

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

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/public/register
///
/// Self-registration endpoint — requires JWT identity (verified email).
/// Email is taken from JWT claims, not the request body.
/// Flow: validate → resolve event → check status → dedup email → append to sheet → return next step.
#[worker::send]
pub async fn register_attendee(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<RegisterRequest>,
) -> Result<ApiOk<RegisterResponse>, crate::error::WorkerError> {
    // 1. Validate input
    let name = body.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::Validation("name is required (max 100 chars)".to_string()).into());
    }

    // Email comes from JWT, not request body — ensures verified identity
    let email = claims.email.trim().to_lowercase();
    tracing::info!("registration email from JWT: {email}");

    let slug = body.slug.trim();
    if slug.is_empty() {
        return Err(AppError::Validation("event slug is required".to_string()).into());
    }

    // 2. Resolve event by slug from KV
    let kv = state.events_kv.as_ref().ok_or_else(|| {
        tracing::error!("EVENTS KV namespace not configured");
        AppError::Internal("EVENTS KV namespace not configured".to_string())
    })?;

    let index = crate::event_store::get_event_index(kv)
        .await
        .map_err(AppError::Internal)?;

    let event_id = index
        .events
        .iter()
        .find(|e| e.slug == slug)
        .map(|e| e.id.clone())
        .ok_or_else(|| AppError::NotFound(format!("event '{slug}' not found")))?;

    let config = crate::event_store::get_event_config(kv, &event_id)
        .await
        .map_err(AppError::Internal)?;

    let config = config.ok_or_else(|| AppError::NotFound(format!("event '{slug}' not found")))?;

    // 3. Check event is Active
    if config.status != EventStatus::Active {
        return Err(
            AppError::Validation("registration is not open for this event".to_string()).into(),
        );
    }

    // 3b. Validate contact info if required by event
    let contact_channel = body
        .contact_channel
        .as_deref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty());
    let contact_handle = body
        .contact_handle
        .as_deref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty());

    if config.require_contact_info {
        if contact_channel.is_none() {
            return Err(AppError::Validation(
                "please select a preferred contact channel".to_string(),
            )
            .into());
        }
        if contact_handle.is_none() {
            return Err(AppError::Validation(
                "please provide your contact username or profile link".to_string(),
            )
            .into());
        }
    }

    // 3c. Validate deposit agreement if deposit is enabled
    if config.deposit_enabled && body.deposit_agreed != Some(true) {
        return Err(AppError::Validation(
            "you must agree to the deposit commitment to register".to_string(),
        )
        .into());
    }

    // 4. Determine participation_type
    let participation_type =
        resolve_participation_type(&config.event_format, body.participation_type.as_deref())?;

    // 5. Check for duplicate email in the Google Sheet
    let attendees = sheets::get_attendees(&state, &config.sheet_id, &config.sheet_name, Some(kv))
        .await
        .map_err(|e| {
            tracing::warn!(error = ?e, "could not fetch attendees for dedup");
            AppError::Internal(format!("failed to check existing registrations: {e}"))
        })?;

    // Duplicate email check: if already registered, return existing attendee info
    // so the frontend can redirect to the correct step (deposit/ticket) instead of
    // showing an error. This handles the case where localStorage is cleared or the
    // attendee uses a different device.
    if let Some(existing) = attendees.iter().find(|a| a.email.to_lowercase() == email) {
        tracing::info!(%email, %slug, "registration duplicate — returning existing attendee");
        let claim_token = existing.claim_token.clone().unwrap_or_default();
        let next_step = build_next_step(
            &config.event_format,
            &event_id,
            &existing.api_id,
            &claim_token,
            &state,
        );
        return Ok(ApiOk::new(RegisterResponse {
            attendee_id: existing.api_id.clone(),
            name: existing.name.clone(),
            email: existing.email.clone(),
            claim_token,
            next_step,
        }));
    }

    // 6. Generate IDs
    let api_id = Uuid::now_v7().to_string();
    let claim_token = Uuid::now_v7().to_string();

    // 7. Split name into first_name / last_name
    let (first_name, last_name) = split_name(name);

    let now = chrono::Utc::now().to_rfc3339();

    // 8. Resolve column mapping
    let mapping = match sheets::get_column_mapping(
        &state,
        &config.sheet_id,
        &config.sheet_name,
        Some(kv),
    )
    .await
    {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "failed to get column mapping, using hardcoded fallback");
            event_checkin_domain::models::attendee::ColumnMapping::hardcoded()
        }
    };

    // 9. Append row to Google Sheet
    sheets::append_attendee_row(
        &api_id,
        name,
        &first_name,
        &last_name,
        &email,
        &claim_token,
        &participation_type,
        &now,
        contact_channel,
        contact_handle,
        body.deposit_agreed.unwrap_or(false),
        &mapping,
        &state,
        &config.sheet_id,
        &config.sheet_name,
        Some(kv),
    )
    .await
    .map_err(|e| {
        tracing::error!(%email, error = ?e, "failed to append registration row");
        AppError::Internal(format!("failed to register: {e}"))
    })?;

    // 10. Determine next_step based on event format
    let next_step = build_next_step(
        &config.event_format,
        &event_id,
        &api_id,
        &claim_token,
        &state,
    );

    tracing::info!(
        %api_id,
        %email,
        %slug,
        %participation_type,
        "attendee self-registered"
    );

    Ok(ApiOk::new(RegisterResponse {
        attendee_id: api_id,
        name: name.to_string(),
        email,
        claim_token,
        next_step,
    }))
}

/// GET /api/my-registration/:slug
///
/// Returns the authenticated attendee's registration for a given event slug.
/// Uses JWT identity (claims.email) to find the matching attendee.
#[worker::send]
pub async fn my_registration(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(slug): Path<String>,
) -> Result<ApiOk<MyRegistrationResponse>, crate::error::WorkerError> {
    let slug = slug.trim();
    if slug.is_empty() {
        return Err(AppError::Validation("event slug is required".to_string()).into());
    }

    // Resolve event by slug from KV
    let kv = state.events_kv.as_ref().ok_or_else(|| {
        tracing::error!("EVENTS KV namespace not configured");
        AppError::Internal("EVENTS KV namespace not configured".to_string())
    })?;

    let index = crate::event_store::get_event_index(kv)
        .await
        .map_err(AppError::Internal)?;

    let event_entry = index
        .events
        .iter()
        .find(|e| e.slug == slug)
        .ok_or_else(|| AppError::NotFound(format!("event '{slug}' not found")))?;

    let config = crate::event_store::get_event_config(kv, &event_entry.id)
        .await
        .map_err(AppError::Internal)?;

    let config = config.ok_or_else(|| AppError::NotFound(format!("event '{slug}' not found")))?;

    // Fetch attendees and find by email (case-insensitive)
    let attendees = sheets::get_attendees(&state, &config.sheet_id, &config.sheet_name, Some(kv))
        .await
        .map_err(|e| {
            tracing::warn!(error = ?e, "could not fetch attendees");
            AppError::Internal(format!("failed to fetch attendees: {e}"))
        })?;

    let attendee = attendees
        .iter()
        .find(|a| a.email.eq_ignore_ascii_case(&claims.email))
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "no registration found for {} at event '{slug}'",
                claims.email
            ))
        })?;

    let claim_token = attendee.claim_token.clone().unwrap_or_default();
    let next_step = build_next_step(
        &config.event_format,
        &event_entry.id,
        &attendee.api_id,
        &claim_token,
        &state,
    );

    tracing::info!(
        email = %claims.email,
        slug = %slug,
        attendee_id = %attendee.api_id,
        "my-registration lookup successful"
    );

    Ok(ApiOk::new(MyRegistrationResponse {
        attendee_id: attendee.api_id.clone(),
        name: attendee.name.clone(),
        email: attendee.email.clone(),
        claim_token,
        participation_type: attendee.participation_type.clone(),
        next_step,
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve participation type based on event format and user selection.
fn resolve_participation_type(
    format: &EventFormat,
    user_choice: Option<&str>,
) -> Result<String, AppError> {
    match format {
        EventFormat::InPerson => Ok("In-Person".to_string()),
        EventFormat::Online => Ok("Online".to_string()),
        EventFormat::Hybrid => match user_choice.map(|v| v.trim()) {
            Some(choice) if !choice.is_empty() => Ok(choice.to_string()),
            _ => Ok("In-Person".to_string()),
        },
    }
}

/// Split a full name into (first_name, last_name).
/// First word → first_name, rest → last_name.
fn split_name(name: &str) -> (String, String) {
    let parts: Vec<&str> = name.split_whitespace().collect();
    match parts.as_slice() {
        [] => (String::new(), String::new()),
        [only] => (only.to_string(), String::new()),
        [first, rest @ ..] => (first.to_string(), rest.join(" ")),
    }
}

// ---------------------------------------------------------------------------
// GET /api/my-registrations — all registrations for the signed-in user
// ---------------------------------------------------------------------------

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
    pub next_step: NextStep,
}

/// GET /api/my-registrations
///
/// Returns all registrations for the authenticated user across all events.
/// Iterates active events from KV index, checks each event's attendee list for the JWT email.
/// Uses KV-cached attendee data so this is efficient.
#[worker::send]
pub async fn my_registrations(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<Vec<MyRegistrationsItem>>, crate::error::WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    let index = crate::event_store::get_event_index(kv)
        .await
        .map_err(AppError::Internal)?;

    let mut results: Vec<MyRegistrationsItem> = Vec::new();

    for event_entry in &index.events {
        // Skip completed/archived events
        if matches!(
            event_entry.status,
            EventStatus::Completed | EventStatus::Archived
        ) {
            continue;
        }

        // Load event config to get sheet info
        let config = match crate::event_store::get_event_config(kv, &event_entry.id).await {
            Ok(Some(c)) => c,
            _ => continue,
        };

        // Fetch attendees for this event (KV-cached)
        let attendees =
            match sheets::get_attendees(&state, &config.sheet_id, &config.sheet_name, Some(kv))
                .await
            {
                Ok(a) => a,
                Err(e) => {
                    tracing::warn!(
                        event_id = %event_entry.id,
                        error = %e,
                        "my-registrations: failed to fetch attendees, skipping"
                    );
                    continue;
                }
            };

        // Find attendee matching JWT email
        if let Some(attendee) = attendees
            .iter()
            .find(|a| a.email.eq_ignore_ascii_case(&claims.email))
        {
            let claim_token = attendee.claim_token.clone().unwrap_or_default();
            let next_step = build_next_step(
                &config.event_format,
                &event_entry.id,
                &attendee.api_id,
                &claim_token,
                &state,
            );

            results.push(MyRegistrationsItem {
                event_id: event_entry.id.clone(),
                event_name: event_entry.name.clone(),
                event_slug: event_entry.slug.clone(),
                event_start_ms: event_entry.event_start_ms,
                attendee_id: attendee.api_id.clone(),
                name: attendee.name.clone(),
                participation_type: attendee.participation_type.clone(),
                next_step,
            });
        }
    }

    tracing::info!(
        email = %claims.email,
        count = results.len(),
        "my-registrations lookup complete"
    );

    Ok(ApiOk::new(results))
}

/// Build the next_step response based on event format.
fn build_next_step(
    format: &EventFormat,
    event_id: &str,
    api_id: &str,
    claim_token: &str,
    state: &AppState,
) -> NextStep {
    let claim_base = &state.config.server.claim_base_url;

    if format.has_in_person() {
        NextStep {
            step_type: "deposit".to_string(),
            url: format!("/deposit/{api_id}?event_id={event_id}"),
        }
    } else {
        NextStep {
            step_type: "quest".to_string(),
            url: format!("{claim_base}/{claim_token}"),
        }
    }
}
