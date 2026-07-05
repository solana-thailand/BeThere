//! D1 helpers for the post-event summary (Plan 008 — Phase 1).
//!
//! Three responsibilities:
//!   1. `get_summary`       — read a persisted frozen snapshot row.
//!   2. `upsert_summary`    — persist a freshly-computed snapshot (the freeze).
//!   3. `compute_snapshot`  — aggregate the live funnel + financials from the
//!      source tables, reusing `db::dashboard` primitives where they exist.
//!
//! All reads are NULL-safe: they bypass the worker crate's `.first::<T>()`
//! (which panics on `JsValue(null)`) via raw JS `.first(None)` + `JSON.stringify`
//! → serde_json, matching the pattern in `db/dashboard.rs`.

use chrono::Utc;
use worker::D1Database;
use worker::d1::D1Type;

use event_checkin_domain::models::event::EventConfig;
use event_checkin_domain::models::event_summary::{
    EventSummary, FinancialSnapshot, FunnelSnapshot,
};

use crate::db::dashboard;

// ---------------------------------------------------------------------------
// Persisted read / write
// ---------------------------------------------------------------------------

/// Read a persisted frozen summary for an event, if one exists.
pub async fn get_summary(db: &D1Database, event_id: &str) -> Result<Option<EventSummary>, String> {
    let stmt = db.prepare("SELECT * FROM event_summaries WHERE event_id = ?1");
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 get_summary bind: {e:?}"))?;

    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 get_summary first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_summary first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    if json_str.is_empty() {
        return Ok(None);
    }

    let row: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(
            error = %e,
            json = %json_str.chars().take(300).collect::<String>(),
            "D1 get_summary: deserialize failed"
        );
        format!("D1 get_summary deserialize: {e}")
    })?;

    Ok(Some(row_to_summary(row)))
}

/// Persist (freeze) a computed snapshot. Replaces any prior freeze for the event.
///
/// `frozen_by` is the actor email; pass `""` for an automatic freeze.
///
/// Uses string-interpolation (the `raw_sql` convention) rather than `bind_refs`
/// because D1's `D1Type::Integer` only accepts `i32`, but financial totals and
/// millisecond timestamps are `i64` and can exceed `i32` range. This mirrors
/// `db::events::upsert_event`. All interpolated values are server-computed
/// integers or the organizer's own event id (already authorized), so there is
/// no injection surface.
pub async fn upsert_summary(
    db: &D1Database,
    summary: &EventSummary,
    frozen_by: &str,
) -> Result<(), String> {
    let frozen_at = summary
        .frozen_at
        .clone()
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let f = &summary.funnel;
    let fin = &summary.financials;

    // Escape single quotes in text fields to keep the interpolated SQL valid.
    // These values are never attacker-controlled (actor email / event id), but
    // defensive escaping is cheap and matches good hygiene.
    let esc = |s: &str| s.replace('"', "''");

    let sql = format!(
        "INSERT INTO event_summaries (\
            event_id, registered_count, deposited_count, checked_in_count, no_show_count, \
            claimed_count, refunded_count, post_event_reg_count, \
            in_person_registered_count, in_person_checked_in_count, \
            usdc_deposited_total, usdc_refunded_total, thb_deposited_total, thb_refunded_total, \
            event_start_ms, event_end_ms, frozen_at, frozen_by, updated_at\
         ) VALUES (\
            '{event_id}', {registered}, {deposited}, {checked_in}, {no_show}, \
            {claimed}, {refunded}, {post_event_reg}, \
            {in_person_reg}, {in_person_chk}, \
            {usdc_dep}, {usdc_ref}, {thb_dep}, {thb_ref}, \
            {start_ms}, {end_ms}, '{frozen_at}', '{frozen_by}', datetime('now')\
         ) ON CONFLICT(event_id) DO UPDATE SET \
            registered_count=excluded.registered_count, deposited_count=excluded.deposited_count, \
            checked_in_count=excluded.checked_in_count, no_show_count=excluded.no_show_count, \
            claimed_count=excluded.claimed_count, refunded_count=excluded.refunded_count, \
            post_event_reg_count=excluded.post_event_reg_count, \
            in_person_registered_count=excluded.in_person_registered_count, \
            in_person_checked_in_count=excluded.in_person_checked_in_count, \
            usdc_deposited_total=excluded.usdc_deposited_total, \
            usdc_refunded_total=excluded.usdc_refunded_total, \
            thb_deposited_total=excluded.thb_deposited_total, \
            thb_refunded_total=excluded.thb_refunded_total, \
            event_start_ms=excluded.event_start_ms, event_end_ms=excluded.event_end_ms, \
            frozen_at=excluded.frozen_at, frozen_by=excluded.frozen_by, \
            updated_at=datetime('now')",
        event_id = esc(&summary.event_id),
        registered = f.registered_count,
        deposited = f.deposited_count,
        checked_in = f.checked_in_count,
        no_show = f.no_show_count,
        claimed = f.claimed_count,
        refunded = f.refunded_count,
        post_event_reg = f.post_event_reg_count,
        in_person_reg = f.in_person_registered_count,
        in_person_chk = f.in_person_checked_in_count,
        usdc_dep = fin.usdc_deposited_total,
        usdc_ref = fin.usdc_refunded_total,
        thb_dep = fin.thb_deposited_total,
        thb_ref = fin.thb_refunded_total,
        start_ms = summary.event_start_ms,
        end_ms = summary.event_end_ms,
        frozen_at = esc(&frozen_at),
        frozen_by = esc(frozen_by),
    );

    db.prepare(&sql)
        .run()
        .await
        .map_err(|e| format!("D1 upsert_summary run: {e:?}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Recap authoring (Plan 008 — Phase 2)
// ---------------------------------------------------------------------------

/// Maximum allowed markdown body length (16 KB). Guards D1 row size + worker
/// memory when rendering. Enforced by the `put_recap` handler before this call.
pub const MAX_RECAP_MARKDOWN_BYTES: usize = 16 * 1024;

/// Read the recap slice of an `event_summaries` row.
///
/// Returns `Ok(None)` when no summary row exists yet (the organizer hasn't
/// frozen a snapshot). The caller decides whether that's an error (PUT path:
/// refuse to publish without a freeze) or an empty draft (GET path).
pub async fn get_recap(
    db: &D1Database,
    event_id: &str,
) -> Result<Option<event_checkin_domain::models::event_summary::EventRecap>, String> {
    use event_checkin_domain::models::event_summary::EventRecap;

    let sql = "SELECT event_id, recap_markdown, recap_image_url, recap_published_at, frozen_at \
               FROM event_summaries WHERE event_id = ?1";
    let stmt = db.prepare(sql);
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 get_recap bind: {e:?}"))?;

    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 get_recap first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_recap first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();
    if json_str.is_empty() {
        return Ok(None);
    }

    let row: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(
            error = %e,
            json = %json_str.chars().take(300).collect::<String>(),
            "D1 get_recap: deserialize failed"
        );
        format!("D1 get_recap deserialize: {e}")
    })?;

    let get_str = |field: &str| -> String {
        row.get(field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let parse_ts = |field: &str| -> Option<String> {
        let s = get_str(field);
        if s.is_empty() { None } else { Some(s) }
    };

    Ok(Some(EventRecap {
        event_id: get_str("event_id"),
        recap_markdown: get_str("recap_markdown"),
        recap_image_url: get_str("recap_image_url"),
        recap_published_at: parse_ts("recap_published_at"),
        frozen_at: parse_ts("frozen_at"),
    }))
}

/// Persist recap content + publish state onto an existing `event_summaries` row.
///
/// Callers must have already verified that a frozen row exists (the handler
/// refuses to publish a recap without a frozen summary — Plan 008 §3.2.1).
/// `published_at` is `Some(now_iso)` when publishing, `None` when saving a draft
/// (which clears the public visibility flag).
pub async fn set_recap(
    db: &D1Database,
    event_id: &str,
    markdown: &str,
    image_url: &str,
    published_at: Option<&str>,
) -> Result<(), String> {
    // Escape single quotes in text fields to keep the interpolated SQL valid.
    // Both fields are organizer-authored but authenticated + role-gated; the
    // escaping is defensive against legitimate content (e.g. apostrophes).
    let esc = |s: &str| s.replace('\'', "''");

    let published_sql = match published_at {
        Some(ts) => format!("'{}'", esc(ts)),
        None => "NULL".to_string(),
    };

    let sql = format!(
        "UPDATE event_summaries SET \
            recap_markdown = '{markdown}', \
            recap_image_url = '{image_url}', \
            recap_published_at = {published_sql}, \
            updated_at = datetime('now') \
         WHERE event_id = '{event_id}'",
        markdown = esc(markdown),
        image_url = esc(image_url),
        published_sql = published_sql,
        event_id = esc(event_id),
    );

    db.prepare(&sql)
        .run()
        .await
        .map_err(|e| format!("D1 set_recap run: {e:?}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Live computation
// ---------------------------------------------------------------------------

/// Aggregate the live funnel + financials for an event from the source tables.
///
/// Reuses `db::dashboard` primitives for registered / checked-in / claims /
/// verified-USDC. Adds local queries for THB (deposited + refunded), USDC
/// refunded (Phase 1 v1: 0 — see `FinancialSnapshot` comment), and the
/// deposited-count + refunded-count totals.
///
/// The returned `EventSummary` has `frozen_at = None` (caller sets it on freeze).
pub async fn compute_snapshot(
    db: &D1Database,
    event: &EventConfig,
) -> Result<EventSummary, String> {
    let event_id = event.id.as_str();

    let registered = dashboard::count_registered(db, event_id).await?;
    let checked_in = dashboard::count_checked_in(db, event_id).await?;
    let claimed = dashboard::count_claims_minted(db, event_id).await?;

    // No-show is computed only across the in-person slice. Online attendees'
    // attendance is not signaled by check-in (quest completion is opt-in, and
    // joining the call isn't recorded), so counting unchecked-in online
    // registrants as no-shows is misleading. See Plan 008 follow-up +
    // `dashboard::IN_PERSON_PREDICATE`.
    let in_person_registered = dashboard::count_in_person_registered(db, event_id).await?;
    let in_person_checked_in = dashboard::count_in_person_checked_in(db, event_id).await?;
    let no_show = in_person_registered.saturating_sub(in_person_checked_in);

    // Deposited = verified USDC + verified THB counts.
    let usdc = dashboard::verified_usdc_summary(db, event_id).await?;
    let thb_verified_count = count_verified_thb(db, event_id).await?;
    let deposited = usdc.count + thb_verified_count;

    // Refunds.
    let thb_refunded = thb_refunded_summary(db, event_id).await?;
    // Phase 1 v1: USDC refunds not summed (no flag on deposit_statuses).
    let usdc_refunded_count: u64 = 0;
    let refunded_count = usdc_refunded_count + thb_refunded.count;

    // Financial totals.
    let thb_deposited_total = thb_deposited_total(db, event_id).await?;

    let post_event_reg_count = count_post_event_registrations(db, event_id).await?;

    Ok(EventSummary {
        event_id: event_id.to_string(),
        funnel: FunnelSnapshot {
            registered_count: registered,
            deposited_count: deposited,
            checked_in_count: checked_in,
            no_show_count: no_show,
            claimed_count: claimed,
            refunded_count,
            post_event_reg_count,
            in_person_registered_count: in_person_registered,
            in_person_checked_in_count: in_person_checked_in,
        },
        financials: FinancialSnapshot {
            usdc_deposited_total: usdc.total_amount,
            usdc_refunded_total: 0,
            thb_deposited_total,
            thb_refunded_total: thb_refunded.total_amount,
        },
        event_start_ms: event.event_start_ms,
        event_end_ms: event.event_end_ms,
        frozen_at: None,
        frozen_by: String::new(),
    })
}

// ---------------------------------------------------------------------------
// Local aggregate helpers (NULL-safe single-row aggregates)
// ---------------------------------------------------------------------------

/// `ThbSummary` re-used for both verified-deposit and refund aggregates.
#[derive(Debug, Clone, Default)]
struct ThbSummary {
    count: u64,
    total_amount: u64,
}

/// Count verified THB deposits for an event.
async fn count_verified_thb(db: &D1Database, event_id: &str) -> Result<u64, String> {
    thb_count_predicate(db, event_id, "verified = 1", "count_verified_thb").await
}

/// Count + SUM(amount_thb) of refunded THB deposits.
async fn thb_refunded_summary(db: &D1Database, event_id: &str) -> Result<ThbSummary, String> {
    thb_count_and_sum_predicate(db, event_id, "refunded = 1", "thb_refunded_summary").await
}

/// SUM(amount_thb) of verified THB deposits (atomic satang).
async fn thb_deposited_total(db: &D1Database, event_id: &str) -> Result<u64, String> {
    let s =
        thb_count_and_sum_predicate(db, event_id, "verified = 1", "thb_deposited_total").await?;
    Ok(s.total_amount)
}

/// Count attendees registered in the post-event phase (Phase 3; 0 in Phase 1).
async fn count_post_event_registrations(db: &D1Database, event_id: &str) -> Result<u64, String> {
    count_attendees_by_phase(db, event_id, "post_event").await
}

/// `SELECT COUNT(*) FROM thb_deposits WHERE event_id = ?1 AND (predicate)`.
async fn thb_count_predicate(
    db: &D1Database,
    event_id: &str,
    predicate: &'static str,
    label: &'static str,
) -> Result<u64, String> {
    let sql =
        format!("SELECT COUNT(*) AS cnt FROM thb_deposits WHERE event_id = ?1 AND ({predicate})");
    count_first_int(db, event_id, &sql, label).await
}

/// `SELECT COUNT(*) AS cnt, COALESCE(SUM(amount_thb),0) AS total FROM thb_deposits ...`.
async fn thb_count_and_sum_predicate(
    db: &D1Database,
    event_id: &str,
    predicate: &'static str,
    label: &'static str,
) -> Result<ThbSummary, String> {
    let sql = format!(
        "SELECT COUNT(*) AS cnt, COALESCE(SUM(amount_thb), 0) AS total \
         FROM thb_deposits WHERE event_id = ?1 AND ({predicate})"
    );
    let stmt = db.prepare(&sql);
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 {label} bind: {e:?}"))?;

    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 {label} first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 {label} first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(ThbSummary::default());
    }
    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();
    if json_str.is_empty() {
        return Ok(ThbSummary::default());
    }
    let row: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(error = %e, "D1 {label}: deserialize failed");
        format!("D1 {label} deserialize: {e}")
    })?;

    Ok(ThbSummary {
        count: row.get("cnt").and_then(|v| v.as_i64()).unwrap_or(0) as u64,
        total_amount: row.get("total").and_then(|v| v.as_i64()).unwrap_or(0) as u64,
    })
}

/// `SELECT COUNT(*) FROM attendees WHERE event_id = ?1 AND registration_phase = ?2`.
async fn count_attendees_by_phase(
    db: &D1Database,
    event_id: &str,
    phase: &'static str,
) -> Result<u64, String> {
    let sql =
        "SELECT COUNT(*) AS cnt FROM attendees WHERE event_id = ?1 AND registration_phase = ?2";
    let stmt = db.prepare(sql);
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(phase)])
        .map_err(|e| format!("D1 count_attendees_by_phase bind: {e:?}"))?;

    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 count_attendees_by_phase first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 count_attendees_by_phase first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(0);
    }
    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();
    if json_str.is_empty() {
        return Ok(0);
    }
    let row: serde_json::Value = serde_json::from_str(&json_str).unwrap_or_default();
    Ok(row.get("cnt").and_then(|v| v.as_i64()).unwrap_or(0) as u64)
}

/// Shared `SELECT COUNT(*) AS cnt`-only helper (no SUM column).
async fn count_first_int(
    db: &D1Database,
    event_id: &str,
    sql: &str,
    label: &'static str,
) -> Result<u64, String> {
    let stmt = db.prepare(sql);
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 {label} bind: {e:?}"))?;

    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 {label} first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 {label} first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(0);
    }
    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();
    if json_str.is_empty() {
        return Ok(0);
    }
    let row: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(error = %e, "D1 {label}: deserialize failed");
        format!("D1 {label} deserialize: {e}")
    })?;
    Ok(row.get("cnt").and_then(|v| v.as_i64()).unwrap_or(0) as u64)
}

// ---------------------------------------------------------------------------
// Row → Domain conversion
// ---------------------------------------------------------------------------

fn row_to_summary(row: serde_json::Value) -> EventSummary {
    let get_u64 =
        |field: &str| -> u64 { row.get(field).and_then(|v| v.as_i64()).unwrap_or(0) as u64 };
    let get_str = |field: &str| -> String {
        row.get(field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    EventSummary {
        event_id: get_str("event_id"),
        funnel: FunnelSnapshot {
            registered_count: get_u64("registered_count"),
            deposited_count: get_u64("deposited_count"),
            checked_in_count: get_u64("checked_in_count"),
            no_show_count: get_u64("no_show_count"),
            claimed_count: get_u64("claimed_count"),
            refunded_count: get_u64("refunded_count"),
            post_event_reg_count: get_u64("post_event_reg_count"),
            in_person_registered_count: get_u64("in_person_registered_count"),
            in_person_checked_in_count: get_u64("in_person_checked_in_count"),
        },
        financials: FinancialSnapshot {
            usdc_deposited_total: get_u64("usdc_deposited_total"),
            usdc_refunded_total: get_u64("usdc_refunded_total"),
            thb_deposited_total: get_u64("thb_deposited_total"),
            thb_refunded_total: get_u64("thb_refunded_total"),
        },
        event_start_ms: row
            .get("event_start_ms")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        event_end_ms: row
            .get("event_end_ms")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        frozen_at: {
            let s = get_str("frozen_at");
            if s.is_empty() { None } else { Some(s) }
        },
        frozen_by: get_str("frozen_by"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_to_summary_handles_missing_columns() {
        // A minimal row (only event_id) — every other field defaults to 0 / "".
        let row = serde_json::json!({ "event_id": "evt-1" });
        let s = row_to_summary(row);
        assert_eq!(s.event_id, "evt-1");
        assert_eq!(s.funnel.registered_count, 0);
        assert_eq!(s.financials.thb_deposited_total, 0);
        assert!(s.frozen_at.is_none());
    }

    #[test]
    fn row_to_summary_maps_all_columns() {
        let row = serde_json::json!({
            "event_id": "evt-2",
            "registered_count": 10,
            "deposited_count": 7,
            "checked_in_count": 5,
            "no_show_count": 2,
            "claimed_count": 4,
            "refunded_count": 2,
            "post_event_reg_count": 1,
            "in_person_registered_count": 6,
            "in_person_checked_in_count": 4,
            "usdc_deposited_total": 350000000,
            "usdc_refunded_total": 0,
            "thb_deposited_total": 3500,
            "thb_refunded_total": 1000,
            "event_start_ms": 1000,
            "event_end_ms": 2000,
            "frozen_at": "2026-06-24T00:00:00Z",
            "frozen_by": "ops@example.com"
        });
        let s = row_to_summary(row);
        assert_eq!(s.funnel.checked_in_count, 5);
        // no_show_count is now the in-person slice (6 − 4 = 2), not 10 − 5.
        assert_eq!(s.funnel.no_show_count, 2);
        assert_eq!(s.funnel.in_person_registered_count, 6);
        assert_eq!(s.funnel.in_person_checked_in_count, 4);
        assert_eq!(s.funnel.refunded_count, 2);
        assert_eq!(s.financials.usdc_deposited_total, 350000000);
        assert_eq!(s.financials.thb_deposited_total, 3500);
        assert_eq!(s.frozen_at.as_deref(), Some("2026-06-24T00:00:00Z"));
        assert_eq!(s.frozen_by, "ops@example.com");
    }
}
