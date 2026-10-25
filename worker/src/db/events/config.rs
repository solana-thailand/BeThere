//! Per-event form config and post-event flags.

use wasm_bindgen_futures::JsFuture;
use worker::D1Database;
use worker::d1::D1Type;

// ---------------------------------------------------------------------------
// Form config D1 helpers (Issue #053 Phase 3f)
// ---------------------------------------------------------------------------

/// Get form config for an event from D1.
/// Returns None if no custom config is stored.
pub async fn get_form_config(
    db: &D1Database,
    event_id: &str,
) -> Result<Option<event_checkin_domain::models::event::RegistrationFormConfig>, String> {
    let sql = "SELECT form_config FROM events WHERE id = ? LIMIT 1";
    let bound = db
        .prepare(sql)
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 get_form_config bind: {e:?}"))?;

    // Bypass worker crate's .first::<T>() — crashes on JsValue(null).
    let raw_first = JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 get_form_config first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_form_config first() await: {e:?}"))?;

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
            "D1 get_form_config: deserialize failed"
        );
        format!("D1 get_form_config deserialize: {e}")
    })?;

    let config_str = row.get("form_config").and_then(|v| v.as_str());
    match config_str {
        Some(json) if !json.is_empty() => serde_json::from_str(json)
            .map(Some)
            .map_err(|e| format!("failed to parse form config for event '{event_id}': {e}")),
        _ => Ok(None),
    }
}

/// Save form config for an event to D1.
pub async fn save_form_config(
    db: &D1Database,
    event_id: &str,
    config: &event_checkin_domain::models::event::RegistrationFormConfig,
) -> Result<(), String> {
    let json_str = serde_json::to_string(config)
        .map_err(|e| format!("failed to serialize form config: {e:?}"))?;
    let args = [D1Type::Text(&json_str), D1Type::Text(event_id)];
    db.prepare("UPDATE events SET form_config = ? WHERE id = ?")
        .bind_refs(&args)
        .map_err(|e| format!("D1 save_form_config bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 save_form_config: {e:?}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Recap helpers (Plan 008 — Phase 2)
// ---------------------------------------------------------------------------

/// Flip the denormalized `recap_published` flag on the `events` row.
///
/// Canonical source is `event_summaries.recap_published_at`; this flag is
/// duplicated onto the events row so `GET /api/public/events/past` can filter
/// without a join. Mirrors the migration 0020 column added by Phase 1.
pub async fn set_recap_published_flag(
    db: &D1Database,
    event_id: &str,
    published: bool,
) -> Result<(), String> {
    let value = if published { 1 } else { 0 };
    let sql = format!("UPDATE events SET recap_published = {value} WHERE id = ?");
    db.prepare(&sql)
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 set_recap_published_flag bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 set_recap_published_flag: {e:?}"))?;
    Ok(())
}

/// Set the post-event registration toggle + optional deadline on the `events`
/// row (Plan 008 — Phase 3 §3.3.1).
///
/// `until_ms = None` clears the deadline (open indefinitely). The flag +
/// deadline are also mirrored onto the KV `EventConfig` by the toggle handler
/// so the KV-first read path stays consistent.
pub async fn set_post_event_registration(
    db: &D1Database,
    event_id: &str,
    open: bool,
    until_ms: Option<i64>,
) -> Result<(), String> {
    let open_val = if open { 1 } else { 0 };
    // NULL deadline when None; the migration column is nullable.
    let until_sql = match until_ms {
        Some(ms) => format!("{ms}"),
        None => "NULL".to_string(),
    };
    let sql = format!(
        "UPDATE events SET post_event_registration_open = {open_val}, \
         post_event_registration_until_ms = {until_sql} WHERE id = ?"
    );
    db.prepare(&sql)
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 set_post_event_registration bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 set_post_event_registration: {e:?}"))?;
    Ok(())
}
