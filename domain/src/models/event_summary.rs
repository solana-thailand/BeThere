//! Post-event summary types (Plan 008 — Phase 1).
//!
//! A frozen point-in-time snapshot of an event's funnel + financials, written
//! once after the event ends. Later refunds/claims do **not** mutate a frozen
//! summary — that is the whole point of freezing.
//!
//! The on-the-wire response payload (returned by `GET /api/events/{id}/summary`)
//! is `EventSummaryResponse`, which wraps `EventSummary` with a `frozen` flag
//! so the UI can distinguish a persisted freeze from a live-computed preview.

// ---------------------------------------------------------------------------
// Response / API payload types
// ---------------------------------------------------------------------------

/// Full summary payload returned by the summary endpoints.
///
/// Mirrors the `event_summaries` table columns. `frozen_at` is `None` when the
/// summary is a live preview (not yet persisted) — see `EventSummaryResponse`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct EventSummary {
    pub event_id: String,
    pub funnel: FunnelSnapshot,
    pub financials: FinancialSnapshot,
    pub event_start_ms: i64,
    pub event_end_ms: i64,
    /// ISO 8601 — `None` for a live preview (not yet frozen).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen_at: Option<String>,
    /// Email of the user who froze; empty string for an auto-freeze.
    #[serde(default)]
    pub frozen_by: String,
}

/// Funnel counters — the top-line attendance funnel.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FunnelSnapshot {
    /// Approved pre-event registrations.
    pub registered_count: u64,
    /// Verified USDC + THB deposits combined.
    pub deposited_count: u64,
    /// Attendees who checked in.
    pub checked_in_count: u64,
    /// Registered but did not check in (`registered − checked_in`).
    pub no_show_count: u64,
    /// Attendees whose cNFT badge was claimed/minted.
    pub claimed_count: u64,
    /// Refunds issued (USDC + THB combined).
    pub refunded_count: u64,
    /// Post-event registrations (Phase 3; always 0 in Phase 1).
    #[serde(default)]
    pub post_event_reg_count: u64,
}

/// Financial totals in atomic units.
///
/// `usdc_*` use USDC atomic units (1 USDC = 1_000_000). `thb_*` use satang
/// (1 THB = 100 satang). Convert to human amounts in the UI.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FinancialSnapshot {
    pub usdc_deposited_total: u64,
    /// Phase 1 v1: USDC refunds are not summed (no `refunded` flag on
    /// `deposit_statuses`; tracked on-chain via `onchain_events`). Left at 0
    /// and surfaced honestly in the UI until a refund-accounting follow-up.
    pub usdc_refunded_total: u64,
    pub thb_deposited_total: u64,
    pub thb_refunded_total: u64,
}
