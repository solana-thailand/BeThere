//! Shared in-person head-count used by every capacity gate.
//!
//! Two handlers enforce the in-person cap — public registration
//! (`register::capacity::enforce_capacity`) and staff walk-in registration
//! (`walkin::enforce_walkin_capacity`). Both previously carried their own copy
//! of the same "count walk-ins from D1, log-and-skip on error" block, and both
//! copies failed **open**: a D1 error dropped the walk-in tally to zero, so an
//! event already at its cap looked like it still had room and the gate admitted
//! the registration anyway.
//!
//! This is the single home for that count, and it fails closed: when the event
//! actually has a cap, a D1 error is an error, not a silent zero.

use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::EventConfig;

use crate::state::AppState;

/// Count the walk-in attendees that count against `in_person_capacity`.
///
/// Walk-ins live only in D1 (the Sheets mirror is best-effort), so this is the
/// D1 half of the in-person tally; the caller adds the sheet-based half.
///
/// - **No cap set** — the count cannot change any decision, so the query is
///   skipped entirely and `0` is returned. One less D1 round-trip on the hot
///   registration path for every uncapped event.
/// - **No D1 binding** — `0` is the correct answer, not a fallback: walk-ins
///   are stored in D1, so without it none can exist.
/// - **Cap set and the query fails** — `Err`. Fail closed. Registration writes
///   go to the same D1, so a caller that cannot reach it was going to fail a
///   few lines later regardless; admitting past the cap is the worse outcome.
pub(crate) async fn count_walkins_against_cap(
    state: &AppState,
    config: &EventConfig,
) -> Result<u32, AppError> {
    match (config.in_person_capacity, state.d1.as_deref()) {
        (None, _) | (_, None) => Ok(0),
        (Some(_), Some(db)) => crate::db::attendees::count_walkin_attendees(db, &config.id)
            .await
            .map_err(|e| {
                tracing::warn!(
                    error = %e,
                    event_id = %config.id,
                    "D1 walk-in count for capacity failed; refusing to admit past the cap"
                );
                AppError::Internal(format!("failed to check capacity: {e}"))
            }),
    }
}
