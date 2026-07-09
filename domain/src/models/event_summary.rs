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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    /// Wire-contract round-trip helper for summary types.
    ///
    /// Mirrors the intent of `frontend-leptos/tests/serde_contract.rs` and
    /// `worker/tests/serde_contract.rs::assert_round_trip` — but compares via
    /// JSON re-serialization rather than `PartialEq` on the value itself. This
    /// lets us lock the wire contract for `EventSummary` / `EventRecap`, which
    /// intentionally do not derive `PartialEq` (they carry only `Serialize`/
    /// `Deserialize` for the API boundary).
    ///
    /// Asserts both directions:
    ///   1. `serialize(value)      == expected_json` — produced wire shape is stable
    ///   2. `serialize(parse(json)) == expected_json` — defaults don't mutate on round-trip
    fn assert_wire_contract<T>(value: &T, expected_json: &str)
    where
        T: Serialize + DeserializeOwned + std::fmt::Debug,
    {
        let actual_json = serde_json::to_string(value)
            .unwrap_or_else(|e| panic!("failed to serialize {:?}: {e}", value));
        assert_eq!(
            actual_json, expected_json,
            "serialization mismatch for {:?}",
            value
        );

        let reparsed: T = serde_json::from_str(expected_json).unwrap_or_else(|e| {
            panic!(
                "failed to deserialize {expected_json} into {}: {e}",
                std::any::type_name::<T>()
            )
        });
        let re_serialized = serde_json::to_string(&reparsed)
            .unwrap_or_else(|e| panic!("failed to re-serialize {reparsed:?}: {e}"));
        assert_eq!(
            re_serialized, expected_json,
            "round-trip serialization mutated the wire shape"
        );
    }

    // -----------------------------------------------------------------------
    // FunnelSnapshot
    // -----------------------------------------------------------------------

    #[test]
    fn funnel_snapshot_round_trip() {
        let snap = FunnelSnapshot {
            registered_count: 100,
            deposited_count: 50,
            checked_in_count: 40,
            no_show_count: 10,
            claimed_count: 35,
            refunded_count: 5,
            post_event_reg_count: 2,
            in_person_registered_count: 80,
            in_person_checked_in_count: 40,
        };
        assert_wire_contract(
            &snap,
            r#"{"registered_count":100,"deposited_count":50,"checked_in_count":40,"no_show_count":10,"claimed_count":35,"refunded_count":5,"post_event_reg_count":2,"in_person_registered_count":80,"in_person_checked_in_count":40}"#,
        );
    }

    /// Pre-Phase-3 payloads omit the three `#[serde(default)]` fields
    /// (`post_event_reg_count`, `in_person_registered_count`,
    /// `in_person_checked_in_count`). Deserialization must succeed and default
    /// them to 0 — backward compatibility for stored rows written before
    /// Phase 3 rolled out.
    #[test]
    fn funnel_snapshot_legacy_payload_defaults_missing_fields() {
        let json = r#"{"registered_count":100,"deposited_count":50,"checked_in_count":40,"no_show_count":10,"claimed_count":35,"refunded_count":5}"#;
        let parsed: FunnelSnapshot =
            serde_json::from_str(json).expect("legacy funnel JSON must deserialize");

        // Fields present in the legacy payload are preserved verbatim.
        assert_eq!(parsed.registered_count, 100);
        assert_eq!(parsed.deposited_count, 50);
        assert_eq!(parsed.checked_in_count, 40);
        assert_eq!(parsed.no_show_count, 10);
        assert_eq!(parsed.claimed_count, 35);
        assert_eq!(parsed.refunded_count, 5);

        // Fields missing from the legacy payload default to 0 (Phase 3 fields).
        assert_eq!(
            parsed.post_event_reg_count, 0,
            "missing post_event_reg_count must default to 0"
        );
        assert_eq!(
            parsed.in_person_registered_count, 0,
            "missing in_person_registered_count must default to 0"
        );
        assert_eq!(
            parsed.in_person_checked_in_count, 0,
            "missing in_person_checked_in_count must default to 0"
        );
    }

    // -----------------------------------------------------------------------
    // FinancialSnapshot
    // -----------------------------------------------------------------------

    #[test]
    fn financial_snapshot_round_trip() {
        let snap = FinancialSnapshot {
            usdc_deposited_total: 50_000_000,
            usdc_refunded_total: 5_000_000,
            thb_deposited_total: 100_000,
            thb_refunded_total: 10_000,
        };
        assert_wire_contract(
            &snap,
            r#"{"usdc_deposited_total":50000000,"usdc_refunded_total":5000000,"thb_deposited_total":100000,"thb_refunded_total":10000}"#,
        );
    }

    // -----------------------------------------------------------------------
    // EventSummary — frozen (persisted) vs live preview
    // -----------------------------------------------------------------------

    fn sample_funnel() -> FunnelSnapshot {
        FunnelSnapshot {
            registered_count: 10,
            deposited_count: 5,
            checked_in_count: 4,
            no_show_count: 1,
            claimed_count: 3,
            refunded_count: 0,
            post_event_reg_count: 0,
            in_person_registered_count: 8,
            in_person_checked_in_count: 4,
        }
    }

    fn sample_financials() -> FinancialSnapshot {
        FinancialSnapshot {
            usdc_deposited_total: 5_000_000,
            usdc_refunded_total: 0,
            thb_deposited_total: 0,
            thb_refunded_total: 0,
        }
    }

    /// A frozen summary serializes `frozen_at` and `frozen_by` — the persisted
    /// shape read back from the `event_summaries` row.
    #[test]
    fn event_summary_round_trip_when_frozen() {
        let summary = EventSummary {
            event_id: "evt-1".to_string(),
            funnel: sample_funnel(),
            financials: sample_financials(),
            event_start_ms: 1_700_000_000_000,
            event_end_ms: 1_700_003_600_000,
            frozen_at: Some("2024-01-01T00:00:00Z".to_string()),
            frozen_by: "organizer@example.com".to_string(),
        };
        assert_wire_contract(
            &summary,
            r#"{"event_id":"evt-1","funnel":{"registered_count":10,"deposited_count":5,"checked_in_count":4,"no_show_count":1,"claimed_count":3,"refunded_count":0,"post_event_reg_count":0,"in_person_registered_count":8,"in_person_checked_in_count":4},"financials":{"usdc_deposited_total":5000000,"usdc_refunded_total":0,"thb_deposited_total":0,"thb_refunded_total":0},"event_start_ms":1700000000000,"event_end_ms":1700003600000,"frozen_at":"2024-01-01T00:00:00Z","frozen_by":"organizer@example.com"}"#,
        );
    }

    /// A live preview (not yet persisted) has `frozen_at = None` and the field
    /// is omitted from the wire shape via `skip_serializing_if = "Option::is_none"`.
    /// `frozen_by` is always emitted (no skip directive) — empty string in this
    /// case, which is the documented sentinel for "no human froze this yet".
    #[test]
    fn event_summary_live_preview_omits_frozen_at() {
        let summary = EventSummary {
            event_id: "evt-1".to_string(),
            funnel: sample_funnel(),
            financials: sample_financials(),
            event_start_ms: 1_700_000_000_000,
            event_end_ms: 1_700_003_600_000,
            frozen_at: None,
            frozen_by: String::new(),
        };
        assert_wire_contract(
            &summary,
            r#"{"event_id":"evt-1","funnel":{"registered_count":10,"deposited_count":5,"checked_in_count":4,"no_show_count":1,"claimed_count":3,"refunded_count":0,"post_event_reg_count":0,"in_person_registered_count":8,"in_person_checked_in_count":4},"financials":{"usdc_deposited_total":5000000,"usdc_refunded_total":0,"thb_deposited_total":0,"thb_refunded_total":0},"event_start_ms":1700000000000,"event_end_ms":1700003600000,"frozen_by":""}"#,
        );
    }

    // -----------------------------------------------------------------------
    // EventRecap — draft vs published
    // -----------------------------------------------------------------------

    /// A draft recap (not yet published, summary not yet frozen) omits both
    /// `recap_published_at` and `frozen_at` from the wire shape. This is what
    /// the organizer sees while editing.
    #[test]
    fn event_recap_draft_omits_optional_timestamps() {
        let recap = EventRecap {
            event_id: "evt-1".to_string(),
            // NOTE: avoid `#` in markdown body — `r#"..."#` raw strings would
            // close prematurely on the `"#` inside `"## ...`. Realistic body
            // is irrelevant here; this test locks the JSON wire shape only.
            recap_markdown: "Highlights from the event.".to_string(),
            recap_image_url: "https://cdn.example/hero.png".to_string(),
            recap_published_at: None,
            frozen_at: None,
        };
        assert_wire_contract(
            &recap,
            r#"{"event_id":"evt-1","recap_markdown":"Highlights from the event.","recap_image_url":"https://cdn.example/hero.png"}"#,
        );
    }

    /// A published recap emits both timestamps. This is the public wire shape
    /// returned by `GET /api/public/event/{slug}/recap`.
    #[test]
    fn event_recap_published_round_trip() {
        let recap = EventRecap {
            event_id: "evt-1".to_string(),
            recap_markdown: "Highlights from the event.".to_string(),
            recap_image_url: "https://cdn.example/hero.png".to_string(),
            recap_published_at: Some("2024-01-02T00:00:00Z".to_string()),
            frozen_at: Some("2024-01-01T00:00:00Z".to_string()),
        };
        assert_wire_contract(
            &recap,
            r#"{"event_id":"evt-1","recap_markdown":"Highlights from the event.","recap_image_url":"https://cdn.example/hero.png","recap_published_at":"2024-01-02T00:00:00Z","frozen_at":"2024-01-01T00:00:00Z"}"#,
        );
    }
}
