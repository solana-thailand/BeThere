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
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_checkin_domain::models::attendee::ParticipationType;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::{EventFormat, EventStatus};

use crate::error::ApiOk;
use crate::handlers::adventure::resolve_claim_token_from_d1;
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
    /// Whether the attendee consented to personal data collection (PDPA).
    /// Always required for registration.
    pub consent_given: Option<bool>,
    /// Whether the attendee consented to photo/media capture (PDPA).
    /// Required when event has `require_photo_consent` enabled.
    pub photo_consent_given: Option<bool>,
    /// Whether the attendee wants to receive marketing communications
    /// about future events (optional, PDPA marketing consent).
    pub consent_marketing: Option<bool>,
    /// Developer profile fields (optional — Issue #049 Phase 1, backward compat).
    pub experience_level: Option<String>,
    pub tech_stack: Option<String>,
    pub interests: Option<String>,
    /// Dynamic profile fields from configurable form (Issue #049 Phase 2).
    #[serde(default)]
    pub profile_fields: Option<std::collections::HashMap<String, String>>,
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

    // 2. Resolve event by slug (KV → D1 fallback)
    let kv = state.events_kv.as_ref();

    let config = crate::event_store::resolve_event_by_slug(kv, slug, state.d1.as_deref())
        .await
        .map_err(AppError::NotFound)?;

    let event_id = config.id.clone();

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
    // Returns canonical snake_case for D1; `*_display` is the display-case form
    // written to the Google Sheet (organizer-facing).
    let participation_type =
        resolve_participation_type(&config.event_format, body.participation_type.as_deref())?;
    let participation_type_display = ParticipationType::parse(&participation_type)
        .display()
        .to_string();

    // 3d. Validate PDPA consent — always required
    if body.consent_given != Some(true) {
        return Err(AppError::Validation(
            "you must consent to data collection to register".to_string(),
        )
        .into());
    }

    // 3d2. Validate photo consent if event requires it
    if config.require_photo_consent && body.photo_consent_given != Some(true) {
        return Err(AppError::Validation(
            "you must consent to photo/media capture to register".to_string(),
        )
        .into());
    }

    // 3e. Validate deposit agreement if deposit is enabled — skip for Online attendees
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
    let attendees = sheets::get_attendees_for_event(
        &state,
        &config.sheet_id,
        &config.sheet_name,
        kv,
        &config.id,
    )
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
        // Fetch deposit status (D1-first, KV fallback)
        let deposit = crate::event_store::get_deposit_status_with_fallback(
            state.events_kv.as_ref(),
            state.d1.as_deref(),
            &event_id,
            &existing.api_id,
        )
        .await
        .ok()
        .flatten();

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
            let is_checked_in = existing
                .checked_in_at
                .as_ref()
                .is_some_and(|s| !s.is_empty());
            let is_claimed = existing.claimed_at.as_ref().is_some_and(|s| !s.is_empty());
            build_next_step(
                &config.event_format,
                &event_id,
                &existing.api_id,
                &claim_token,
                &state,
                deposit.as_ref(),
                &existing.participation_type,
                is_checked_in,
                is_claimed,
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
        let resolved_contacts = if let Some(db) = state.d1.as_deref() {
            crate::org_store::resolve_contacts_sheet(db, &config, &state.config.sheets).await
        } else {
            event_checkin_domain::models::org::ResolvedContactsSheet {
                sheet_id: state.config.sheets.contacts_sheet_id.clone(),
                contacts_sheet_name: state.config.sheets.contacts_sheet_name.clone(),
                events_sheet_name: state.config.sheets.events_sheet_name.clone(),
            }
        };
        if !resolved_contacts.sheet_id.is_empty()
            && let Ok((credit_thb, credit_usdc)) = crate::sheets::contacts::get_credit_balance(
                &state,
                &resolved_contacts.sheet_id,
                &resolved_contacts.contacts_sheet_name,
                kv,
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
    let mapping = match sheets::get_column_mapping(&state, &config.sheet_id, &config.sheet_name, kv)
        .await
    {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "failed to get column mapping, using hardcoded fallback");
            event_checkin_domain::models::attendee::ColumnMapping::hardcoded()
        }
    };

    // 9. Write to D1 first (source of truth), then detach all Sheets writes

    // 9a. Dual-write to D1 — attendee row (non-fatal, Phase 2a)
    if let Some(ref d1) = state.d1 {
        // Parallelize attendee + contact upserts (independent D1 writes)
        let attendee_fut = crate::db::attendees::upsert_attendee(
            d1,
            &api_id,
            &event_id,
            &email,
            name,
            "approved", // self-registered attendees are auto-approved (consistent with Sheets)
            &participation_type,
            contact_channel.unwrap_or(""),
            contact_handle.unwrap_or(""),
            body.consent_marketing,
            Some(&claim_token),
        );

        let events_joined = event_id.clone();
        let contact_fut = crate::db::contacts::upsert_contact(
            d1,
            &email,
            name,
            &events_joined,
            1, // event_count will be updated on subsequent registrations
            contact_channel.unwrap_or(""),
            contact_handle.unwrap_or(""),
        );

        let (attendee_result, contact_result) = futures_util::join!(attendee_fut, contact_fut);

        if let Err(e) = attendee_result {
            tracing::warn!(
                %api_id,
                %email,
                error = %e,
                "D1 attendee upsert failed (non-fatal)"
            );
        }

        if let Err(e) = contact_result {
            tracing::warn!(
                %email,
                error = %e,
                "D1 contact upsert failed (non-fatal)"
            );
        }

        // 9d. Developer profile + registration responses (Issue #049 Phase 2, non-fatal)
        let mut profile_fields: Vec<(String, String)> = Vec::new();

        // Merge hardcoded fields (backward compat)
        if let Some(ref v) = body.experience_level {
            profile_fields.push(("experience_level".to_string(), v.clone()));
        }
        if let Some(ref v) = body.tech_stack {
            profile_fields.push(("tech_stack".to_string(), v.clone()));
        }
        if let Some(ref v) = body.interests {
            profile_fields.push(("interests".to_string(), v.clone()));
        }

        // Merge dynamic fields (Phase 2)
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
            participation_type: &participation_type,
            consent_given: body.consent_given.unwrap_or(false),
            photo_consent_given: body.photo_consent_given.unwrap_or(false),
            consent_marketing: body.consent_marketing.unwrap_or(false),
            profile_fields,
        })
        .await;
    }

    // 9b. Detach Google Sheets writes — response returns immediately (Phase 2c)
    let bg_credit_method = credit_covered_method.clone();
    if let Some(ctx) = &state.worker_ctx {
        let bg_state = state.clone();
        let bg_api_id = api_id.clone();
        let bg_name = name.to_string();
        let bg_first_name = first_name.to_string();
        let bg_last_name = last_name.to_string();
        let bg_email = email.clone();
        let bg_claim_token = claim_token.clone();
        let bg_participation_type = participation_type_display.clone();
        let bg_now = now.clone();
        let bg_contact_channel = contact_channel.map(String::from);
        let bg_contact_handle = contact_handle.map(String::from);
        let bg_deposit_agreed = body.deposit_agreed.unwrap_or(false);
        let bg_consent_given = body.consent_given.unwrap_or(false);
        let bg_photo_consent_given = body.photo_consent_given.unwrap_or(false);
        let bg_consent_marketing = body.consent_marketing;
        let bg_mapping = mapping.clone();
        let bg_sheet_id = config.sheet_id.clone();
        let bg_sheet_name = config.sheet_name.clone();
        let bg_kv = kv.cloned();
        let bg_event_id = event_id.clone();
        let bg_config = config.clone();

        ctx.wait_until(async move {
            // Append attendee row
            crate::sheets::bg_sync::append_attendee_row(
                bg_state.clone(),
                bg_api_id.clone(),
                bg_name.to_string(),
                bg_first_name.clone(),
                bg_last_name.clone(),
                bg_email.clone(),
                bg_claim_token.clone(),
                bg_participation_type.clone(),
                bg_now.clone(),
                bg_contact_channel.clone(),
                bg_contact_handle.clone(),
                bg_deposit_agreed,
                bg_consent_given,
                bg_photo_consent_given,
                bg_consent_marketing,
                bg_mapping.clone(),
                bg_sheet_id.clone(),
                bg_sheet_name.clone(),
                bg_kv.clone(),
            )
            .await;

            // Upsert to contacts sheet (matches upsert_contact_after_registration)
            let resolved = if let Some(db) = bg_state.d1.as_deref() {
                crate::org_store::resolve_contacts_sheet(db, &bg_config, &bg_state.config.sheets)
                    .await
            } else {
                event_checkin_domain::models::org::ResolvedContactsSheet {
                    sheet_id: bg_state.config.sheets.contacts_sheet_id.clone(),
                    contacts_sheet_name: bg_state.config.sheets.contacts_sheet_name.clone(),
                    events_sheet_name: bg_state.config.sheets.events_sheet_name.clone(),
                }
            };
            if !resolved.sheet_id.is_empty() {
                let contact_upsert = crate::sheets::contacts::ContactUpsert {
                    email: &bg_email,
                    name: &bg_name,
                    event_id: &bg_event_id,
                    contact_channel: bg_contact_channel.as_deref(),
                    contact_handle: bg_contact_handle.as_deref(),
                };
                if let Err(e) = crate::sheets::contacts::upsert_contact(
                    &contact_upsert,
                    &bg_state,
                    &resolved.sheet_id,
                    &resolved.contacts_sheet_name,
                    bg_kv.as_ref(),
                )
                .await
                {
                    tracing::warn!(%bg_email, error = %e, "bg_sync: contacts upsert failed");
                }
            }

            // Write deposit_method if credit covered the deposit
            if let Some(ref method) = bg_credit_method
                && let Err(e) = crate::sheets::write::update_deposit_method(
                    &bg_state,
                    &bg_sheet_id,
                    &bg_sheet_name,
                    bg_kv.as_ref(),
                    &bg_api_id,
                    method,
                )
                .await
            {
                tracing::warn!(%bg_api_id, error = %e, "bg_sync: deposit_method write failed");
            }
        });
    } else {
        // Fallback: blocking Sheets writes when worker_ctx unavailable (tests)
        if let Err(e) = sheets::append_attendee_row(
            &api_id,
            name,
            &first_name,
            &last_name,
            &email,
            &claim_token,
            &participation_type_display,
            &now,
            contact_channel,
            contact_handle,
            body.deposit_agreed.unwrap_or(false),
            body.consent_given.unwrap_or(false),
            body.photo_consent_given.unwrap_or(false),
            body.consent_marketing,
            &mapping,
            &state,
            &config.sheet_id,
            &config.sheet_name,
            kv,
        )
        .await
        {
            tracing::warn!(%email, error = %e, "Sheets append row failed (non-fatal)");
        }

        upsert_contact_after_registration(
            &email,
            name,
            &event_id,
            contact_channel,
            contact_handle,
            &state,
            &config,
            kv,
        )
        .await;
    }

    // 10. Determine next_step based on event format and participation type
    // Note: deposit_method is now written in the background (step 9b)
    // New registrations are never checked in or claimed yet
    let next_step = if credit_covered_method.is_some() {
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
            false, // new registration, not checked in
            false, // new registration, not claimed
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

    // Resolve event by slug (KV → D1 fallback)
    let kv = state.events_kv.as_ref();

    let config = crate::event_store::resolve_event_by_slug(kv, slug, state.d1.as_deref())
        .await
        .map_err(AppError::NotFound)?;

    // Fetch attendees and find by email (case-insensitive)
    let attendees = sheets::get_attendees_for_event(
        &state,
        &config.sheet_id,
        &config.sheet_name,
        kv,
        &config.id,
    )
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

    let claim_token = resolve_claim_token_from_d1(&state, attendee)
        .await
        .unwrap_or_default();
    let is_checked_in = attendee
        .checked_in_at
        .as_ref()
        .is_some_and(|s| !s.is_empty());
    let is_claimed = attendee.claimed_at.as_ref().is_some_and(|s| !s.is_empty());
    // Fetch deposit status (D1-first, KV fallback)
    let deposit = crate::event_store::get_deposit_status_with_fallback(
        kv,
        state.d1.as_deref(),
        &config.id,
        &attendee.api_id,
    )
    .await
    .ok()
    .flatten();
    let next_step = build_next_step(
        &config.event_format,
        &config.id,
        &attendee.api_id,
        &claim_token,
        &state,
        deposit.as_ref(),
        &attendee.participation_type,
        is_checked_in,
        is_claimed,
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
///
/// Returns the **canonical** storage form (`in_person`/`online`) — see
/// `ParticipationType::as_str()`. The Google Sheet append path must convert
/// this to display-case via `ParticipationType::display()` for organizer-facing
/// cells; everything else (D1, capacity checks, logging) consumes canonical.
fn resolve_participation_type(
    format: &EventFormat,
    user_choice: Option<&str>,
) -> Result<String, AppError> {
    let resolved = match format {
        EventFormat::InPerson => ParticipationType::InPerson,
        EventFormat::Online => ParticipationType::Online,
        EventFormat::Hybrid => match user_choice.map(str::trim).filter(|s| !s.is_empty()) {
            Some(choice) => ParticipationType::parse(choice),
            None => ParticipationType::InPerson,
        },
    };
    Ok(resolved.as_str().to_string())
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
/// Iterates active events (KV index → D1 fallback), checks each event's attendee list for the JWT email.
#[worker::send]
pub async fn my_registrations(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<Vec<MyRegistrationsItem>>, crate::error::WorkerError> {
    let kv = state.events_kv.as_ref();
    let d1 = state.d1.as_deref();

    // Build list of (event_id, event_name, event_slug, event_start_ms, EventConfig)
    // from KV index → D1 fallback
    let event_list: Vec<(
        String,
        String,
        String,
        i64,
        event_checkin_domain::models::event::EventConfig,
    )> = if let Some(kv_store) = kv {
        // KV path: use index + per-event config
        let index = crate::event_store::get_event_index(kv_store)
            .await
            .map_err(AppError::Internal)?;
        let mut list = Vec::new();
        for meta in &index.events {
            if matches!(meta.status, EventStatus::Completed | EventStatus::Archived) {
                continue;
            }
            if let Ok(Some(config)) = crate::event_store::get_event_config(kv_store, &meta.id).await
            {
                list.push((
                    meta.id.clone(),
                    meta.name.clone(),
                    meta.slug.clone(),
                    meta.event_start_ms,
                    config,
                ));
            }
        }
        list
    } else if let Some(db) = d1 {
        // D1 fallback: list all events
        let rows = crate::db::events::list_events(db)
            .await
            .map_err(AppError::Internal)?;
        rows.into_iter()
            .filter(|r| {
                let status = r.status.as_deref().unwrap_or("");
                status != "completed" && status != "archived"
            })
            .map(|r| {
                let config = r.to_event_config();
                let id = config.id.clone();
                let name = config.name.clone();
                let slug = config.slug.clone();
                let start_ms = config.event_start_ms;
                (id, name, slug, start_ms, config)
            })
            .collect()
    } else {
        return Err(AppError::Internal("neither KV nor D1 configured".to_string()).into());
    };

    // Process events concurrently (config + attendee loads are independent per-event)
    let event_futures: Vec<_> = event_list
        .into_iter()
        .map(
            |(event_id, event_name, event_slug, event_start_ms, config)| {
                let state = state.clone();
                let kv = kv.cloned();
                let email = claims.email.clone();
                async move {
                    // Fetch attendees (KV-cached or direct sheet read)
                    let attendees = match crate::sheets::get_attendees_for_event(
                        &state,
                        &config.sheet_id,
                        &config.sheet_name,
                        kv.as_ref(),
                        &config.id,
                    )
                    .await
                    {
                        Ok(a) => a,
                        Err(e) => {
                            tracing::warn!(
                                event_id = %event_id,
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

                    let claim_token = resolve_claim_token_from_d1(&state, attendee)
                        .await
                        .unwrap_or_default();
                    let is_checked_in = attendee
                        .checked_in_at
                        .as_ref()
                        .is_some_and(|s| !s.is_empty());
                    let is_claimed = attendee.claimed_at.as_ref().is_some_and(|s| !s.is_empty());
                    // Fetch deposit status (D1-first, KV fallback)
                    let deposit = crate::event_store::get_deposit_status_with_fallback(
                        kv.as_ref(),
                        state.d1.as_deref(),
                        &event_id,
                        &attendee.api_id,
                    )
                    .await
                    .ok()
                    .flatten();
                    let next_step = build_next_step(
                        &config.event_format,
                        &event_id,
                        &attendee.api_id,
                        &claim_token,
                        &state,
                        deposit.as_ref(),
                        &attendee.participation_type,
                        is_checked_in,
                        is_claimed,
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
                        event_id,
                        event_name,
                        event_slug,
                        event_start_ms,
                        attendee_id: attendee.api_id.clone(),
                        name: attendee.name.clone(),
                        participation_type: attendee.participation_type.clone(),
                        status,
                        next_step,
                    })
                }
            },
        )
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

/// Build the next_step response based on event format, deposit, and check-in status.
///
/// Logic:
/// - Already claimed → ticket page
/// - Checked in (not claimed) → claim page
/// - Online-only events → quest/claim page
/// - In-person/hybrid events:
///   - No deposit yet → deposit page
///   - Deposit exists → ticket page
#[allow(clippy::too_many_arguments)]
fn build_next_step(
    format: &EventFormat,
    event_id: &str,
    api_id: &str,
    claim_token: &str,
    state: &AppState,
    deposit: Option<&event_checkin_domain::models::deposit::DepositStatus>,
    participation_type: &str,
    is_checked_in: bool,
    is_claimed: bool,
) -> NextStep {
    let _claim_base = &state.config.server.claim_base_url;

    // Already claimed NFT — go to ticket page (final state)
    if is_claimed {
        return NextStep {
            step_type: "ticket".to_string(),
            url: format!("/ticket/{api_id}?event_id={event_id}"),
        };
    }

    // Checked in but not yet claimed — go directly to claim page if token exists
    if is_checked_in && !claim_token.is_empty() {
        return NextStep {
            step_type: "claim".to_string(),
            url: format!("/claim/{claim_token}"),
        };
    }

    // Online attendees never need deposit — skip straight to waiting/ticket.
    // Quest completion (quiz/adventure) serves as virtual check-in at claim time.
    if is_online_participation(participation_type) {
        return NextStep {
            step_type: "waiting".to_string(),
            url: format!("/ticket/{api_id}?event_id={event_id}"),
        };
    }

    if format.has_in_person() {
        // A USDC deposit initiation that was never signed is an orphan
        // (user rejected the wallet prompt after `deposit_usdc` saved a
        // pending record). Treat it as "no deposit" so the attendee is sent
        // back to the deposit page to retry, not to the ticket page.
        // Real deposits (verified, or carrying a tx_signature) count.
        let real_deposit = deposit.is_some_and(|d| {
            d.verified
                || !matches!(
                    d.method,
                    event_checkin_domain::models::deposit::DepositMethod::Usdc
                )
                || d.tx_signature.as_deref().is_some_and(|t| !t.is_empty())
        });
        if real_deposit {
            // Deposit exists (verified or pending with a TX) — show ticket page
            NextStep {
                step_type: "ticket".to_string(),
                url: format!("/ticket/{api_id}?event_id={event_id}"),
            }
        } else {
            // No real deposit yet — go to deposit page
            NextStep {
                step_type: "deposit".to_string(),
                url: format!("/deposit/{api_id}?event_id={event_id}"),
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

fn is_online_participation(participation_type: &str) -> bool {
    // Delegates to the canonical ParticipationType enum (single source of truth).
    // Equivalent to "contains online/virtual" for all realistic inputs; an
    // ambiguous value mentioning both tracks now resolves in-person-first.
    matches!(
        ParticipationType::parse(participation_type),
        ParticipationType::Online
    )
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
    kv: Option<&worker::kv::KvStore>,
) -> Result<(), AppError> {
    use event_checkin_domain::models::event::OnlineOpenMode;

    // Judge the registering attendee with the SAME canonical enum used for
    // existing attendees (Attendee::is_in_person) below — keeps both checks
    // consistent and covers the `in_person`/`physical` variants the old inline
    // matcher missed.
    let is_in_person = matches!(
        ParticipationType::parse(participation_type),
        ParticipationType::InPerson
    );

    // Count current attendees from sheet
    let attendees = crate::sheets::get_attendees_for_event(
        state,
        &config.sheet_id,
        &config.sheet_name,
        kv,
        &config.id,
    )
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

    // Count walk-in attendees from D1
    if let Some(db) = state.d1.as_deref() {
        match crate::db::attendees::count_walkin_attendees(db, &config.id).await {
            Ok(count) => {
                in_person_count += count;
            }
            Err(e) => {
                tracing::warn!(error = %e, "D1 walkin count for capacity failed, skipping");
            }
        }
    }

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
        if !config.has_in_person_capacity(in_person_count) {
            return Err(AppError::Validation(
                "In-person spots are full. Please register for the online track instead."
                    .to_string(),
            ));
        }
    } else {
        // Check online capacity
        if !config.has_online_capacity(online_count) {
            return Err(AppError::Validation(
                "Online spots are full. Registration is closed.".to_string(),
            ));
        }

        // Check online registration gating
        let in_person_available = config.has_in_person_capacity(in_person_count);

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
    let resolved = if let Some(db) = state.d1.as_deref() {
        crate::org_store::resolve_contacts_sheet(db, event_config, &state.config.sheets).await
    } else {
        let global = &state.config.sheets;
        event_checkin_domain::models::org::ResolvedContactsSheet {
            sheet_id: global.contacts_sheet_id.clone(),
            contacts_sheet_name: global.contacts_sheet_name.clone(),
            events_sheet_name: global.events_sheet_name.clone(),
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

/// Input data for writing developer profile + registration responses to D1.
struct DeveloperData<'a> {
    d1: &'a worker::D1Database,
    email: &'a str,
    name: &'a str,
    event_id: &'a str,
    contact_channel: &'a str,
    contact_handle: &'a str,
    participation_type: &'a str,
    consent_given: bool,
    photo_consent_given: bool,
    consent_marketing: bool,
    /// Dynamic profile fields (key, value) pairs from form config.
    profile_fields: Vec<(String, String)>,
}

/// Write developer profile + registration responses to D1 (Issue #049 Phase 2).
///
/// Best-effort: each write is individually wrapped in warn-on-error.
async fn write_developer_data(data: &DeveloperData<'_>) {
    let DeveloperData {
        d1,
        email,
        name,
        event_id,
        contact_channel,
        contact_handle,
        participation_type,
        consent_given,
        photo_consent_given,
        consent_marketing,
        profile_fields: _,
    } = data;

    // 1. Upsert developer profile (display_name + consent_outreach)
    if let Err(e) =
        crate::db::developers::upsert_developer_field(d1, email, "display_name", name).await
    {
        tracing::warn!(%email, error = %e, "D1 developer display_name upsert failed (non-fatal)");
    }

    // 1b. Upsert consent_outreach from marketing consent
    let consent_val = if *consent_marketing { "1" } else { "0" };
    if let Err(e) =
        crate::db::developers::upsert_developer_field(d1, email, "consent_outreach", consent_val)
            .await
    {
        tracing::warn!(%email, error = %e, "D1 developer consent_outreach upsert failed (non-fatal)");
    }

    // 1c. Upsert dynamic profile fields
    for (key, value) in &data.profile_fields {
        if !value.is_empty()
            && let Err(e) = crate::db::developers::upsert_developer_field(
                d1,
                email,
                key.as_str(),
                value.as_str(),
            )
            .await
        {
            tracing::warn!(%email, key, error = %e, "D1 developer field upsert failed (non-fatal)");
        }
    }

    // 2. Store registration responses (single batch INSERT — 1 D1 call instead of N)
    let mut responses: Vec<(&str, &str, bool)> = vec![
        ("participation_type", participation_type, false),
        ("contact_channel", contact_channel, false),
        ("contact_handle", contact_handle, false),
        (
            "consent_given",
            if *consent_given { "true" } else { "false" },
            false,
        ),
        (
            "photo_consent_given",
            if *photo_consent_given {
                "true"
            } else {
                "false"
            },
            false,
        ),
        (
            "consent_marketing",
            if *consent_marketing { "true" } else { "false" },
            false,
        ),
    ];

    // Profile-enriching fields
    for (key, value) in &data.profile_fields {
        if !value.is_empty() {
            responses.push((key.as_str(), value.as_str(), true));
        }
    }

    if let Err(e) =
        crate::db::developers::batch_insert_registration_responses(d1, event_id, email, &responses)
            .await
    {
        tracing::warn!(
            %email,
            %event_id,
            error = %e,
            "D1 batch registration responses failed (non-fatal)"
        );
    }
}

// ---------------------------------------------------------------------------
// Post-event registration (Plan 008 — Phase 3 §3.3.2)
// ---------------------------------------------------------------------------
//
// Lead capture for completed events: a visitor who missed the event signs in
// with Google (JWT) and submits a stripped registration (name + contact +
// developer-profile fields). No deposit, no check-in, no NFT — they become a
// `post_event_registered` attendee row + a `contacts` / `developer_profiles`
// entry for future outreach.

/// Request body for `POST /api/public/event/{slug}/register-post-event`.
///
/// A subset of `RegisterRequest` — drops `participation_type`, `deposit_agreed`,
/// `photo_consent_given` (irrelevant: they didn't attend). Keeps the
/// developer-profile fields because capturing the visitor's stack/interests is
/// the primary value of post-event registration.
#[derive(Debug, Clone, Deserialize)]
pub struct PostEventRegisterRequest {
    pub name: String,
    pub contact_channel: Option<String>,
    pub contact_handle: Option<String>,
    /// PDPA consent for data collection (always required).
    pub consent_given: Option<bool>,
    /// Marketing consent for future-event outreach (the point of lead capture).
    #[serde(default)]
    pub consent_marketing: Option<bool>,
    pub experience_level: Option<String>,
    pub tech_stack: Option<String>,
    pub interests: Option<String>,
    #[serde(default)]
    pub profile_fields: Option<std::collections::HashMap<String, String>>,
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use event_checkin_domain::models::attendee::ParticipationType;
    use event_checkin_domain::models::event::EventFormat;

    #[test]
    fn is_online_participation_uses_canonical_enum() {
        // resolve_participation_type produces these for the registration path
        assert!(is_online_participation("Online"));
        assert!(is_online_participation("online"));
        assert!(is_online_participation("Virtual"));
        assert!(!is_online_participation("In-Person"));
        assert!(!is_online_participation("in_person"));
        assert!(!is_online_participation("physical"));
        // Empty defaults to in-person (legacy), not online
        assert!(!is_online_participation(""));
        // walk-in sentinel is neither online nor in-person
        assert!(!is_online_participation("walkin"));
    }

    #[test]
    fn resolve_participation_type_defaults_by_format() {
        // Returns canonical snake_case (stored in D1); the Sheet append path
        // converts to display-case via ParticipationType::display().
        assert_eq!(
            resolve_participation_type(&EventFormat::InPerson, None).unwrap(),
            "in_person"
        );
        assert_eq!(
            resolve_participation_type(&EventFormat::Online, None).unwrap(),
            "online"
        );
        // Hybrid honors the user's choice (normalized to canonical), defaulting
        // to in-person when absent. Accepts both display-case and canonical input.
        assert_eq!(
            resolve_participation_type(&EventFormat::Hybrid, Some("Online")).unwrap(),
            "online"
        );
        assert_eq!(
            resolve_participation_type(&EventFormat::Hybrid, Some("In-Person")).unwrap(),
            "in_person"
        );
        assert_eq!(
            resolve_participation_type(&EventFormat::Hybrid, Some("in_person")).unwrap(),
            "in_person"
        );
        assert_eq!(
            resolve_participation_type(&EventFormat::Hybrid, None).unwrap(),
            "in_person"
        );
    }

    /// enforce_capacity judges the registering attendee via ParticipationType::parse
    /// (the same enum as Attendee::is_in_person), so its in-person detection is
    /// covered by the domain crate's participation_type tests. This pins the
    /// shared contract that both checks now use.
    #[test]
    fn registering_in_person_detection_matches_canonical_enum() {
        for v in [
            "In-Person",
            "in_person",
            "in person",
            "IN-PERSON",
            "physical",
            "",
        ] {
            assert_eq!(
                ParticipationType::parse(v),
                ParticipationType::InPerson,
                "expected '{v}' to be in-person"
            );
        }
        for v in ["Online", "online", "Virtual"] {
            assert_eq!(
                ParticipationType::parse(v),
                ParticipationType::Online,
                "expected '{v}' to be online"
            );
        }
        // walk-in sentinel stays out of both tracks
        assert_eq!(ParticipationType::parse("walkin"), ParticipationType::Other);
    }

    /// Cross-cutting invariant: every write path in 3.2 (registration,
    /// manual override, Sheet→D1 sync) must produce a canonical value that,
    /// when read back through `Attendee::is_in_person()`, gives the expected
    /// in-person/online judgment. This is the contract that prevents the
    /// pre-3.2 bug class (registration wrote display-case, sync wrote
    /// snake_case, reads were inconsistent).
    #[test]
    fn write_paths_produce_canonical_values_round_tripping_is_in_person() {
        use event_checkin_domain::models::attendee::CheckInStatus;

        // Registration: resolve_participation_type returns canonical
        for (format, choice, expected_in_person) in [
            (EventFormat::InPerson, None, true),
            (EventFormat::Online, None, false),
            (EventFormat::Hybrid, Some("Online"), false),
            (EventFormat::Hybrid, Some("in_person"), true),
            (EventFormat::Hybrid, None, true),
        ] {
            let stored = resolve_participation_type(&format, choice).unwrap();
            // The canonical value, read back as an Attendee, must match intent
            let attendee = make_minimal_attendee(&stored, CheckInStatus::Approved);
            assert_eq!(
                attendee.is_in_person(),
                expected_in_person,
                "registration stored '{stored}' for {format:?}/choice={choice:?}"
            );
        }
    }

    /// Minimal Attendee constructor for write-path round-trip tests.
    fn make_minimal_attendee(
        participation_type: &str,
        status: event_checkin_domain::models::attendee::CheckInStatus,
    ) -> event_checkin_domain::models::attendee::Attendee {
        use event_checkin_domain::models::attendee::Attendee;
        Attendee {
            api_id: "test".to_string(),
            first_name: String::new(),
            last_name: String::new(),
            name: "Test".to_string(),
            email: "test@test.com".to_string(),
            ticket_name: String::new(),
            approval_status: status,
            participation_type: participation_type.to_string(),
            registration_date: None,
            phone: None,
            contact_channel: None,
            contact_handle: None,
            deposit_agreed: None,
            deposit_method: None,
            deposit_amount: None,
            deposit_tx_signature: None,
            deposit_verified: None,
            checked_in_at: None,
            checked_in_by: None,
            solana_address: None,
            qr_code_url: None,
            claim_token: None,
            claimed_at: None,
            nft_proof_url: None,
            bank_account: None,
            bank_name: None,
            account_name: None,
            refund_status: None,
            refund_link: None,
            send_email_status: None,
            row_index: 0,
        }
    }
}
