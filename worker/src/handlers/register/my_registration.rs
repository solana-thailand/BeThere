//! `my_registration` / `my_registrations` — authenticated registration lookups,
//! plus the shared `build_next_step` / `is_online_participation` helpers.

use axum::{
    Extension,
    extract::{Path, State},
};
use futures_util::future::join_all;

use event_checkin_domain::models::attendee::ParticipationType;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::{EventFormat, EventStatus};

use crate::error::ApiOk;
use crate::handlers::adventure::resolve_claim_token_from_d1;
use crate::sheets;
use crate::state::AppState;

use super::types::{MyRegistrationResponse, MyRegistrationsItem, NextStep};

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
pub(super) fn build_next_step(
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

pub(super) fn is_online_participation(participation_type: &str) -> bool {
    // Delegates to the canonical ParticipationType enum (single source of truth).
    // Equivalent to "contains online/virtual" for all realistic inputs; an
    // ambiguous value mentioning both tracks now resolves in-person-first.
    matches!(
        ParticipationType::parse(participation_type),
        ParticipationType::Online
    )
}
