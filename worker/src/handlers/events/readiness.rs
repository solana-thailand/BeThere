use axum::{Extension, extract::State};
use serde_json::json;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::error::ApiOk;
use crate::state::AppState;

/// GET /api/events/readiness — aggregate, authorization-filtered core warnings.
#[worker::send]
pub async fn get_readiness(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database unavailable".into()))?;
    let is_super_admin = state
        .config
        .super_admin_emails
        .iter()
        .any(|email| email.eq_ignore_ascii_case(&claims.email));
    let email_filter = (!is_super_admin).then_some(claims.email.as_str());
    let quiz_problems = crate::db::readiness::quiz_problems(db, email_filter)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "failed to build core readiness report");
            AppError::Internal("failed to build readiness report".into())
        })?;
    let blocked_attendees = quiz_problems
        .iter()
        .map(|problem| problem.blocked_attendees)
        .sum::<u64>();

    Ok(ApiOk::new(json!({
        "quiz_problems": quiz_problems,
        "blocked_attendees": blocked_attendees,
    })))
}
