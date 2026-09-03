//! Event recap and past-event listings (Plan 008 Phase 2).

use serde::{Deserialize, Serialize};

use crate::api::types::{ApiError, ApiResponse};
use crate::api::{
    api_get, api_put_json, fetch::response_json,
};


// ===== Event Recap (Plan 008 Phase 2) =====

/// Recap content payload returned by `GET /api/events/{id}/recap` (organizer).
///
/// `summary_frozen` tells the UI whether a frozen `event_summaries` row exists.
/// When `false`, the organizer must freeze the summary before authoring a recap
/// (the backend refuses to publish without a freeze).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EventRecapData {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub summary_frozen: bool,
    #[serde(default)]
    pub recap: EventRecapPayload,
}

/// Recap slice of the `event_summaries` row. Mirrors the backend
/// `EventRecap` domain type; all fields `#[serde(default)]` so a draft
/// (empty markdown, no image, no published_at) deserializes cleanly.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EventRecapPayload {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub recap_markdown: String,
    #[serde(default)]
    pub recap_image_url: String,
    #[serde(default)]
    pub recap_published_at: Option<String>,
    #[serde(default)]
    pub frozen_at: Option<String>,
}

/// Request body for `PUT /api/events/{id}/recap`.
///
/// `publish: true` sets `recap_published_at = now` and mirrors
/// `recap_published = 1` onto the events row (visible on `/past-events`).
/// `publish: false` saves as draft (unpublishes a live recap if one exists).
#[derive(Debug, Clone, Serialize)]
pub struct PutRecapRequest {
    pub recap_markdown: String,
    pub recap_image_url: String,
    pub publish: bool,
}

/// `GET /api/events/{id}/recap` — fetch the current draft/published recap.
pub async fn get_event_recap(id: &str) -> Result<EventRecapData, ApiError> {
    let path = format!("/events/{id}/recap");
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to load recap".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<EventRecapData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse recap response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// `PUT /api/events/{id}/recap` — author + publish/unpublish the recap.
///
/// Returns the freshly-persisted recap state. The backend rejects publishing
/// when no frozen summary exists (409-style Validation error surfaced as
/// `ApiError::message`); the caller should render that message to the organizer.
pub async fn put_event_recap(
    id: &str,
    markdown: &str,
    image_url: &str,
    publish: bool,
) -> Result<EventRecapData, ApiError> {
    let path = format!("/events/{id}/recap");
    let body = PutRecapRequest {
        recap_markdown: markdown.to_string(),
        recap_image_url: image_url.to_string(),
        publish,
    };
    api_put_json::<EventRecapData>(&path, &body).await
}

/// One entry in the public past-events feed (`GET /api/public/events/past`).
///
/// Mirrors the sanitized payload from `list_past_events_raw`. Same shape as a
/// public event card, plus `poster_url` (Phase 2 cards prefer the marketing
/// poster over the NFT badge image).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PastEventItem {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub event_start_ms: i64,
    #[serde(default)]
    pub event_end_ms: i64,
    #[serde(default)]
    pub time_tba: bool,
    #[serde(default)]
    pub event_format: String,
    #[serde(default)]
    pub tagline: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub nft_image_url: String,
    #[serde(default)]
    pub poster_url: String,
    #[serde(default)]
    pub created_at: String,
}

/// Wrapper for the past-events feed response.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PastEventsResponse {
    #[serde(default)]
    pub events: Vec<PastEventItem>,
}

/// `GET /api/public/events/past` — list completed events with a published recap.
///
/// Public endpoint (no auth required). Cached 60s server-side.
pub async fn list_past_events() -> Result<PastEventsResponse, ApiError> {
    let response = api_get("/public/events/past").await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Failed to load past events".to_string(),
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<PastEventsResponse> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse past events response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// Headline funnel counts surfaced on the public recap page. Sensitive
/// financials (refunded totals, no-show) are intentionally excluded by the
/// backend — public recaps celebrate attendance, not accounting.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PublicRecapFunnel {
    #[serde(default)]
    pub registered_count: u64,
    #[serde(default)]
    pub deposited_count: u64,
    #[serde(default)]
    pub checked_in_count: u64,
    #[serde(default)]
    pub claimed_count: u64,
}

/// Sanitized event meta embedded in the public recap response.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PublicRecapEvent {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub tagline: String,
    #[serde(default)]
    pub event_start_ms: i64,
    #[serde(default)]
    pub event_end_ms: i64,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub event_format: String,
    #[serde(default)]
    pub poster_url: String,
    #[serde(default)]
    pub nft_image_url: String,
    /// Whether post-event registration (lead capture) is open (Plan 008 — Phase 3).
    /// Drives the "join the community" CTA on the recap page.
    #[serde(default)]
    pub post_event_registration_open: bool,
}

/// Public recap payload returned by `GET /api/public/event/{slug}/recap`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PublicRecapData {
    #[serde(default)]
    pub event: PublicRecapEvent,
    #[serde(default)]
    pub recap_markdown: String,
    #[serde(default)]
    pub recap_image_url: String,
    #[serde(default)]
    pub recap_published_at: Option<String>,
    #[serde(default)]
    pub frozen_at: Option<String>,
    #[serde(default)]
    pub funnel: PublicRecapFunnel,
}

/// `GET /api/public/event/{slug}/recap` — public recap for a completed event.
///
/// Returns 404 when the event isn't found, isn't `Completed`, has no published
/// recap, or no frozen summary exists. The 404 is indistinguishable from "no
/// recap" by design (don't leak existence of unpublished drafts).
pub async fn get_public_recap(slug: &str) -> Result<PublicRecapData, ApiError> {
    let path = format!("/public/event/{slug}/recap");
    let response = api_get(&path).await?;

    if !response.ok() {
        return Err(ApiError {
            message: if response.status() == 404 {
                "No public recap for this event".to_string()
            } else {
                "Failed to load recap".to_string()
            },
            status: response.status(),
        });
    }

    let wrapper: ApiResponse<PublicRecapData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse public recap response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}
