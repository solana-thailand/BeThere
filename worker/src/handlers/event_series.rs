//! Public event-series endpoint (Plan 013).
//!
//! Surfaces the existing campaign data to attendees on the ticket page so they
//! can navigate to related events in the same series (prev/next) and see a
//! "Part of {Series}" badge. This is a *read-only, public* view of data that
//! the admin campaign UI already manages (issue #051).
//!
//! No auth: campaign structure is public (like event listings). A 404 when the
//! event has no campaign lets the frontend cache layer treat it as a clean miss
//! and the `SeriesNav` component hide itself without a null-check dance.

use axum::extract::{Path, State};
use serde_json::{Value, json};

use crate::db::campaigns;
use crate::error::ApiOk;
use crate::state::AppState;
use event_checkin_domain::models::error::AppError;

/// `GET /api/public/event-series/{event_id}`
///
/// Returns the campaign containing `event_id` (if any), the ordered list of
/// events in that campaign, the current event's index, and its prev/next
/// neighbors. Designed to drive a "Related events" / prev-next section on the
/// ticket page and (later) a public playlist page.
#[worker::send]
pub async fn get_event_series(
    State(state): State<AppState>,
    Path(event_id): Path<String>,
) -> Result<ApiOk<Value>, crate::error::WorkerError> {
    let d1 = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::NotFound("D1 database not available".to_string()))?;

    // Reverse lookup: which campaign (if any) contains this event?
    let campaign = campaigns::get_campaign_for_event(d1, &event_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| {
            AppError::NotFound(format!("event '{event_id}' is not part of a campaign"))
        })?;

    // Ordered list of events in the campaign.
    let events = campaigns::list_campaign_event_summaries(d1, &campaign.id)
        .await
        .map_err(AppError::Internal)?;

    // Locate the current event and compute prev/next by position in the
    // ordered list. Extracted into `compute_series_neighbors` so the edge
    // cases (first/last/single/orphan) are unit-tested directly.
    let (current_index, previous, next) = campaigns::compute_series_neighbors(&events, &event_id);

    let series_event = |e: &campaigns::EventSeriesEntry| {
        json!({
            "event_id": e.event_id,
            "name": e.name,
            "slug": e.slug,
            "event_start_ms": e.event_start_ms,
            "sequence_order": e.sequence_order,
        })
    };

    let events_json: Vec<Value> = events.iter().map(series_event).collect();

    tracing::info!(
        campaign_id = %campaign.id,
        event_id = %event_id,
        event_count = events_json.len(),
        "event series resolved"
    );

    Ok(ApiOk::new(json!({
        "campaign": {
            "id": campaign.id,
            "title": campaign.title,
            "description": campaign.description,
        },
        "events": events_json,
        "current_index": current_index,
        "previous": previous.as_ref().map(series_event),
        "next": next.as_ref().map(series_event),
    })))
}
