//! Public event listings (upcoming and past) and per-org counts.

use super::row::parse_enum_column;
use wasm_bindgen_futures::JsFuture;
use worker::D1Database;
use worker::d1::D1Type;

/// List completed events with a published recap, for the public past-events
/// feed (Plan 008 — Phase 2 §3.2.2). Mirrors `list_public_events_raw` but
/// filters on `status = 'completed' AND recap_published = 1` and sorts by
/// `event_end_ms DESC` (most recent events first).
///
/// Returns the same sanitized field set as `list_public_events_raw`, plus
/// `poster_url` (Phase 2 cards prefer the marketing poster over the NFT badge
/// image, matching the dedicated event page hero).
pub async fn list_past_events_raw(db: &D1Database) -> Result<Vec<serde_json::Value>, String> {
    let sql = format!(
        "SELECT {PUBLIC_EVENT_COLUMNS} FROM events \
         WHERE status = 'completed' AND recap_published = 1 \
         AND visibility = 'public' \
         ORDER BY event_end_ms DESC"
    );

    let stmt = db.prepare(&sql);
    let raw_result = JsFuture::from(
        stmt.inner()
            .all()
            .map_err(|e| format!("D1 list_past_events_raw all() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 list_past_events_raw all() await: {e:?}"))?;

    let results_key = wasm_bindgen::JsValue::from_str("results");
    let raw_rows =
        js_sys::Reflect::get(&raw_result, &results_key).unwrap_or(wasm_bindgen::JsValue::NULL);

    let json_str = js_sys::JSON::stringify(&raw_rows)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    let rows: Vec<PublicEventRow> = serde_json::from_str(&json_str).unwrap_or_default();

    Ok(rows.iter().map(public_event_json).collect())
}

/// The row shape behind both public event listings.
///
/// Raw column subset rather than `results::<D1EventRow>()`, which can panic on
/// nullable columns through the workers-rs serde bridge.
///
/// One struct, one column list, one payload builder — because there used to be
/// two of each. `list_past_events_raw` selected and emitted `poster_url` and
/// `list_public_events_raw` did not, so the landing page's only event card fell
/// through to the generic badge image for events that had a real poster
/// (`.issues/094`). Adding the field to one of the two copies is what that
/// defect looks like from the inside; sharing the shape is what stops a third
/// listing re-introducing it.
#[derive(serde::Deserialize)]
struct PublicEventRow {
    id: Option<String>,
    name: Option<String>,
    slug: Option<String>,
    status: Option<String>,
    event_format: Option<String>,
    event_start_ms: Option<i64>,
    event_end_ms: Option<i64>,
    time_tba: Option<i64>,
    deposit_enabled: Option<i64>,
    deposit_amount_thb: Option<i64>,
    tagline: Option<String>,
    location: Option<String>,
    nft_image_url: Option<String>,
    poster_url: Option<String>,
    created_at: Option<String>,
    in_person_capacity: Option<i64>,
    online_capacity: Option<i64>,
    visibility: Option<String>,
    postponed_note: Option<String>,
}

/// The columns both listings select. Kept next to the struct that receives them.
const PUBLIC_EVENT_COLUMNS: &str = "id, name, slug, status, event_format, event_start_ms, \
     event_end_ms, time_tba, deposit_enabled, deposit_amount_thb, tagline, location, \
     nft_image_url, poster_url, created_at, in_person_capacity, online_capacity, visibility, \
     postponed_note";

/// The payload both listings emit.
fn public_event_json(r: &PublicEventRow) -> serde_json::Value {
    use event_checkin_domain::models::event::*;

    let id = r.id.as_deref().unwrap_or("<unknown>");
    let status = parse_enum_column(
        id,
        "status",
        r.status.as_deref(),
        EventStatus::Draft,
        EventStatus::Draft,
    );
    let event_format = parse_enum_column(
        id,
        "event_format",
        r.event_format.as_deref(),
        EventFormat::InPerson,
        EventFormat::InPerson,
    );
    let visibility = parse_enum_column(
        id,
        "visibility",
        r.visibility.as_deref(),
        // NULL predates migration 0016; those events were all public.
        EventVisibility::Public,
        // These listings are public surfaces — a corrupt visibility must not
        // put a private event on one.
        EventVisibility::Private,
    );
    serde_json::json!({
        "id": r.id.clone().unwrap_or_default(),
        "name": r.name.clone().unwrap_or_default(),
        "slug": r.slug.clone().unwrap_or_default(),
        "status": status.as_str(),
        "event_start_ms": r.event_start_ms.unwrap_or(0),
        "event_end_ms": r.event_end_ms.unwrap_or(0),
        "time_tba": r.time_tba.unwrap_or(0) == 1,
        "deposit_enabled": r.deposit_enabled.unwrap_or(0) != 0,
        // The configured PromptPay amount, so a card states the event's own
        // deposit (build plan rule 1), not a literal. 0 = none configured.
        "deposit_amount_thb": r.deposit_amount_thb.unwrap_or(0).max(0),
        "event_format": event_format.as_str(),
        "tagline": r.tagline.clone().unwrap_or_default(),
        "location": r.location.clone().unwrap_or_default(),
        "nft_image_url": r.nft_image_url.clone().unwrap_or_default(),
        "poster_url": r.poster_url.clone().unwrap_or_default(),
        "created_at": r.created_at.clone().unwrap_or_default(),
        "in_person_capacity": r.in_person_capacity.and_then(|v| if v >= 0 { Some(v as u32) } else { None }),
        "online_capacity": r.online_capacity.and_then(|v| if v >= 0 { Some(v as u32) } else { None }),
        "visibility": visibility.as_str(),
        "postponed_note": r.postponed_note.clone().unwrap_or_default(),
    })
}

/// List the events that have not ended by `now_ms`, from D1 using raw JSON
/// deserialization. Bypasses `results::<T>()` to avoid workers-rs serde panics
/// on nullable columns.
///
/// Plan 028 W4: only the end-time half of the landing page's filter runs in
/// SQL, so past events (most rows, and growing) are never read or parsed. It
/// keeps exactly the rows the caller's `end_ms > now_ms` check keeps, since
/// `event_end_ms` is `INTEGER NOT NULL`. Status and visibility stay in the
/// caller, where they are compared after `parse_enum_column` normalises them.
pub async fn list_public_events_raw(
    db: &D1Database,
    now_ms: i64,
) -> Result<Vec<serde_json::Value>, String> {
    let sql = format!(
        "SELECT {PUBLIC_EVENT_COLUMNS} FROM events WHERE event_end_ms > ?1 ORDER BY created_at DESC"
    );

    // Bypass workers-rs D1Result::results() which uses serde_wasm_bindgen::from_value
    // with .unwrap() — panics on nullable columns or type mismatches.
    // Instead, use .all() on the inner JsValue, then stringify and parse via serde_json.
    // `as f64`: D1 binds JS numbers; epoch milliseconds are exact in an f64.
    let stmt = db
        .prepare(&sql)
        .bind_refs(&[D1Type::Real(now_ms as f64)])
        .map_err(|e| format!("D1 list_public_events_raw bind: {e:?}"))?;
    let raw_result = JsFuture::from(
        stmt.inner()
            .all()
            .map_err(|e| format!("D1 list_public_events_raw all() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 list_public_events_raw all() await: {e:?}"))?;

    // Extract the `results` array from the D1 response object
    let results_key = wasm_bindgen::JsValue::from_str("results");
    let raw_rows =
        js_sys::Reflect::get(&raw_result, &results_key).unwrap_or(wasm_bindgen::JsValue::NULL);

    let json_str = js_sys::JSON::stringify(&raw_rows)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    let rows: Vec<PublicEventRow> = serde_json::from_str(&json_str).unwrap_or_default();

    Ok(rows.iter().map(public_event_json).collect())
}

/// Check whether an organization has any non-archived events.
pub async fn has_active_events_for_org(db: &D1Database, org_id: &str) -> Result<bool, String> {
    let stmt = db.prepare(
        "SELECT 1 AS found FROM events WHERE organization_id = ?1 AND status != 'Archived' LIMIT 1",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(org_id)])
        .map_err(|e| format!("D1 has_active_events_for_org bind: {e:?}"))?;

    // Bypass worker crate's .first::<T>() — crashes on JsValue(null).
    let raw_first = JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 has_active_events_for_org first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 has_active_events_for_org first() await: {e:?}"))?;

    Ok(!raw_first.is_null() && !raw_first.is_undefined())
}

/// Count non-archived events for an organization.
pub async fn count_active_events_for_org(db: &D1Database, org_id: &str) -> Result<usize, String> {
    let stmt = db.prepare(
        "SELECT COUNT(*) AS cnt FROM events WHERE organization_id = ?1 AND status != 'Archived'",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(org_id)])
        .map_err(|e| format!("D1 count_active_events_for_org bind: {e:?}"))?;

    // Bypass worker crate's .first::<T>() — crashes on JsValue(null).
    let raw_first = JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 count_active_events_for_org first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 count_active_events_for_org first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(0);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    let row: serde_json::Value = serde_json::from_str(&json_str).unwrap_or_default();
    Ok(row.get("cnt").and_then(|c| c.as_i64()).unwrap_or(0) as usize)
}
