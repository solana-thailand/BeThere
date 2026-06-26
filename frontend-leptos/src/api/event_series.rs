//! Event series API client (Plan 013).
//!
//! Drives the "Related events" / prev-next section on the ticket page by
//! reading the public, cached `GET /api/public/event-series/{event_id}`
//! endpoint. Series structure changes rarely and is not user-specific, so it
//! is safe to cache (server caches 120s, client cache 30s).

use serde::Deserialize;

use super::fetch::get as http_get;
use super::{ApiResponse, ApiError, api_base, response_json, response_text};

/// One event in an ordered series (playlist position).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SeriesEvent {
    pub event_id: String,
    pub name: String,
    pub slug: String,
    pub event_start_ms: i64,
    pub sequence_order: i64,
}

/// The campaign/series containing the current event, with prev/next neighbors.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EventSeries {
    pub campaign: SeriesCampaign,
    pub events: Vec<SeriesEvent>,
    /// Position of the current event in `events`, or -1 if it's linked to the
    /// campaign but missing from the joined events list (orphan link).
    pub current_index: i64,
    pub previous: Option<SeriesEvent>,
    pub next: Option<SeriesEvent>,
}

/// Minimal campaign info shown in the "Part of {Series}" badge.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SeriesCampaign {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
}

/// GET /api/public/event-series/{event_id}
///
/// Returns `Ok(Some(series))` when the event belongs to a campaign, and
/// `Ok(None)` when it does not (HTTP 404). Network/parse errors surface as
/// `Err` so the caller can distinguish "no series" from "request failed".
///
/// Public endpoint; server caches 120s. The series nav is fetched once per
/// ticket view (no polling), so we skip the in-memory SWR cache.
pub async fn get_event_series(event_id: &str) -> Result<Option<EventSeries>, ApiError> {
    let path = format!("/public/event-series/{event_id}");
    let url = format!("{}{path}", api_base());

    // Single request. 404 is the expected "no series" signal — map it to None
    // rather than surfacing it as an error to the caller.
    let response = http_get(&url, &[]).await?;
    if response.status() == 404 {
        return Ok(None);
    }
    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("HTTP {}", response.status())),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_else(|| format!("HTTP {}", response.status())),
            status: response.status(),
        });
    }

    let json = response_text(&response).await?;
    let wrapper: ApiResponse<EventSeries> =
        serde_json::from_str(&json).map_err(|e| ApiError {
            message: format!("Failed to parse event series: {e}"),
            status: 0,
        })?;

    wrapper.data.map(Some).ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or_else(|| "No data".to_string()),
        status: 0,
    })
}
