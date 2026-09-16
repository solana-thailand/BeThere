//! `GET /api/my-feedback-events` — the sessions this person can still rate.
//!
//! `/feedback` used to build its question blocks from the notification inbox,
//! which was fine while the queue held one row per (person, event). Migration
//! 0038 collapsed the queue to one row per person — correct for *sending*, and
//! it collapsed the form with it: someone who attended five sessions was
//! offered one (`.issues/102`).
//!
//! The message and the form legitimately want different grains:
//!
//! - the message is per **person** — "how were the sessions you attended"
//! - the form is per **(person, event)** — the per-event answers are the whole
//!   signal DevRel reports on
//!
//! So the form gets its own source at its own grain. Eligibility still has one
//! definition, in `feedback_eligible_events`; it is simply keyed to enrolment
//! rather than to a queued message.
//!
//! Also provides `GET /api/admin/feedback` (Issue #113) for event organizers
//! and admins to inspect survey results, sentiment metrics, and CSV exports,
//! including cross-event and series-level aggregation.

use axum::{
    Extension,
    extract::{Query, State},
};
use event_checkin_domain::models::{
    api::AdminFeedbackResponse,
    auth::Claims,
    error::AppError,
};
use serde::{Deserialize, Serialize};

use crate::{
    db::d1_safe::safe_all_rows,
    error::{ApiOk, WorkerError},
    handlers::ext::resolve_event_with_access,
    state::AppState,
};

/// Query parameters for admin feedback dashboard.
#[derive(Debug, Clone, Deserialize)]
pub struct AdminFeedbackQuery {
    pub event_id: Option<String>,
    pub scope: Option<String>,
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Extracts a series title/prefix from an event name.
/// Examples:
/// - "Solana x AI Builders: The Road to Mainnet #4 (Bangkok)" -> "Solana x AI Builders: The Road to Mainnet"
/// - "Solana in Latent Space Part 3" -> "Solana in Latent Space"
pub fn extract_series_key(event_name: &str) -> Option<String> {
    let name = event_name.trim();
    if let Some(pos) = name.find(" #") {
        let prefix = name[..pos].trim();
        if !prefix.is_empty() {
            return Some(prefix.to_string());
        }
    }
    if let Some(pos) = name.to_lowercase().find(" part ") {
        let prefix = name[..pos].trim();
        if !prefix.is_empty() {
            return Some(prefix.to_string());
        }
    }
    if let Some(pos) = name.to_lowercase().find(" - part ") {
        let prefix = name[..pos].trim();
        if !prefix.is_empty() {
            return Some(prefix.to_string());
        }
    }
    if let Some(pos) = name.to_lowercase().find(" episode ") {
        let prefix = name[..pos].trim();
        if !prefix.is_empty() {
            return Some(prefix.to_string());
        }
    }
    if let Some(pos) = name.to_lowercase().find(" ep. ") {
        let prefix = name[..pos].trim();
        if !prefix.is_empty() {
            return Some(prefix.to_string());
        }
    }
    None
}

/// One rateable session. Carries everything the block needs to render, so the
/// page does not have to fetch each event's public payload separately.
#[derive(Serialize, Deserialize)]
pub struct FeedbackEvent {
    pub event_id: String,
    /// Lets the page link to this person's own ticket for the session, which
    /// carries the recording — the best recall aid the platform has
    /// (`.issues/112`).
    pub attendee_id: String,
    pub slug: String,
    pub event_name: String,
    #[serde(default)]
    pub event_start_ms: i64,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub poster_url: String,
    #[serde(default)]
    pub nft_image_url: String,
    /// Decides which satisfaction dimensions the block asks (`.issues/098`).
    #[serde(default)]
    pub participation_type: String,
    /// Whether this person has already answered for this session.
    ///
    /// Not a filter. An answered session stays listed so the reader can see it
    /// is done and change their mind; dropping it would read as the answer
    /// having been lost (`.issues/107`).
    #[serde(default)]
    pub answered: i64,
}

#[worker::send]
pub async fn my_feedback_events(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<Vec<FeedbackEvent>>, WorkerError> {
    // Same gate as the inbox: the survey is addressed to a verified identity,
    // and a wallet-only session is not one.
    if !claims.email_verified {
        return Err(
            AppError::Forbidden("Sign in with Google again to give feedback.".into()).into(),
        );
    }
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 unavailable".into()))?;
    let stmt = db
        .prepare(include_str!("sql/my_feedback_events.sql"))
        .bind_refs(&[worker::d1::D1Type::Text(&claims.email)])
        .map_err(|e| AppError::Internal(format!("feedback events bind: {e:?}")))?;
    let rows = safe_all_rows(&stmt)
        .await
        .map_err(AppError::Internal)?
        .into_iter()
        .map(|value| {
            serde_json::from_value::<FeedbackEvent>(value)
                .map_err(|e| AppError::Internal(format!("feedback events row: {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    tracing::info!(
        identity_fingerprint = %state.log_fingerprint(&claims.email),
        count = rows.len(),
        "feedback events listed"
    );
    Ok(ApiOk::new(rows))
}

/// GET /api/admin/feedback?event_id={id}&scope={event|series|all}
///
/// Fetches aggregated survey statistics, satisfaction breakdown, online viewership,
/// continuation sentiment, individual responses, and pre-built CSV for an event,
/// a multi-part series, or across all events.
#[worker::send]
pub async fn admin_feedback_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<AdminFeedbackQuery>,
) -> Result<ApiOk<AdminFeedbackResponse>, WorkerError> {
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database unavailable".into()))?;

    let is_super_admin = state
        .config
        .super_admin_emails
        .iter()
        .any(|email| email.eq_ignore_ascii_case(&claims.email));
    let staff_role = crate::auth::get_staff_role(&claims.email, &state).await;
    let is_global_admin_or_org = is_super_admin
        || matches!(staff_role.as_deref(), Some("admin" | "organizer"));

    let is_all_scope = query.scope.as_deref() == Some("all")
        || query.event_id.as_deref() == Some("all");
    let is_series_scope = query.scope.as_deref() == Some("series")
        || query
            .event_id
            .as_deref()
            .is_some_and(|id| id.starts_with("series:"));

    if is_all_scope {
        // Cross-event aggregation across all events
        let filter_event_ids = if is_global_admin_or_org {
            None
        } else {
            // Filter to only events this person has organizer access to
            let metas = crate::db::events::list_event_meta_page(db, Some(&claims.email), None, 100)
                .await
                .map_err(|e| AppError::Internal(format!("failed to fetch accessible events: {e}")))?;
            let ids: Vec<String> = metas.events.into_iter().map(|e| e.id).collect();
            if ids.is_empty() {
                return Err(AppError::Forbidden("You do not have access to any events.".into()).into());
            }
            Some(ids)
        };

        let feedback_data = crate::db::feedback::get_admin_feedback(
            db,
            crate::db::feedback::FeedbackFilterScope {
                target_id: "all",
                title: "All Events (Cross-Event Aggregation)",
                series_name: None,
                filter_event_ids: filter_event_ids.as_deref(),
            },
        )
        .await
        .map_err(AppError::Internal)?;

        tracing::info!(
            admin_email = %state.log_fingerprint(&claims.email),
            respondents = feedback_data.total_respondents,
            events_count = feedback_data.events_included.len(),
            "Cross-event feedback dashboard fetched (all events)"
        );

        return Ok(ApiOk::new(feedback_data));
    }

    // Otherwise, we have an anchor event ID
    let raw_event_id = query.event_id.as_deref().map(|id| {
        if let Some(stripped) = id.strip_prefix("series:") {
            stripped
        } else {
            id
        }
    });

    let event = resolve_event_with_access(&state, &claims, raw_event_id).await?;
    let detected_series = extract_series_key(&event.name);

    if is_series_scope
        && let Some(ref series_title) = detected_series {
            // Find all events belonging to this series
            let all_metas = crate::db::events::list_events_as_meta(db)
                .await
                .map_err(AppError::Internal)?;

            let matching_ids: Vec<String> = all_metas
                .into_iter()
                .filter(|m| {
                    m.name
                        .to_lowercase()
                        .starts_with(&series_title.to_lowercase())
                })
                .map(|m| m.id)
                .collect();

            let target_slug = format!("series-{}", slugify(series_title));
            let title = format!("{series_title} (Series)");

            let feedback_data = crate::db::feedback::get_admin_feedback(
                db,
                crate::db::feedback::FeedbackFilterScope {
                    target_id: &target_slug,
                    title: &title,
                    series_name: Some(series_title.clone()),
                    filter_event_ids: Some(&matching_ids),
                },
            )
            .await
            .map_err(AppError::Internal)?;

            tracing::info!(
                admin_email = %state.log_fingerprint(&claims.email),
                series = %series_title,
                respondents = feedback_data.total_respondents,
                events_count = feedback_data.events_included.len(),
                "Series feedback dashboard fetched"
            );

            return Ok(ApiOk::new(feedback_data));
    }

    // Default: Single event scope
    let filter_ids = [event.id.clone()];
    let feedback_data = crate::db::feedback::get_admin_feedback(
        db,
        crate::db::feedback::FeedbackFilterScope {
            target_id: &event.id,
            title: &event.name,
            series_name: detected_series,
            filter_event_ids: Some(&filter_ids),
        },
    )
    .await
    .map_err(AppError::Internal)?;

    tracing::info!(
        admin_email = %state.log_fingerprint(&claims.email),
        event_id = %event.id,
        respondents = feedback_data.total_respondents,
        "Admin feedback dashboard fetched (single event)"
    );

    Ok(ApiOk::new(feedback_data))
}
