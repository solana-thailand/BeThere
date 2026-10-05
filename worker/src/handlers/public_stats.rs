//! `GET /api/public/stats` — the landing page's numbers (Phase 0.3).
//!
//! Aggregates only, from D1, with the time they were counted. Anonymous GETs
//! go through the 30 s edge cache, so a landing burst reads D1 once per cache
//! node; `measured_at` is the stored copy's own time.

use axum::extract::State;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::public_stats::PublicStats;

use crate::error::{ApiOk, WorkerError};
use crate::state::AppState;

#[worker::send]
pub async fn get_public_stats(
    State(state): State<AppState>,
) -> Result<ApiOk<PublicStats>, WorkerError> {
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;
    let measured_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let stats = crate::db::public_stats::read_public_stats(db, measured_at)
        .await
        .map_err(AppError::Internal)?;
    Ok(ApiOk::new(stats))
}
