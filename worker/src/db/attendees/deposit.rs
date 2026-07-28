//! Deposit-status D1 helpers.

use serde::Deserialize;
use worker::D1Database;
use worker::d1::D1Type;

/// Get deposit status from D1 by attendee ID.
/// Returns the raw deposit columns if a row is found.
///
/// Uses JSON.stringify + serde_json to bypass `.first::<T>()` which uses
/// `serde_wasm_bindgen::from_value()` — that crashes on `JsValue(null)`
/// columns (e.g. rows inserted before ALTER TABLE added NOT NULL DEFAULT).
#[allow(dead_code)]
pub(crate) async fn get_deposit_status_from_d1(
    db: &D1Database,
    attendee_id: &str,
) -> Result<Option<DepositStatusRow>, String> {
    let stmt = db.prepare(
        "SELECT id, event_id, deposit_status, deposit_tx_hash, deposit_amount_usdc \
         FROM attendees WHERE id = ?1",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(attendee_id)])
        .map_err(|e| format!("D1 get_deposit_status bind: {e:?}"))?;

    // Bypass worker crate's .first::<T>() which uses serde_wasm_bindgen::from_value()
    // — that crashes on JsValue(null) columns. Instead, call the raw JS .first() via
    // inner(), then JSON.stringify → serde_json (same pattern as get_attendees_by_event).
    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 get_deposit_status first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_deposit_status first() await: {e:?}"))?;

    // No row found
    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    if json_str.is_empty() {
        return Ok(None);
    }

    let row: DepositStatusRow = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(
            error = %e,
            json = %json_str.chars().take(300).collect::<String>(),
            "D1 get_deposit_status: deserialize failed"
        );
        format!("D1 get_deposit_status deserialize: {e}")
    })?;

    Ok(Some(row))
}

/// Raw D1 row for deposit status columns.
///
/// Uses serde_json (via JSON.stringify) so NULL columns become `None`
/// in Option fields — no serde_wasm_bindgen crash.
#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct DepositStatusRow {
    pub id: Option<String>,
    pub event_id: Option<String>,
    pub deposit_status: Option<String>,
    pub deposit_tx_hash: Option<String>,
    pub deposit_amount_usdc: Option<i64>,
}

/// Save a pending deposit to D1 (upsert deposit columns on the attendee row).
#[allow(dead_code)]
pub(crate) async fn save_deposit_status_to_d1(
    db: &D1Database,
    attendee_id: &str,
    deposit_status: &str,
    deposit_tx_hash: Option<&str>,
    deposit_amount_usdc: i64,
) -> Result<(), String> {
    let tx_hash = deposit_tx_hash.unwrap_or("");
    let stmt = db.prepare(
        "UPDATE attendees \
         SET deposit_status = ?1, deposit_tx_hash = ?2, deposit_amount_usdc = ?3, \
         updated_at = datetime('now') \
         WHERE id = ?4",
    );
    stmt.bind_refs(&[
        D1Type::Text(deposit_status),
        D1Type::Text(tx_hash),
        D1Type::Integer(deposit_amount_usdc as i32),
        D1Type::Text(attendee_id),
    ])
    .map_err(|e| format!("D1 save_deposit_status bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 save_deposit_status run: {e:?}"))?;

    Ok(())
}

/// Count deposits for an event from D1 (fallback for KV deposit counter).
#[allow(dead_code)]
pub(crate) async fn count_deposits_by_event(
    db: &D1Database,
    event_id: &str,
) -> Result<u32, String> {
    let stmt = db.prepare(
        "SELECT COUNT(*) as cnt FROM attendees \
         WHERE event_id = ?1 AND deposit_status IS NOT NULL AND deposit_status != ''",
    );
    let cnt = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 count_deposits bind: {e:?}"))?
        .first::<i64>(Some("cnt"))
        .await
        .map_err(|e| format!("D1 count_deposits first: {e:?}"))?;

    Ok(cnt.map(|c| c as u32).unwrap_or(0))
}
