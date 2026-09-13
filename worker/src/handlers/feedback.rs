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

use axum::{Extension, extract::State};
use event_checkin_domain::models::{auth::Claims, error::AppError};
use serde::Serialize;

use crate::{
    db::d1_safe::safe_all_rows,
    error::{ApiOk, WorkerError},
    state::AppState,
};

/// One rateable session. Carries everything the block needs to render, so the
/// page does not have to fetch each event's public payload separately.
#[derive(Serialize, serde::Deserialize)]
pub struct FeedbackEvent {
    pub event_id: String,
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
