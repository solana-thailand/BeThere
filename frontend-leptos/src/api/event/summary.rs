//! Event summary (Plan 008 Phase 1).


use crate::api::types::{ApiError, ApiResponse};
use crate::api::{
    api_get, api_post, fetch::response_json,
};


// ===== Event Summary (Plan 008 Phase 1) =====

/// Top-level wrapper returned by `/events/{id}/summary` and
/// `/events/{id}/summary/freeze`. The `frozen` flag tells the UI whether the
/// snapshot is permanent (`true`) or a live preview recomputed on each request
/// (`false`). All inner fields are `#[serde(default)]` so a partial backend
/// response degrades gracefully instead of erroring the whole page.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct EventSummaryData {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub frozen: bool,
    #[serde(default)]
    pub summary: EventSummaryPayload,
}

/// Frozen-or-live snapshot of the funnel + financials for one event.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct EventSummaryPayload {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub event_start_ms: i64,
    #[serde(default)]
    pub event_end_ms: i64,
    /// ISO 8601 timestamp the snapshot was frozen at. `None` while the
    /// event is still live (`frozen == false`) — the numbers shown are then
    /// a preview recomputed on each request.
    #[serde(default)]
    pub frozen_at: Option<String>,
    #[serde(default)]
    pub frozen_by: String,
    #[serde(default)]
    pub funnel: FunnelSnapshotData,
    #[serde(default)]
    pub financials: FinancialSnapshotData,
}

/// Funnel counts for the post-event summary. Mirrors the backend
/// `FunnelSnapshot`; every field is `u64` because D1 stores these as
/// `INTEGER` aggregates that never go negative.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct FunnelSnapshotData {
    #[serde(default)]
    pub registered_count: u64,
    #[serde(default)]
    pub deposited_count: u64,
    #[serde(default)]
    pub checked_in_count: u64,
    #[serde(default)]
    pub no_show_count: u64,
    #[serde(default)]
    pub claimed_count: u64,
    #[serde(default)]
    pub refunded_count: u64,
    #[serde(default)]
    pub post_event_reg_count: u64,
    /// In-person registrants — denominator for `no_show_count`.
    #[serde(default)]
    pub in_person_registered_count: u64,
    /// In-person registrants who checked in.
    #[serde(default)]
    pub in_person_checked_in_count: u64,
}

/// Financial totals for the post-event summary. USDC amounts are atomic
/// (1 USDC = 1_000_000); THB amounts are satang (1 THB = 100).
///
/// NOTE: `usdc_refunded_total` is always 0 in v1 — the backend doesn't yet
/// sum USDC refunds. The UI surfaces this honestly rather than pretending
/// the figure is authoritative.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct FinancialSnapshotData {
    #[serde(default)]
    pub usdc_deposited_total: u64,
    #[serde(default)]
    pub usdc_refunded_total: u64,
    #[serde(default)]
    pub thb_deposited_total: u64,
    #[serde(default)]
    pub thb_refunded_total: u64,
}

/// GET /api/events/{id}/summary — fetch the post-event summary snapshot.
///
/// Returns a frozen snapshot if one exists, otherwise a live preview computed
/// from current D1 state (`frozen == false`). Requires a staff JWT
/// (organizer+ role is enforced server-side).
pub async fn get_event_summary(id: &str) -> Result<EventSummaryData, ApiError> {
    let path = format!("/events/{id}/summary");
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to load summary".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<EventSummaryData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse summary response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

/// POST /api/events/{id}/summary/freeze — permanently freeze the summary.
///
/// Returns 400 if the event has not ended yet or is still Draft, 403 if the
/// caller is Staff (not organizer+), 404 if the event doesn't exist. On
/// success the returned snapshot has `frozen == true` and `frozen_at` set;
/// subsequent refunds will not change these numbers.
pub async fn freeze_event_summary(id: &str) -> Result<EventSummaryData, ApiError> {
    let path = format!("/events/{id}/summary/freeze");
    let response = api_post(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Freeze failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<EventSummaryData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse freeze response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}
