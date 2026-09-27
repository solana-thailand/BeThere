//! Shared per-track head-count used by every capacity check.
//!
//! History: two gates enforce the in-person cap — public registration
//! (`register::capacity::enforce_capacity`) and staff walk-in registration
//! (`walkin::enforce_walkin_capacity`). Both once carried their own copy of the
//! "count walk-ins from D1, log-and-skip on error" block, and both copies
//! failed **open**: a D1 error dropped the walk-in tally to zero, so an event
//! at its cap looked like it had room.
//!
//! Plan 028 W3 (.issues/157): seven sites counted by fetching every attendee
//! row, and they disagreed. A walk-in row (`participation_type = 'walkin'`)
//! parsed as `Other`, so the list-based counts put it in the **online**
//! bucket, and three reclaim checks ignored walk-ins entirely. They all read
//! `count_tracks` now: one D1 `GROUP BY` on the common path, classified by
//! `TrackCounts::add`.
//!
//! It fails closed: any part of the count that cannot be established is an
//! `Err`, never a silent zero. Each caller decides what an unknown count means
//! for it (the gates refuse; the display paths show no remaining-spots figure).

use event_checkin_domain::models::attendee::TrackCounts;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::EventConfig;
use worker::KvStore;

use crate::state::AppState;

/// Attendees counted against each live track of `config`, walk-ins included.
///
/// - **D1 has rows for the event** — the `GROUP BY` answer, walk-ins included.
/// - **D1 has none** — the attendees can only be in the event's sheet, and no
///   walk-in exists (walk-ins are written to D1). An unreadable sheet then
///   trusts D1's empty answer, the rule `register::attendees` already applies:
///   a brand-new event must not fail its first registration on a sheet outage.
/// - **D1 errors** — the sheet count plus the D1 walk-in count, as before W3.
///   Mirrored walk-ins are `In-Person` rows in the sheet, so this path can
///   count one twice; it errs toward "full", and only runs while D1 is failing.
/// - **No D1 binding** — the sheet alone; without D1 no walk-in can exist.
pub(crate) async fn count_tracks(
    state: &AppState,
    config: &EventConfig,
    kv: Option<&KvStore>,
) -> Result<TrackCounts, String> {
    let Some(db) = state.d1.as_deref() else {
        return sheet_counts(state, config, kv).await;
    };
    let d1_err = match crate::db::attendees::count_tracks_by_event(db, &config.id).await {
        Ok(Some(counts)) => return Ok(counts),
        Ok(None) => {
            return match sheet_counts(state, config, kv).await {
                Ok(counts) => Ok(counts),
                Err(e) => {
                    tracing::warn!(event_id = %config.id, error = %e,
                        "sheet unreadable and D1 has no attendees — counting none");
                    Ok(TrackCounts::default())
                }
            };
        }
        Err(e) => e,
    };
    tracing::warn!(event_id = %config.id, error = %d1_err, "D1 track count failed — counting the sheet");
    let mut counts = sheet_counts(state, config, kv)
        .await
        .map_err(|e| format!("sheet: {e} | D1: {d1_err}"))?;
    let walk_ins = crate::db::attendees::count_walkin_attendees(db, &config.id)
        .await
        .map_err(|e| format!("D1 walk-ins: {e} | D1: {d1_err}"))?;
    counts.in_person = counts.in_person.saturating_add(walk_ins);
    Ok(counts)
}

/// `count_tracks` for a gate that admits against a cap: an unknown count
/// refuses the request instead of admitting past the cap.
pub(crate) async fn count_tracks_for_cap(
    state: &AppState,
    config: &EventConfig,
    kv: Option<&KvStore>,
) -> Result<TrackCounts, AppError> {
    count_tracks(state, config, kv).await.map_err(|e| {
        tracing::warn!(error = %e, event_id = %config.id,
            "capacity count failed; refusing to admit past the cap");
        AppError::Internal(format!("failed to check capacity: {e}"))
    })
}

/// Sheet-only count (the D1-first read is skipped: the caller already has D1's answer).
async fn sheet_counts(
    state: &AppState,
    config: &EventConfig,
    kv: Option<&KvStore>,
) -> Result<TrackCounts, String> {
    let attendees =
        crate::sheets::get_attendees(state, &config.sheet_id, &config.sheet_name, kv).await?;
    Ok(TrackCounts::from_participation_types(
        attendees.iter().map(|a| a.participation_type.as_str()),
    ))
}
