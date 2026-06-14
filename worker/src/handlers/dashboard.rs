//! Live event dashboard API.
//!
//! Provides real-time aggregate metrics for the in-room demo dashboard.
//! Powers `GET /api/dashboard/live`, which is polled every 2.5s by the
//! `/dashboard/live` Leptos page during the live check-in demo.
//!
//! All endpoints require staff auth (registered under the `protected` route
//! group in `handlers::routes`).

use axum::{
    Extension,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};

use crate::error::{ApiOk, WorkerError};
use crate::state::AppState;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::db::dashboard::{self, ActivityEntry, UsdcSummary};

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

/// Query parameters for `GET /api/dashboard/live`.
///
/// `event_id` is optional: when omitted the handler falls back to the single
/// active event (the common demo case where one event is on stage).
#[derive(Debug, Deserialize)]
pub struct LiveDashboardQuery {
    pub event_id: Option<String>,
}

/// Lightweight event metadata embedded in the dashboard response.
///
/// Trimmed from the full `D1EventRow` to the fields the UI actually renders,
/// keeping the 2.5s-poll payload small.
#[derive(Debug, Clone, Serialize)]
pub struct EventDashboardMeta {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub capacity: i64,
    pub deposit_amount_usdc: i64,
    pub event_start_ms: i64,
}

/// Aggregate counts for the dashboard's headline tiles.
#[derive(Debug, Clone, Serialize)]
pub struct DashboardTotals {
    pub registered: u64,
    pub deposits_verified: u64,
    /// Sum of verified USDC deposit amounts in atomic units (1 USDC = 1_000_000).
    pub usdc_locked_total: u64,
    pub checked_in: u64,
    pub claims_minted: u64,
}

/// One row of the registration→deposit→checkin→claim funnel visualization.
#[derive(Debug, Clone, Serialize)]
pub struct FunnelStage {
    pub stage: &'static str,
    pub count: u64,
}

/// Full response shape for `GET /api/dashboard/live`.
#[derive(Debug, Clone, Serialize)]
pub struct LiveDashboardResponse {
    pub event: EventDashboardMeta,
    pub totals: DashboardTotals,
    pub funnel: Vec<FunnelStage>,
    pub recent_activity: Vec<ActivityEntry>,
    /// RFC 3339 timestamp marking when this snapshot was generated. Lets the
    /// UI detect stale polls (e.g., when the worker isolate has been cold).
    pub generated_at: String,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// `GET /api/dashboard/live`
///
/// Returns the live aggregate snapshot for one event. All five sub-queries
/// are independently resilient: a failure in one (e.g., a transient D1 read
/// error) degrades gracefully to zero rather than failing the whole response,
/// so the dashboard keeps rendering during a live demo.
///
/// Event resolution precedence:
///   1. Explicit `?event_id=` query parameter
///   2. The single active event (`status = 'active'`)
///   3. `404 NotFound` if neither is available
#[worker::send]
pub async fn live_dashboard(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Query(query): Query<LiveDashboardQuery>,
) -> Result<ApiOk<LiveDashboardResponse>, WorkerError> {
    let d1 = state
        .d1
        .as_ref()
        .ok_or_else(|| AppError::NotFound("D1 database not available".to_string()))?;

    // Resolve the target event: explicit id first, else active event fallback.
    let event = match query.event_id.as_deref() {
        Some(eid) if !eid.is_empty() => crate::db::events::get_event(d1, eid)
            .await
            .map_err(|e| AppError::Internal(format!("failed to load event: {e}")))?
            .ok_or_else(|| AppError::NotFound(format!("event '{eid}' not found")))?,
        _ => crate::db::events::get_active_event(d1)
            .await
            .map_err(|e| AppError::Internal(format!("failed to load active event: {e}")))?
            .ok_or_else(|| AppError::NotFound("no active event found".to_string()))?,
    };

    let event_id = event.id.clone().unwrap_or_default();
    if event_id.is_empty() {
        return Err(AppError::Internal("event row missing id".to_string()).into());
    }

    tracing::info!(
        event_id = %event_id,
        event_name = ?event.name,
        "live dashboard snapshot requested",
    );

    // Run the five aggregates. Each is independently fault-tolerant: a query
    // failure logs and degrades to zero / empty rather than poisoning the whole
    // response. This matters during a live demo where a transient D1 blip
    // must not blank the big-screen dashboard.
    let registered = dashboard::count_registered(d1, &event_id)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, event_id = %event_id, "count_registered failed");
            0
        });

    let checked_in = dashboard::count_checked_in(d1, &event_id)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, event_id = %event_id, "count_checked_in failed");
            0
        });

    let claims_minted = dashboard::count_claims_minted(d1, &event_id)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, event_id = %event_id, "count_claims_minted failed");
            0
        });

    let usdc_summary: UsdcSummary = dashboard::verified_usdc_summary(d1, &event_id)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, event_id = %event_id, "verified_usdc_summary failed");
            UsdcSummary::default()
        });

    let recent_activity = dashboard::recent_activity(d1, &event_id, 20)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, event_id = %event_id, "recent_activity failed");
            Vec::new()
        });

    let deposits_verified = usdc_summary.count;
    let usdc_locked_total = usdc_summary.total_amount;

    let event_meta = EventDashboardMeta {
        id: event_id.clone(),
        name: event.name.clone().unwrap_or_default(),
        slug: event.slug.clone().unwrap_or_default(),
        capacity: event.capacity.unwrap_or(0),
        deposit_amount_usdc: event.deposit_amount_usdc.unwrap_or(0),
        event_start_ms: event.event_start_ms.unwrap_or(0),
    };

    let totals = DashboardTotals {
        registered,
        deposits_verified,
        usdc_locked_total,
        checked_in,
        claims_minted,
    };

    // Ordered funnel: registration → deposit → check-in → NFT claim.
    // Stage names match the canonical attendee lifecycle and are referenced
    // verbatim by the frontend's funnel renderer.
    let funnel = vec![
        FunnelStage {
            stage: "registered",
            count: registered,
        },
        FunnelStage {
            stage: "deposited",
            count: deposits_verified,
        },
        FunnelStage {
            stage: "checked_in",
            count: checked_in,
        },
        FunnelStage {
            stage: "claimed_nft",
            count: claims_minted,
        },
    ];

    let generated_at = chrono::Utc::now().to_rfc3339();

    Ok(ApiOk::new(LiveDashboardResponse {
        event: event_meta,
        totals,
        funnel,
        recent_activity,
        generated_at,
    }))
}
