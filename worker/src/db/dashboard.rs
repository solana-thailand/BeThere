//! D1 aggregate query helpers for the live event dashboard.
//!
//! All functions are NULL-safe — they bypass the worker crate's
//! `.first::<T>()` / `.results::<T>()` which panic on `JsValue(null)`.
//! Instead they use raw JS interop + `JSON.stringify` → `serde_json`,
//! matching the pattern in `deposit_statuses::count_deposits_by_event`
//! and `d1_safe::safe_all_rows`.
//!
//! The five public functions back the `GET /api/dashboard/live` endpoint:
//!   1. `count_registered`        — approved attendees
//!   2. `count_checked_in`         — attendees with `checked_in_at` set
//!   3. `count_claims_minted`      — attendees with `claim_asset_id` set
//!   4. `verified_usdc_summary`    — count + SUM(amount) of verified USDC deposits
//!   5. `recent_activity`          — newest audit_log rows for the live feed

use worker::D1Database;
use worker::d1::D1Type;

use crate::db::d1_safe;

// ---------------------------------------------------------------------------
// Public response types
// ---------------------------------------------------------------------------

/// USDC deposit summary for a single event.
///
/// `count` is the number of verified USDC deposits.
/// `total_amount` is the sum of deposit amounts in **atomic USDC units**
/// (1 USDC = 1_000_000), matching how `deposit_statuses.amount` is stored.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct UsdcSummary {
    pub count: u64,
    pub total_amount: u64,
}

/// A single entry in the dashboard's live activity feed.
///
/// Sourced from the append-only `audit_log` table; trimmed to the columns
/// the dashboard UI needs to keep the 2.5s-poll payload small.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ActivityEntry {
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Aggregate queries
// ---------------------------------------------------------------------------

/// Count approved attendees (registered) for an event.
///
/// Computed live from the source-of-truth `attendees` table rather than the
/// cached `events.total_attendees` column, which can drift under concurrent
/// writes during a live check-in demo.
pub async fn count_registered(db: &D1Database, event_id: &str) -> Result<u64, String> {
    count_attendees_by_predicate(
        db,
        event_id,
        "approval_status = 'approved'",
        "count_registered",
    )
    .await
}

/// Count attendees who have checked in for an event.
///
/// `checked_in_at IS NOT NULL` is the canonical check-in signal — it is set
/// by `db::attendees::check_in_attendee` and cleared by `undo_check_in`.
pub async fn count_checked_in(db: &D1Database, event_id: &str) -> Result<u64, String> {
    count_attendees_by_predicate(
        db,
        event_id,
        "checked_in_at IS NOT NULL",
        "count_checked_in",
    )
    .await
}

/// Count attendees whose cNFT badge has been minted.
///
/// `claim_asset_id` is populated atomically by the claim handler after the
/// compressed NFT mint confirms. Empty-string guard covers rows written
/// before the claim flow backfilled the column.
pub async fn count_claims_minted(db: &D1Database, event_id: &str) -> Result<u64, String> {
    count_attendees_by_predicate(
        db,
        event_id,
        "claim_asset_id IS NOT NULL AND claim_asset_id != ''",
        "count_claims_minted",
    )
    .await
}

/// SQL fragment matching in-person attendees. Mirrors the Rust logic in
/// `Attendee::is_in_person()` (domain crate): empty / unrecognized values
/// default to in-person for legacy events, and substring matching covers the
/// sheet's inconsistent capitalization / spacing
/// ("In-Person", "in person", "IN_PERSON", ...).
///
/// Kept as a single `const` so the registered and checked-in in-person counts
/// use the identical predicate and can never drift.
const IN_PERSON_PREDICATE: &str = "(\
    participation_type IS NULL \
    OR TRIM(LOWER(participation_type)) = '' \
    OR LOWER(participation_type) LIKE '%in-person%' \
    OR LOWER(participation_type) LIKE '%in person%' \
    OR LOWER(participation_type) LIKE '%in_person%'\
)";

/// Count approved in-person registrants. Used as the no-show denominator.
///
/// Online attendees are excluded because their attendance is not signaled by
/// check-in (quest completion is opt-in; joining the call isn't recorded), so
/// counting them as no-shows is misleading — see Plan 008 follow-up.
pub async fn count_in_person_registered(db: &D1Database, event_id: &str) -> Result<u64, String> {
    count_attendees_by_predicate(
        db,
        event_id,
        IN_PERSON_PREDICATE,
        "count_in_person_registered",
    )
    .await
}

/// Count in-person registrants who have checked in. `no_show_count` is this
/// subtracted from `count_in_person_registered`.
pub async fn count_in_person_checked_in(db: &D1Database, event_id: &str) -> Result<u64, String> {
    let predicate: &'static str = "(checked_in_at IS NOT NULL)";
    // Combine the in-person predicate with the check-in predicate at the SQL
    // level via an AND, keeping a single COUNT(*) round-trip.
    let sql = format!(
        "SELECT COUNT(*) AS cnt FROM attendees \
         WHERE event_id = ?1 AND {IN_PERSON_PREDICATE} AND ({predicate})"
    );
    let stmt = db.prepare(&sql);
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 count_in_person_checked_in bind: {e:?}"))?;

    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 count_in_person_checked_in first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 count_in_person_checked_in first() await: {e:?}"))?;

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

/// Aggregate verified USDC deposits for an event: count + total amount.
///
/// Reads from `deposit_statuses` (Phase 3e source of truth), not the legacy
/// `attendees.deposit_status` text column. `COALESCE(SUM(amount), 0)` guards
/// the empty-table case where `SUM` would otherwise return SQL `NULL`.
pub async fn verified_usdc_summary(db: &D1Database, event_id: &str) -> Result<UsdcSummary, String> {
    let stmt = db.prepare(
        "SELECT COUNT(*) AS cnt, COALESCE(SUM(amount), 0) AS total_usdc \
         FROM deposit_statuses \
         WHERE event_id = ?1 AND verified = 1 AND method = 'usdc'",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 verified_usdc_summary bind: {e:?}"))?;

    // Raw JS `.first()` + JSON.stringify bypasses the worker crate's
    // `.first::<T>()` which crashes on `JsValue(null)`.
    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 verified_usdc_summary first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 verified_usdc_summary first() await: {e:?}"))?;

    // COUNT(*) always returns exactly one row, but be defensive: if the D1
    // binding returns null/undefined, treat it as "no data" rather than panic.
    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(UsdcSummary::default());
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    if json_str.is_empty() {
        return Ok(UsdcSummary::default());
    }

    let row: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(
            error = %e,
            json = %json_str.chars().take(300).collect::<String>(),
            "D1 verified_usdc_summary: deserialize failed"
        );
        format!("D1 verified_usdc_summary deserialize: {e}")
    })?;

    Ok(UsdcSummary {
        count: row.get("cnt").and_then(|v| v.as_i64()).unwrap_or(0) as u64,
        total_amount: row.get("total_usdc").and_then(|v| v.as_i64()).unwrap_or(0) as u64,
    })
}

/// Fetch the most recent audit entries for the live activity feed.
///
/// Wraps the existing `audit_log` read path via `d1_safe::safe_all_rows`,
/// selecting only the columns the dashboard renders. Ordered newest-first
/// using the indexed `(event_id, timestamp DESC)` access path so polling
/// stays cheap even as the audit log grows.
pub async fn recent_activity(
    db: &D1Database,
    event_id: &str,
    limit: usize,
) -> Result<Vec<ActivityEntry>, String> {
    let stmt = db
        .prepare(
            "SELECT timestamp, actor, action, description \
             FROM audit_log \
             WHERE event_id = ?1 \
             ORDER BY timestamp DESC LIMIT ?2",
        )
        .bind_refs(&[D1Type::Text(event_id), D1Type::Integer(limit as i32)])
        .map_err(|e| format!("D1 recent_activity bind: {e:?}"))?;

    let rows = d1_safe::safe_all_rows(&stmt).await?;

    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        entries.push(ActivityEntry {
            timestamp: extract_str(&row, "timestamp"),
            actor: extract_str(&row, "actor"),
            action: extract_str(&row, "action"),
            description: extract_str(&row, "description"),
        });
    }
    Ok(entries)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Run `SELECT COUNT(*) AS cnt FROM attendees WHERE event_id = ?1 AND (predicate)`.
///
/// Centralizes the NULL-safe raw JS `.first()` + `JSON.stringify` pattern so
/// the three attendee-count queries share one defensive implementation.
///
/// # Safety
/// `predicate` is `&'static str` — only compile-time string literals can be
/// passed, guaranteeing no SQL injection even though it is interpolated into
/// the SQL string rather than bound. The only user-influenced value
/// (`event_id`) is bound via parameterized `?1`.
async fn count_attendees_by_predicate(
    db: &D1Database,
    event_id: &str,
    predicate: &'static str,
    label: &'static str,
) -> Result<u64, String> {
    let sql =
        format!("SELECT COUNT(*) AS cnt FROM attendees WHERE event_id = ?1 AND ({predicate})");
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
        return Ok(0);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    if json_str.is_empty() {
        return Ok(0);
    }

    let row: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(
            error = %e,
            json = %json_str.chars().take(300).collect::<String>(),
            "D1 {label}: deserialize failed"
        );
        format!("D1 {label} deserialize: {e}")
    })?;

    Ok(row.get("cnt").and_then(|v| v.as_i64()).unwrap_or(0) as u64)
}

/// Extract a string field from a `serde_json::Value` row, defaulting to "".
///
/// Shared by `recent_activity` so each field read is one line and the
/// NULL-to-empty-string coercion is uniform.
#[inline]
fn extract_str(row: &serde_json::Value, field: &str) -> String {
    row.get(field)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usdc_summary_default_is_zero() {
        let s = UsdcSummary::default();
        assert_eq!(s.count, 0);
        assert_eq!(s.total_amount, 0);
    }

    #[test]
    fn extract_str_handles_missing_field() {
        let row: serde_json::Value = serde_json::json!({ "actor": "alice" });
        assert_eq!(extract_str(&row, "actor"), "alice");
        assert_eq!(extract_str(&row, "missing"), "");
    }

    #[test]
    fn extract_str_handles_null_field() {
        let row: serde_json::Value = serde_json::json!({ "actor": null });
        assert_eq!(extract_str(&row, "actor"), "");
    }
}
