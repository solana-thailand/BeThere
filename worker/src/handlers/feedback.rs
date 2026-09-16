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
//! and admins to inspect survey results, sentiment metrics, and CSV exports.

use axum::{
    Extension,
    extract::{Query, State},
};
use event_checkin_domain::models::{
    api::AdminFeedbackResponse,
    auth::Claims,
    error::AppError,
};
use serde::Serialize;

use crate::{
    db::d1_safe::safe_all_rows,
    error::{ApiOk, WorkerError},
    handlers::ext::{EventIdQuery, resolve_event_with_access},
    state::AppState,
};

/// One rateable session. Carries everything the block needs to render, so the
/// page does not have to fetch each event's public payload separately.
#[derive(Serialize, serde::Deserialize)]
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

/// GET /api/admin/feedback?event_id={id}
///
/// Fetches aggregated survey statistics, satisfaction breakdown, online viewership,
/// continuation sentiment, individual responses, and pre-built CSV for an event.
#[worker::send]
pub async fn admin_feedback_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<AdminFeedbackResponse>, WorkerError> {
    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;

    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database unavailable".into()))?;

    let feedback_data = crate::db::feedback::get_admin_feedback(db, &event.id, &event.name)
        .await
        .map_err(AppError::Internal)?;

    tracing::info!(
        admin_email = %state.log_fingerprint(&claims.email),
        event_id = %event.id,
        respondents = feedback_data.total_respondents,
        "Admin feedback dashboard fetched"
    );

    Ok(ApiOk::new(feedback_data))
}
