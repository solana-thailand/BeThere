//! Public event endpoint — returns sanitized event details by slug.
//! No authentication required for public events. Private events require auth.

use axum::extract::{Path, State};
use serde_json::{Value, json};

use crate::error::ApiOk;
use crate::state::AppState;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::EventVisibility;

/// `GET /api/public/events`
///
/// Returns a list of publicly visible events (Active or Completed only).
/// Sensitive fields (sheet_id, organizer_wallet, staff_emails, etc.) are excluded.
/// Used by the landing page to display upcoming events.
///
/// D1-first: reads from D1 when available, falls back to KV.
#[worker::send]
pub async fn list_public_events(
    State(state): State<AppState>,
) -> Result<ApiOk<Value>, crate::error::WorkerError> {
    let now_ms = chrono::Utc::now().timestamp_millis();

    let metas = if let Some(d1) = &state.d1 {
        crate::db::events::list_events_as_meta(d1)
            .await
            .map_err(AppError::Internal)?
    } else if let Some(kv) = &state.events_kv {
        let index = crate::event_store::get_event_index(kv)
            .await
            .map_err(AppError::Internal)?;
        index.events
    } else {
        return Err(AppError::Internal("no data store configured".into()).into());
    };

    // Only show Active events whose end time is in the future (upcoming).
    // Sort nearest-first so the soonest event appears at the top.
    let mut public_events: Vec<Value> = metas
        .into_iter()
        .filter(|e| {
            matches!(
                e.status,
                event_checkin_domain::models::event::EventStatus::Active
            ) && e.event_end_ms > now_ms
                && e.visibility == EventVisibility::Public
        })
        .map(|e| {
            json!({
                "id": e.id,
                "name": e.name,
                "slug": e.slug,
                "status": e.status.as_str(),
                "event_start_ms": e.event_start_ms,
                "event_end_ms": e.event_end_ms,
                "time_tba": e.time_tba,
                "deposit_enabled": e.deposit_enabled,
                "event_format": e.event_format.as_str(),
                "tagline": e.tagline,
                "location": e.location,
                "nft_image_url": e.nft_image_url,
                "created_at": e.created_at,
                "in_person_capacity": e.in_person_capacity,
                "online_capacity": e.online_capacity,
                "visibility": e.visibility.as_str(),
            })
        })
        .collect();

    // Sort by event_start_ms ascending (nearest first)
    public_events.sort_by_key(|e| {
        e.get("event_start_ms")
            .and_then(|v| v.as_i64())
            .unwrap_or(i64::MAX)
    });

    tracing::info!(
        count = public_events.len(),
        "public events listed (upcoming only)"
    );

    Ok(ApiOk::new(json!({
        "events": public_events,
    })))
}

/// `GET /api/public/event/{slug}`
///
/// Returns publicly visible event details for a given slug.
/// Draft and Archived events return 404.
/// Private events require authentication + access check.
/// Sensitive fields (sheet_id, organizer_wallet, etc.) are excluded.
#[worker::send]
pub async fn get_public_event(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    request: axum::extract::Request,
) -> Result<ApiOk<Value>, crate::error::WorkerError> {
    // Resolve slug → event config via D1-first, KV fallback
    let config = crate::event_store::read::resolve_event_by_slug(
        state.events_kv.as_ref(),
        &slug,
        state.d1.as_ref().map(|v| &**v),
    )
    .await
    .map_err(|e| AppError::NotFound(e))?;

    // Only show Active or Completed events publicly
    match config.status {
        event_checkin_domain::models::event::EventStatus::Draft
        | event_checkin_domain::models::event::EventStatus::Archived => {
            return Err(AppError::NotFound(format!("event '{slug}' not found")).into());
        }
        _ => {}
    }

    // Gate private events — require auth + access check
    if config.visibility == EventVisibility::Private {
        let token = crate::auth::extract_token_from_request(&request);
        match token {
            Some(t) => {
                let claims = crate::auth::verify_session_jwt(&t, &state.config.jwt_secret)
                    .await
                    .map_err(|_| {
                        AppError::Unauthorized("authentication required for private event".into())
                    })?;
                crate::auth::check_event_access(&claims.email, &state, &config)
                    .await
                    .map_err(AppError::Forbidden)?;
            }
            None => {
                return Err(AppError::Unauthorized(
                    "authentication required for private event".into(),
                )
                .into());
            }
        }
    }

    tracing::info!(slug = %slug, "public event fetched");

    // Count attendees for capacity display
    let (in_person_count, online_count) =
        count_attendees_by_track(&state, &config, state.events_kv.as_ref()).await;

    let in_person_remaining = config
        .in_person_capacity
        .map(|cap| cap.saturating_sub(in_person_count));
    let online_remaining = config
        .online_capacity
        .map(|cap| cap.saturating_sub(online_count));

    // Determine track availability for frontend gating
    let in_person_available =
        config.event_format.has_in_person() && in_person_remaining.is_none_or(|r| r > 0);
    let online_available = config.event_format.has_online()
        && online_remaining.is_none_or(|r| r > 0)
        && is_online_registration_open(&config, in_person_available);

    // Return sanitized response — exclude all sensitive/internal fields
    Ok(ApiOk::new(json!({
        "id": config.id,
        "name": config.name,
        "slug": config.slug,
        "tagline": config.tagline,
        "link": config.link,
        "status": config.status.as_str(),
        "event_start_ms": config.event_start_ms,
        "event_end_ms": config.event_end_ms,
        "time_tba": config.time_tba,
        "deposit_enabled": config.deposit_enabled,
        "deposit_amount_usdc": config.deposit_amount_usdc,
        "deposit_amount_thb": config.deposit_amount_thb,
        "event_format": config.event_format.as_str(),
        "nft_image_url": config.nft_image_url,
        "nft_name_template": config.nft_name_template,
        "nft_symbol": config.nft_symbol,
        "nft_description_template": config.nft_description_template,
        "quiz_enabled": config.quiz_enabled,
        "refund_deadline_hours": config.refund_deadline_hours,
        "require_contact_info": config.require_contact_info,
        "require_photo_consent": config.require_photo_consent,
        "description": config.description,
        "location": config.location,
        "created_at": config.created_at,
        "dev_mode": state.config.dev_mode,
        // Capacity info for frontend gating
        "in_person_capacity": config.in_person_capacity,
        "online_capacity": config.online_capacity,
        "in_person_count": in_person_count,
        "online_count": online_count,
        "in_person_remaining": in_person_remaining,
        "online_remaining": online_remaining,
        "in_person_available": in_person_available,
        "online_available": online_available,
        "online_open_mode": config.online_open_mode.as_str(),
        "visibility": config.visibility.as_str(),
        "escrow_status": config.escrow_status.as_str(),
    })))
}

/// Count attendees by track from sheet data.
/// Returns (in_person_count, online_count).
async fn count_attendees_by_track(
    state: &AppState,
    config: &event_checkin_domain::models::event::EventConfig,
    kv: Option<&worker::kv::KvStore>,
) -> (u32, u32) {
    let attendees = match crate::sheets::get_attendees_for_event(
        state,
        &config.sheet_id,
        &config.sheet_name,
        kv,
        &config.id,
    )
    .await
    {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(error = %e, "failed to count attendees for capacity");
            return (0, 0);
        }
    };

    let mut in_person_count: u32 = 0;
    let mut online_count: u32 = 0;

    for attendee in &attendees {
        if attendee.is_in_person() {
            in_person_count += 1;
        } else {
            online_count += 1;
        }
    }

    // Count UNSYNCED walk-in attendees as in-person.
    // Walk-in records are stored as individual KV keys: walkin:{event_id}:{email}
    // Once synced to the Google Sheet, a walkin_synced:{event_id}:{email} marker is set.
    // We only count unsynced walk-ins to avoid double-counting with sheet-based attendees.
    if let Some(kv) = kv {
        let prefix = format!("walkin:{}:", config.id);
        let mut walkin_cursor: Option<String> = None;
        let mut walkin_count: u32 = 0;
        loop {
            let mut builder = kv.list().prefix(prefix.clone());
            if let Some(c) = walkin_cursor.take() {
                builder = builder.cursor(c);
            }
            match builder.execute().await {
                Ok(resp) => {
                    for key in &resp.keys {
                        // Extract email from key: walkin:{event_id}:{email}
                        let email = key.name.strip_prefix(&prefix).unwrap_or("");
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
                    tracing::warn!(error = ?e, "failed to list walk-in keys for capacity count");
                    break;
                }
            }
        }
        in_person_count += walkin_count;
    }

    (in_person_count, online_count)
}

/// Check whether online registration is currently open based on `OnlineOpenMode`.
fn is_online_registration_open(
    config: &event_checkin_domain::models::event::EventConfig,
    in_person_available: bool,
) -> bool {
    use event_checkin_domain::models::event::OnlineOpenMode;

    match config.online_open_mode {
        OnlineOpenMode::Always => true,
        OnlineOpenMode::AutoOnFull => !in_person_available,
        OnlineOpenMode::Manual => config.online_registration_open,
    }
}
