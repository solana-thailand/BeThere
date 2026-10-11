//! `GET /api/me/record` (Super GOAT SG1): the signed-in person's own door
//! history and badges (`crate::record`). Attendee-authed; the email comes
//! from the session only, so no request can name another person.

use axum::Extension;
use axum::extract::State;

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::record::MyRecord;

use crate::error::{ApiOk, WorkerError};
use crate::state::AppState;

#[worker::send]
pub async fn my_record(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<MyRecord>, WorkerError> {
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 not configured".into()))?;
    let mut record = crate::record::my_record(db, &claims.email.to_lowercase())
        .await
        .map_err(AppError::Internal)?;
    // the mint cluster follows the Crossmint host (state.rs)
    record.badge_cluster = match state.config.solana.crossmint_host.trim_end_matches('/') {
        "www.crossmint.com" => "mainnet-beta",
        _ => "devnet",
    }
    .to_string();
    Ok(ApiOk::new(record))
}
