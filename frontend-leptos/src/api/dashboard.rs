//! Dashboard API client — live aggregate metrics for the in-room demo dashboard.
//!
//! Backs the `/dashboard/live` Leptos page, which polls
//! `GET /api/dashboard/live` every 2.5 seconds during the live check-in demo.
//! All requests bypass the browser HTTP cache via `api_get_no_cache` so a
//! stale snapshot can never appear on the big screen.

use serde::Deserialize;

use super::types::{ApiError, ApiResponse};
use super::fetch::response_json;
use super::api_get_no_cache;

// ---------------------------------------------------------------------------
// Response types — mirror `worker/src/handlers/dashboard.rs`
// ---------------------------------------------------------------------------

/// Lightweight event metadata embedded in the dashboard response.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct EventDashboardMeta {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub capacity: i64,
    #[serde(default)]
    pub deposit_amount_usdc: i64,
    #[serde(default)]
    pub event_start_ms: i64,
}

/// Aggregate counts for the dashboard's headline tiles.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DashboardTotals {
    #[serde(default)]
    pub registered: u64,
    #[serde(default)]
    pub deposits_verified: u64,
    /// Sum of verified USDC deposit amounts in atomic units (1 USDC = 1_000_000).
    #[serde(default)]
    pub usdc_locked_total: u64,
    #[serde(default)]
    pub checked_in: u64,
    #[serde(default)]
    pub claims_minted: u64,
}

/// One row of the registration→deposit→checkin→claim funnel visualization.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FunnelStage {
    #[serde(default)]
    pub stage: String,
    #[serde(default)]
    pub count: u64,
}

/// A single entry in the dashboard's live activity feed.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ActivityEntry {
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub actor: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub description: String,
}

/// Full response shape for `GET /api/dashboard/live`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct LiveDashboardResponse {
    #[serde(default)]
    pub event: EventDashboardMeta,
    #[serde(default)]
    pub totals: DashboardTotals,
    #[serde(default)]
    pub funnel: Vec<FunnelStage>,
    #[serde(default)]
    pub recent_activity: Vec<ActivityEntry>,
    #[serde(default)]
    pub generated_at: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format an atomic USDC amount (1 USDC = 1_000_000 units) as a human-readable
/// string with 2 decimal places, e.g. `25_000_000` → `"25.00"`.
///
/// Returns `"0.00"` for zero or empty values. Used by the dashboard tiles
/// to render `usdc_locked_total` without pulling in a decimal crate.
pub fn format_usdc(amount_atomic: u64) -> String {
    let whole = amount_atomic / 1_000_000;
    let cents = (amount_atomic % 1_000_000) / 10_000;
    format!("{whole}.{cents:02}")
}

/// Map an audit_log action string to a short emoji tag for the live feed.
///
/// The audit action vocabulary comes from `audit_store::AuditAction` in the
/// worker. Unknown actions fall back to a neutral pulse so new audit events
/// never render as blank tiles.
pub fn action_emoji(action: &str) -> &'static str {
    match action {
        // Attendee lifecycle
        "attendee_checked_in" | "AttendeeCheckedIn" => "✅",
        "attendee_checkin_undone" | "AttendeeCheckinUndone" => "↩️",

        // Deposit / escrow lifecycle
        "deposit_verified" | "DepositVerified" => "💰",
        "escrow_initialized" | "EscrowInitialized" => "🎯",
        "escrow_deactivated" | "EscrowDeactivated" => "🛑",
        "escrow_claimed" | "EscrowClaimed" => "🏁",

        // NFT / claim lifecycle
        "nft_minted" | "NftMinted" | "claim_minted" => "🎖️",
        "claim_attempted" | "ClaimAttempted" => "🎫",

        // Refund lifecycle
        "refund_marked" | "RefundMarked" => "💸",
        "refund_completed" | "RefundCompleted" => "✓",

        // Event lifecycle
        "event_created" | "EventCreated" => "🎉",
        "event_updated" | "EventUpdated" => "✏️",

        _ => "•",
    }
}

// ---------------------------------------------------------------------------
// API function
// ---------------------------------------------------------------------------

/// `GET /api/dashboard/live?event_id={id}`
///
/// Fetches the live aggregate snapshot for one event. Uses `api_get_no_cache`
/// to bypass the browser HTTP cache entirely — critical because the dashboard
/// polls every 2.5 seconds during the live demo and must always reflect the
/// newest D1 state.
///
/// When `event_id` is `None`, the backend falls back to the single active
/// event (the common demo case where exactly one event is on stage).
pub async fn get_live_dashboard(event_id: Option<&str>) -> Result<LiveDashboardResponse, ApiError> {
    let path = match event_id {
        Some(eid) if !eid.is_empty() => format!("/dashboard/live?event_id={eid}"),
        _ => "/dashboard/live".to_string(),
    };

    let response = api_get_no_cache(&path).await?;

    if !response.ok() {
        return Err(ApiError {
            message: "Failed to fetch live dashboard".to_string(),
            status: response.status(),
        });
    }

    let result: ApiResponse<LiveDashboardResponse> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse live dashboard response: {e}"),
            status: 0,
        })?;

    result.data.ok_or_else(|| ApiError {
        message: "No data in live dashboard response".to_string(),
        status: 0,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_usdc_handles_zero() {
        assert_eq!(format_usdc(0), "0.00");
    }

    #[test]
    fn format_usdc_handles_exact_usdc() {
        assert_eq!(format_usdc(25_000_000), "25.00");
        assert_eq!(format_usdc(10_000_000), "10.00");
    }

    #[test]
    fn format_usdc_handles_fractional() {
        assert_eq!(format_usdc(1_500_000), "1.50");
        assert_eq!(format_usdc(1_005_000), "1.00"); // truncates sub-cent
        assert_eq!(format_usdc(99), "0.00");
    }

    #[test]
    fn action_emoji_returns_pulse_for_unknown() {
        assert_eq!(action_emoji("some_new_action"), "•");
    }

    #[test]
    fn action_emoji_returns_check_for_checkin_variants() {
        assert_eq!(action_emoji("attendee_checked_in"), "✅");
        assert_eq!(action_emoji("AttendeeCheckedIn"), "✅");
    }
}
