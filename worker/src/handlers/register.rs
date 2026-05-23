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
use futures::future::join_all;
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

    // 3c. Determine participation_type early (needed before deposit check)
    let participation_type =
        resolve_participation_type(&config.event_format, body.participation_type.as_deref())?;

    // 3d. Validate deposit agreement if deposit is enabled — skip for Online attendees
    if config.deposit_enabled
        && !is_online_participation(&participation_type)
        && body.deposit_agreed != Some(true)
    {
        return Err(AppError::Validation(
            "you must agree to the deposit commitment to register".to_string(),
        )
        .into());
    }

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
        // Fetch deposit status if KV is available
        let deposit = if let Some(ref kv) = state.events_kv {
            crate::event_store::get_deposit_status(kv, &event_id, &existing.api_id)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        // Check if deposit deadline expired — attendee may have been auto-switched to Online
        let deadline_expired = deposit.is_none()
            && config.deposit_deadline_hours.is_some()
            && existing.is_in_person()
            && existing.registration_date.as_ref().is_some_and(|reg_str| {
                if let Ok(reg_time) = chrono::DateTime::parse_from_rfc3339(reg_str) {
                    let deadline = reg_time.with_timezone(&chrono::Utc)
                        + chrono::Duration::hours(i64::from(
                            config.deposit_deadline_hours.unwrap_or(0),
                        ));
                    chrono::Utc::now() > deadline
                } else {
                    false
                }
            });

        let next_step = if deadline_expired {
            // Deadline expired — check if reclaim is possible
            let capacity_available = if let Some(cap) = config.in_person_capacity {
                let in_person_count = attendees.iter().filter(|a| a.is_in_person()).count() as u32;
                in_person_count < cap
            } else {
                true // No capacity limit = reclaim available
            };

            if capacity_available && deposit.is_none() {
                // Reclaim: send to deposit page — the deposit handler will
                // switch participation_type back to In-Person
                NextStep {
                    step_type: "deposit".to_string(),
                    url: format!("/deposit/{}?event_id={}", existing.api_id, event_id),
                }
            } else {
                // Capacity full or already deposited — online track
                NextStep {
                    step_type: "waiting".to_string(),
                    url: format!("/ticket/{}?event_id={}", existing.api_id, event_id),
                }
            }
        } else {
            build_next_step(
                &config.event_format,
                &event_id,
                &existing.api_id,
                &claim_token,
                &state,
                deposit.as_ref(),
                &existing.participation_type,
            )
        };
        return Ok(ApiOk::new(RegisterResponse {
            attendee_id: existing.api_id.clone(),
            name: existing.name.clone(),
            email: existing.email.clone(),
            claim_token,
            next_step,
        }));
    }

    // 5b. Enforce capacity limits (only for new registrations)
    enforce_capacity(&state, &config, &participation_type, kv).await?;

    // 5c. Check if attendee has rolling deposit credit that covers this event's deposit
    let mut credit_covered_method: Option<String> = None;
    if config.deposit_enabled && !is_online_participation(&participation_type) {
        let resolved_contacts =
            crate::org_store::resolve_contacts_sheet(kv, &config, &state.config.sheets).await;
        if !resolved_contacts.sheet_id.is_empty() {
            if let Ok((credit_thb, credit_usdc)) = crate::sheets::contacts::get_credit_balance(
                &state,
                &resolved_contacts.sheet_id,
                &resolved_contacts.contacts_sheet_name,
                Some(kv),
                &email,
            )
            .await
            {
                let required_thb = config.deposit_amount_thb;
                let required_usdc = config.deposit_amount_usdc;
                if required_thb > 0 && credit_thb >= required_thb {
                    credit_covered_method = Some("credit_thb".to_string());
                } else if required_usdc > 0 && credit_usdc >= required_usdc {
                    credit_covered_method = Some("credit_usdc".to_string());
                }
            }
        }
        if let Some(ref method) = credit_covered_method {
            tracing::info!(%email, %slug, %method, "deposit covered by rolling credit");
        }
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

    // 9b. Upsert to master contacts sheet (non-fatal)
    upsert_contact_after_registration(
        &email,
        name,
        &event_id,
        contact_channel,
        contact_handle,
        &state,
        &config,
        Some(kv),
    )
    .await;

    // 10. Determine next_step based on event format and participation type
    // 10a. If deposit was covered by rolling credit, skip deposit page
    //      and write credit method to the attendee's deposit_method column
    let next_step = if let Some(ref method) = credit_covered_method {
        // Write deposit_method to the attendee row (update column N)
        if let Err(e) = sheets::update_deposit_method(
            &state,
            &config.sheet_id,
            &config.sheet_name,
            Some(kv),
            &api_id,
            method,
        )
        .await
        {
            tracing::warn!(%api_id, error = %e, "failed to write credit deposit_method to sheet");
        }
        NextStep {
            step_type: "ticket".to_string(),
            url: format!("/ticket/{api_id}?event_id={event_id}"),
        }
    } else {
        build_next_step(
            &config.event_format,
            &event_id,
            &api_id,
            &claim_token,
            &state,
            None,
            &participation_type,
        )
    };

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
    // Fetch deposit status
    let deposit = if let Some(ref kv) = state.events_kv {
        crate::event_store::get_deposit_status(kv, &event_entry.id, &attendee.api_id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };
    let next_step = build_next_step(
        &config.event_format,
        &event_entry.id,
        &attendee.api_id,
        &claim_token,
        &state,
        deposit.as_ref(),
        &attendee.participation_type,
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
    /// Human-readable status: "registered", "deposit pending", "deposit confirmed",
    /// "checked in", "nft claimed".
    pub status: String,
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
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?
        .clone();

    let index = crate::event_store::get_event_index(&kv)
        .await
        .map_err(AppError::Internal)?;

    // Process events concurrently (config + attendee loads are independent per-event)
    let event_futures: Vec<_> = index
        .events
        .iter()
        .filter(|e| !matches!(e.status, EventStatus::Completed | EventStatus::Archived))
        .map(|event_entry| {
            let state = state.clone();
            let kv = kv.clone();
            let email = claims.email.clone();
            let event_entry = event_entry.clone();
            async move {
                // Load event config
                let config = match crate::event_store::get_event_config(&kv, &event_entry.id).await
                {
                    Ok(Some(c)) => c,
                    _ => return None,
                };

                // Fetch attendees (KV-cached)
                let attendees = match crate::sheets::get_attendees(
                    &state,
                    &config.sheet_id,
                    &config.sheet_name,
                    Some(&kv),
                )
                .await
                {
                    Ok(a) => a,
                    Err(e) => {
                        tracing::warn!(
                            event_id = %event_entry.id,
                            error = %e,
                            "my-registrations: failed to fetch attendees, skipping"
                        );
                        return None;
                    }
                };

                // Find attendee matching JWT email
                let attendee = attendees
                    .iter()
                    .find(|a| a.email.eq_ignore_ascii_case(&email))?;

                let claim_token = attendee.claim_token.clone().unwrap_or_default();
                // Fetch deposit status for this attendee
                let deposit =
                    crate::event_store::get_deposit_status(&kv, &event_entry.id, &attendee.api_id)
                        .await
                        .ok()
                        .flatten();
                let next_step = build_next_step(
                    &config.event_format,
                    &event_entry.id,
                    &attendee.api_id,
                    &claim_token,
                    &state,
                    deposit.as_ref(),
                    &attendee.participation_type,
                );

                let status = if attendee.claimed_at.as_ref().is_some_and(|s| !s.is_empty()) {
                    "nft claimed".to_string()
                } else if attendee
                    .checked_in_at
                    .as_ref()
                    .is_some_and(|s| !s.is_empty())
                {
                    "checked in".to_string()
                } else if attendee
                    .deposit_verified
                    .as_ref()
                    .is_some_and(|s| !s.is_empty())
                {
                    "deposit confirmed".to_string()
                } else if attendee
                    .deposit_method
                    .as_ref()
                    .is_some_and(|s| !s.is_empty())
                {
                    "deposit pending".to_string()
                } else {
                    "registered".to_string()
                };

                Some(MyRegistrationsItem {
                    event_id: event_entry.id.clone(),
                    event_name: event_entry.name.clone(),
                    event_slug: event_entry.slug.clone(),
                    event_start_ms: event_entry.event_start_ms,
                    attendee_id: attendee.api_id.clone(),
                    name: attendee.name.clone(),
                    participation_type: attendee.participation_type.clone(),
                    status,
                    next_step,
                })
            }
        })
        .collect();

    let results: Vec<MyRegistrationsItem> = join_all(event_futures)
        .await
        .into_iter()
        .flatten()
        .collect();

    tracing::info!(
        email = %claims.email,
        count = results.len(),
        "my-registrations lookup complete"
    );

    Ok(ApiOk::new(results))
}

/// Build the next_step response based on event format and deposit status.
///
/// Logic:
/// - Online-only events → quest/claim page
/// - In-person/hybrid events:
///   - No deposit yet → deposit page
///   - THB deposit pending verification → ticket page (info view, no QR)
///   - Deposit verified (USDC or THB) → ticket page
fn is_online_participation(participation_type: &str) -> bool {
    let lower = participation_type.trim().to_lowercase();
    lower.contains("online") || lower.contains("virtual")
}

fn build_next_step(
    format: &EventFormat,
    event_id: &str,
    api_id: &str,
    _claim_token: &str,
    state: &AppState,
    deposit: Option<&event_checkin_domain::models::deposit::DepositStatus>,
    participation_type: &str,
) -> NextStep {
    let _claim_base = &state.config.server.claim_base_url;

    // Online attendees never need deposit — skip straight to waiting/ticket.
    // Quest completion (quiz/adventure) serves as virtual check-in at claim time.
    if is_online_participation(participation_type) {
        return NextStep {
            step_type: "waiting".to_string(),
            url: format!("/ticket/{api_id}?event_id={event_id}"),
        };
    }

    if format.has_in_person() {
        match deposit {
            Some(_) => {
                // Deposit exists (verified or pending) — show ticket page
                NextStep {
                    step_type: "ticket".to_string(),
                    url: format!("/ticket/{api_id}?event_id={event_id}"),
                }
            }
            None => {
                // No deposit yet — go to deposit page
                NextStep {
                    step_type: "deposit".to_string(),
                    url: format!("/deposit/{api_id}?event_id={event_id}"),
                }
            }
        }
    } else {
        // Online-only event format (shouldn't reach here for in-person attendees,
        // but kept as fallback)
        NextStep {
            step_type: "waiting".to_string(),
            url: format!("/ticket/{api_id}?event_id={event_id}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Capacity enforcement
// ---------------------------------------------------------------------------

/// Enforce capacity limits before registration.
/// Returns an error if the selected track is full or not available.
async fn enforce_capacity(
    state: &AppState,
    config: &event_checkin_domain::models::event::EventConfig,
    participation_type: &str,
    kv: &worker::kv::KvStore,
) -> Result<(), AppError> {
    use event_checkin_domain::models::event::OnlineOpenMode;

    let is_in_person = participation_type
        .trim()
        .to_lowercase()
        .contains("in-person")
        || participation_type
            .trim()
            .to_lowercase()
            .contains("in person")
        || participation_type.trim().is_empty();

    // Count current attendees from sheet
    let attendees =
        crate::sheets::get_attendees(state, &config.sheet_id, &config.sheet_name, Some(kv))
            .await
            .map_err(|e| AppError::Internal(format!("failed to check capacity: {e}")))?;

    let mut in_person_count: u32 = 0;
    let mut online_count: u32 = 0;
    for a in &attendees {
        if a.is_in_person() {
            in_person_count += 1;
        } else {
            online_count += 1;
        }
    }

    // Count UNSYNCED walk-in attendees as in-person (avoid double-counting with sheet)
    let walkin_prefix = format!("walkin:{}:", config.id);
    let mut walkin_cursor: Option<String> = None;
    let mut walkin_count: u32 = 0;
    loop {
        let mut builder = kv.list().prefix(walkin_prefix.clone());
        if let Some(c) = walkin_cursor.take() {
            builder = builder.cursor(c);
        }
        match builder.execute().await {
            Ok(resp) => {
                for key in &resp.keys {
                    let email = key.name.strip_prefix(&walkin_prefix).unwrap_or("");
                    let sync_key = format!("walkin_synced:{}:{}", config.id, email);
                    let synced: Option<bool> = kv.get(&sync_key).json().await.ok().flatten();
                    if synced != Some(true) {
                        walkin_count += 1;
                    }
                }
                if resp.list_complete {
                    break;
                }
                walkin_cursor = resp.cursor;
            }
            Err(e) => {
                tracing::warn!(error = ?e, "failed to list walk-in keys for capacity");
                break;
            }
        }
    }
    in_person_count += walkin_count;

    tracing::info!(
        event_id = %config.id,
        participation_type = %participation_type,
        is_in_person = is_in_person,
        in_person_count = in_person_count,
        online_count = online_count,
        in_person_capacity = ?config.in_person_capacity,
        online_capacity = ?config.online_capacity,
        "capacity check"
    );

    if is_in_person {
        // Check in-person capacity
        if let Some(cap) = config.in_person_capacity
            && in_person_count >= cap
        {
            return Err(AppError::Validation(
                "In-person spots are full. Please register for the online track instead."
                    .to_string(),
            ));
        }
    } else {
        // Check online capacity
        if let Some(cap) = config.online_capacity
            && online_count >= cap
        {
            return Err(AppError::Validation(
                "Online spots are full. Registration is closed.".to_string(),
            ));
        }

        // Check online registration gating
        let in_person_available = config
            .in_person_capacity
            .is_none_or(|cap| in_person_count < cap);

        let online_open = match config.online_open_mode {
            OnlineOpenMode::Always => true,
            OnlineOpenMode::AutoOnFull => !in_person_available,
            OnlineOpenMode::Manual => config.online_registration_open,
        };

        if !online_open {
            return Err(AppError::Validation(
                "Online registration is not open yet. Please check back later or register for the in-person track.".to_string(),
            ));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Contacts upsert helper
// ---------------------------------------------------------------------------

/// Non-fatal upsert to the master contacts sheet after successful registration.
/// Errors are logged but never block the registration response.
#[allow(clippy::too_many_arguments)]
async fn upsert_contact_after_registration(
    email: &str,
    name: &str,
    event_id: &str,
    contact_channel: Option<&str>,
    contact_handle: Option<&str>,
    state: &AppState,
    event_config: &event_checkin_domain::models::event::EventConfig,
    kv: Option<&worker::KvStore>,
) {
    // Resolve the contacts sheet from the event's organization
    let resolved = match kv {
        Some(kv_store) => {
            crate::org_store::resolve_contacts_sheet(kv_store, event_config, &state.config.sheets)
                .await
        }
        None => {
            // No KV — fall back to global config
            let global = &state.config.sheets;
            event_checkin_domain::models::org::ResolvedContactsSheet {
                sheet_id: global.contacts_sheet_id.clone(),
                contacts_sheet_name: global.contacts_sheet_name.clone(),
                events_sheet_name: global.events_sheet_name.clone(),
            }
        }
    };

    if resolved.sheet_id.is_empty() {
        return; // Not configured — skip silently
    }

    let upsert = crate::sheets::contacts::ContactUpsert {
        email,
        name,
        event_id,
        contact_channel,
        contact_handle,
    };

    if let Err(e) = crate::sheets::contacts::upsert_contact(
        &upsert,
        state,
        &resolved.sheet_id,
        &resolved.contacts_sheet_name,
        kv,
    )
    .await
    {
        tracing::warn!(
            %email,
            %event_id,
            error = %e,
            "failed to upsert contact to master sheet (non-fatal)"
        );
    }
}
