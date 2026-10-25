//! Event reads: by id, slug, active, and the admin listings.

use super::*;
use wasm_bindgen_futures::JsFuture;
use worker::D1Database;
use worker::d1::D1Type;

// ---------------------------------------------------------------------------
// Read operations
// ---------------------------------------------------------------------------

/// Get a single event by ID from D1.
/// Returns None if not found.
pub async fn get_event(db: &D1Database, event_id: &str) -> Result<Option<D1EventRow>, String> {
    get_event_raw(db, "id", event_id).await
}

/// Get an event from D1 by column value using raw JSON deserialization.
///
/// Bypasses `first::<D1EventRow>()` which uses `serde_wasm_bindgen::from_value`
/// with `.unwrap()` — panics on certain nullable column types.
async fn get_event_raw(
    db: &D1Database,
    column: &'static str,
    value: &str,
) -> Result<Option<D1EventRow>, String> {
    // `column` is `&'static str`, so only a compile-time literal ("id" / "slug")
    // can reach the SQL — SQLite cannot parameterise an identifier, and the type
    // is what makes the interpolation safe. `value` comes from a URL path
    // segment and is bound.
    let sql = format!("SELECT * FROM events WHERE {column} = ? LIMIT 1");
    let args = [D1Type::Text(value)];
    let stmt = db
        .prepare(&sql)
        .bind_refs(&args)
        .map_err(|e| format!("D1 get_event_raw bind: {e:?}"))?;
    let raw_first = JsFuture::from(
        stmt.inner()
            .first(None)
            .map_err(|e| format!("D1 get_event_raw first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_event_raw first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    let row: D1EventRow = serde_json::from_str(&json_str)
        .map_err(|e| format!("D1 get_event_raw deserialize: {e:?}"))?;
    Ok(Some(row))
}

/// Get the first active event from D1.
pub async fn get_active_event(db: &D1Database) -> Result<Option<D1EventRow>, String> {
    let sql = "SELECT * FROM events WHERE status = 'active' ORDER BY created_at DESC LIMIT 1";
    let stmt = db.prepare(sql);
    let raw_first = JsFuture::from(
        stmt.inner()
            .first(None)
            .map_err(|e| format!("D1 get_active_event first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_active_event first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    let row: D1EventRow = serde_json::from_str(&json_str)
        .map_err(|e| format!("D1 get_active_event deserialize: {e:?}"))?;
    Ok(Some(row))
}

/// Get an event from D1 by slug.
pub async fn get_event_by_slug(db: &D1Database, slug: &str) -> Result<Option<D1EventRow>, String> {
    get_event_raw(db, "slug", slug).await
}

/// List all events from D1 (for index rebuild).
pub async fn list_events(db: &D1Database) -> Result<Vec<D1EventRow>, String> {
    let sql = "SELECT * FROM events ORDER BY created_at DESC";
    let stmt = db.prepare(sql);

    // Bypass D1Result::results() — it uses serde_wasm_bindgen::from_value().unwrap()
    // which panics on NULL columns. Use raw JS interop + JSON.stringify instead.
    let raw_result = JsFuture::from(
        stmt.inner()
            .all()
            .map_err(|e| format!("D1 list_events all() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 list_events all() await: {e:?}"))?;

    let results_key = wasm_bindgen::JsValue::from_str("results");
    let raw_rows =
        js_sys::Reflect::get(&raw_result, &results_key).unwrap_or(wasm_bindgen::JsValue::NULL);

    let json_str = js_sys::JSON::stringify(&raw_rows)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    let rows: Vec<D1EventRow> = serde_json::from_str(&json_str)
        .map_err(|e| format!("D1 list_events deserialize: {e:?}"))?;

    Ok(rows)
}

/// List all events from D1 as `EventMeta` (for KV fallback).
pub async fn list_events_as_meta(
    db: &D1Database,
) -> Result<Vec<event_checkin_domain::models::event::EventMeta>, String> {
    let rows = list_events(db).await?;
    Ok(rows.iter().map(|r| r.to_event_config().to_meta()).collect())
}

/// One authorization-filtered keyset page for the admin event listing.
pub struct EventMetaPage {
    pub events: Vec<event_checkin_domain::models::event::EventMeta>,
    pub next_cursor: Option<String>,
}

#[derive(Debug)]
pub enum EventPageError {
    InvalidCursor,
    Storage(String),
}

impl std::fmt::Display for EventPageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCursor => formatter.write_str("invalid or expired event cursor"),
            Self::Storage(error) => formatter.write_str(error),
        }
    }
}

/// Read a bounded event page from D1. `email=None` grants the already-checked
/// super-admin view; every other caller is filtered before rows leave D1.
pub async fn list_event_meta_page(
    db: &D1Database,
    email: Option<&str>,
    cursor: Option<&str>,
    limit: usize,
) -> Result<EventMetaPage, EventPageError> {
    if limit == 0 || limit >= i32::MAX as usize {
        return Err(EventPageError::Storage("invalid event page limit".into()));
    }
    let (cursor_created_at, cursor_id) = match cursor {
        None => (String::new(), String::new()),
        Some(raw) => {
            let (source, created_at, id): (String, String, String) =
                serde_json::from_str(raw).map_err(|_| EventPageError::InvalidCursor)?;
            if source != "d1" || created_at.is_empty() || id.is_empty() {
                return Err(EventPageError::InvalidCursor);
            }
            (created_at, id)
        }
    };
    let is_super_admin = email.is_none();
    let email = email.unwrap_or("").trim().to_lowercase();
    let fetch_limit = limit
        .checked_add(1)
        .ok_or_else(|| EventPageError::Storage("event page limit overflow".into()))?;
    let sql = include_str!("../sql/list_event_meta_page.sql");
    let stmt = db
        .prepare(sql)
        .bind_refs(&[
            D1Type::Integer(i32::from(is_super_admin)),
            D1Type::Text(&email),
            D1Type::Text(&cursor_created_at),
            D1Type::Text(&cursor_id),
            D1Type::Integer(fetch_limit as i32),
        ])
        .map_err(|e| EventPageError::Storage(format!("D1 list_event_meta_page bind: {e:?}")))?;
    let values = crate::db::d1_safe::safe_all_rows(&stmt)
        .await
        .map_err(|e| EventPageError::Storage(format!("D1 list_event_meta_page execute: {e}")))?;
    let mut rows: Vec<D1EventRow> = values
        .into_iter()
        .map(|value| {
            serde_json::from_value(value).map_err(|e| {
                EventPageError::Storage(format!("D1 list_event_meta_page deserialize: {e}"))
            })
        })
        .collect::<Result<_, _>>()?;
    let has_more = rows.len() > limit;
    if has_more {
        rows.pop();
    }
    let next_cursor = if has_more {
        rows.last()
            .map(|row| {
                serde_json::to_string(&(
                    "d1",
                    row.created_at.as_deref().unwrap_or(""),
                    row.id.as_deref().unwrap_or(""),
                ))
                .map_err(|e| EventPageError::Storage(format!("D1 event cursor serialize: {e}")))
            })
            .transpose()?
    } else {
        None
    };
    Ok(EventMetaPage {
        events: rows
            .iter()
            .map(|row| row.to_event_config().to_meta())
            .collect(),
        next_cursor,
    })
}
