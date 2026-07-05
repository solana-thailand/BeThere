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

/// Public recap content for a completed event (Plan 008 — Phase 2).
///
/// Author-authored markdown + hero image, stored on the `event_summaries` row
/// alongside the frozen snapshot. Recap publishing is gated on a frozen
/// summary existing — "recaps without numbers are misleading" (Plan 008 §3.2.1).
///
/// Returned by:
///   - `GET /api/events/{id}/recap` (organizer; includes draft state)
///   - `GET /api/public/event/{slug}/recap` (public; only when `published_at` is `Some`)
///
/// `frozen_at` is mirrored from the parent summary row so the public recap
/// page can render "Snapshot frozen {date}" without a second lookup.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct EventRecap {
    pub event_id: String,
    /// Markdown body (≤ 16 KB enforced server-side).
    #[serde(default)]
    pub recap_markdown: String,
    /// Hero image URL (https only; empty = no image).
    #[serde(default)]
    pub recap_image_url: String,
    /// ISO 8601 timestamp the recap was published at. `None` = draft (not yet
    /// visible publicly). Toggled by `PUT /api/events/{id}/recap` with
    /// `publish: bool`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recap_published_at: Option<String>,
    /// ISO 8601 timestamp the underlying summary was frozen at. Mirrored from
    /// `event_summaries.frozen_at` for display on the public recap page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen_at: Option<String>,
}

/// Funnel counters — the top-line attendance funnel.
///
/// Wire-ready (Plan 014 Phase 1.2a): pure `u64` fields, `#[repr(C)]` layout.
/// Under the `wire` feature this also derives `Pod`/`Zeroable` so it can be
/// shipped as a zero-copy binary blob alongside the canonical JSON form.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(C)]
#[cfg_attr(feature = "wire", derive(bytemuck::Pod, bytemuck::Zeroable))]
pub struct FunnelSnapshot {
    /// Approved pre-event registrations (all tracks: in-person + online).
    pub registered_count: u64,
    /// Verified USDC + THB deposits combined.
    pub deposited_count: u64,
    /// Attendees who checked in (physical + virtual).
    pub checked_in_count: u64,
    /// In-person registrants who did **not** check in. Computed only across the
    /// in-person slice: `in_person_registered − in_person_checked_in`. Online
    /// attendees are excluded because their attendance is not signaled by
    /// check-in (quest completion is opt-in, and joining the call isn't
    /// recorded) — counting them as no-shows is misleading.
    pub no_show_count: u64,
    /// Attendees whose cNFT badge was claimed/minted.
    pub claimed_count: u64,
    /// Refunds issued (USDC + THB combined).
    pub refunded_count: u64,
    /// Post-event registrations (Phase 3; always 0 in Phase 1).
    #[serde(default)]
    pub post_event_reg_count: u64,
    /// In-person registrants — the denominator for `no_show_count`. Online
    /// attendees are excluded. Mirrors `Attendee::is_in_person()` (empty /
    /// unrecognized defaults to in-person for legacy events).
    #[serde(default)]
    pub in_person_registered_count: u64,
    /// In-person registrants who checked in. `no_show_count` is this subtracted
    /// from `in_person_registered_count`.
    #[serde(default)]
    pub in_person_checked_in_count: u64,
}

/// Financial totals in atomic units.
///
/// `usdc_*` use USDC atomic units (1 USDC = 1_000_000). `thb_*` use satang
/// (1 THB = 100 satang). Convert to human amounts in the UI.
///
/// Wire-ready (Plan 014 Phase 1.2a): pure `u64` fields, `#[repr(C)]` layout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(C)]
#[cfg_attr(feature = "wire", derive(bytemuck::Pod, bytemuck::Zeroable))]
pub struct FinancialSnapshot {
    pub usdc_deposited_total: u64,
    /// Phase 1 v1: USDC refunds are not summed (no `refunded` flag on
    /// `deposit_statuses`; tracked on-chain via `onchain_events`). Left at 0
    /// and surfaced honestly in the UI until a refund-accounting follow-up.
    pub usdc_refunded_total: u64,
    pub thb_deposited_total: u64,
    pub thb_refunded_total: u64,
}
