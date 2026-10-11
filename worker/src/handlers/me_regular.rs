//! `GET /api/me/regular/{slug}` (Super GOAT SG1b): the event's regular rule
//! and where the signed-in person stands. The person comes from the session;
//! the path names only the event.

use axum::Extension;
use axum::extract::State;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::error::{ApiOk, WorkerError};
use crate::state::AppState;

/// `GET /api/me/regular/{slug}` (Super GOAT SG1b): the event's regular rule,
/// if any, and where the signed-in person stands. What the register form says;
/// the waiver itself is decided again at registration.
#[worker::send]
pub async fn my_regular(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    axum::extract::Path(slug): axum::extract::Path<String>,
) -> Result<ApiOk<event_checkin_domain::models::record::RegularStanding>, WorkerError> {
    use event_checkin_domain::models::record::RegularStanding;
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 not configured".into()))?;
    let Some(event_id) = crate::showed::event_id(db, &slug)
        .await
        .map_err(AppError::Internal)?
    else {
        return Ok(ApiOk::new(RegularStanding::default()));
    };
    let rule = crate::regular::rule(db, &event_id)
        .await
        .map_err(AppError::Internal)?;
    let record = crate::record::my_record(db, &claims.email.to_lowercase())
        .await
        .map_err(AppError::Internal)?;
    Ok(ApiOk::new(RegularStanding {
        qualifies: rule.is_some_and(|r| r.qualifies(record.door_checkins)),
        rule,
        door_checkins: record.door_checkins,
    }))
}
