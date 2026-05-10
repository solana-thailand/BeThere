//! Self-registration handler for public event sign-up.
//!
//! POST /api/public/register — allows attendees to register from the public event page.
//! Validates input, checks for duplicates, appends to Google Sheet, returns next step.

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub email: String,
    /// Optional for InPerson/Online events. Required for Hybrid to choose track.
    /// Defaults based on event format if omitted.
    pub participation_type: Option<String>,
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
// Handler
// ---------------------------------------------------------------------------

/// POST /api/public/register
///
/// Public self-registration endpoint — no auth required.
/// Flow: validate → resolve event → check status → dedup email → append to sheet → return next step.
#[worker::send]
pub async fn register_attendee(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<ApiOk<RegisterResponse>, crate::error::WorkerError> {
    // 1. Validate input
    let name = body.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::Validation("name is required (max 100 chars)".to_string()).into());
    }

    let email = body.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') || !email.contains('.') || email.len() > 254 {
        return Err(AppError::Validation("a valid email address is required".to_string()).into());
    }

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

    if attendees.iter().any(|a| a.email.to_lowercase() == email) {
        tracing::info!(%email, %slug, "registration duplicate");
        return Err(AppError::Validation(
            "this email is already registered for this event".to_string(),
        )
        .into());
    }

    // 6. Generate IDs
    let api_id = Uuid::now_v7().to_string();
    let claim_token = Uuid::now_v7().to_string();

    // 7. Split name into first_name / last_name
    let (first_name, last_name) = split_name(name);

    let now = chrono::Utc::now().to_rfc3339();

    // 8. Append row to Google Sheet
    sheets::append_attendee_row(
        &api_id,
        name,
        &first_name,
        &last_name,
        &email,
        &claim_token,
        &participation_type,
        &now,
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

    // 9. Determine next_step based on event format
    let next_step = build_next_step(&config.event_format, &api_id, &claim_token, &state);

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

/// Build the next_step response based on event format.
fn build_next_step(
    format: &EventFormat,
    api_id: &str,
    claim_token: &str,
    state: &AppState,
) -> NextStep {
    let claim_base = &state.config.server.claim_base_url;

    if format.has_in_person() {
        NextStep {
            step_type: "deposit".to_string(),
            url: format!("/deposit?attendee_id={api_id}"),
        }
    } else {
        NextStep {
            step_type: "quest".to_string(),
            url: format!("{claim_base}/{claim_token}"),
        }
    }
}
