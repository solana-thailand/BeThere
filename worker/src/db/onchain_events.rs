//! D1 on-chain event query helpers.
//!
//! On-chain events, dedup markers, and polling cursors stored in D1 (Phase 3b).

use js_sys::Array;
use wasm_bindgen::JsCast;
use worker::D1Database;
use worker::d1::D1Type;

use crate::escrow_indexer::{EscrowInstruction, OnChainEvent};

/// Fetch on-chain events for an event, newest first.
pub async fn get_onchain_events_from_d1(
    db: &D1Database,
    event_id: &str,
    limit: usize,
) -> Result<Vec<OnChainEvent>, String> {
    let stmt = db.prepare(
        "SELECT * FROM onchain_events WHERE event_id = ?1 ORDER BY block_time DESC LIMIT ?2",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id), D1Type::Integer(limit as i32)])
        .map_err(|e| format!("D1 get_onchain_events bind: {e:?}"))?;

    // Bypass D1Result::results() — it uses serde_wasm_bindgen::from_value().unwrap()
    // which panics on NULL columns. Use raw JS interop + JSON.stringify instead.
    let raw_result = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .all()
            .map_err(|e| format!("D1 raw all() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_onchain_events query: {e:?}"))?;

    let results_val = js_sys::Reflect::get(&raw_result, &js_sys::JsString::from("results"))
        .map_err(|e| format!("D1 get results property: {e:?}"))?;

    if results_val.is_null() || results_val.is_undefined() {
        return Ok(Vec::new());
    }

    let results_arr: Array = results_val
        .dyn_into()
        .map_err(|e| format!("D1 results is not an array: {e:?}"))?;

    let mut events = Vec::with_capacity(results_arr.length() as usize);
    for (i, js_row) in results_arr.iter().enumerate() {
        let json_str = match js_sys::JSON::stringify(&js_row) {
            Ok(s) => s.as_string().unwrap_or_default(),
            Err(e) => {
                tracing::warn!(row_index = i, error = ?e, "D1: JSON.stringify failed, skipping row");
                continue;
            }
        };

        let row: serde_json::Value = match serde_json::from_str(&json_str) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    row_index = i,
                    error = %e,
                    "D1: skipping row with deserialize error"
                );
                continue;
            }
        };

        let instruction_str = row
            .get("instruction")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let instruction = serde_json::from_str::<EscrowInstruction>(instruction_str)
            .unwrap_or(EscrowInstruction::Unknown);

        let event = OnChainEvent {
            signature: row
                .get("signature")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            slot: row.get("slot").and_then(|v| v.as_i64()).unwrap_or(0) as u64,
            block_time: row.get("block_time").and_then(|v| v.as_i64()).unwrap_or(0),
            instruction,
            escrow_address: row
                .get("escrow_address")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            target_escrow_address: row
                .get("target_escrow_address")
                .and_then(|v| v.as_str())
                .map(String::from),
            organizer: row
                .get("organizer")
                .and_then(|v| v.as_str())
                .map(String::from),
            attendee: row
                .get("attendee")
                .and_then(|v| v.as_str())
                .map(String::from),
            amount: row.get("amount").and_then(|v| v.as_i64()).map(|v| v as u64),
            indexed_at: row
                .get("indexed_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        };
        events.push(event);
    }

    Ok(events)
}

/// Insert an on-chain event into D1. Returns `Ok(false)` if signature already exists (duplicate).
pub async fn insert_onchain_event_to_d1(
    db: &D1Database,
    event_id: &str,
    event: &OnChainEvent,
) -> Result<bool, String> {
    let instruction_json = serde_json::to_string(&event.instruction).unwrap_or_default();

    let stmt = db.prepare(
        "INSERT INTO onchain_events \
         (event_id, signature, slot, block_time, instruction, escrow_address, \
          target_escrow_address, organizer, attendee, amount, indexed_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    );
    let bind_result = stmt.bind_refs(&[
        D1Type::Text(event_id),
        D1Type::Text(&event.signature),
        D1Type::Integer(event.slot as i32),
        D1Type::Integer(event.block_time as i32),
        D1Type::Text(&instruction_json),
        D1Type::Text(&event.escrow_address),
        D1Type::Text(event.target_escrow_address.as_deref().unwrap_or("")),
        D1Type::Text(event.organizer.as_deref().unwrap_or("")),
        D1Type::Text(event.attendee.as_deref().unwrap_or("")),
        D1Type::Integer(event.amount.map(|a| a as i32).unwrap_or(0)),
        D1Type::Text(event.indexed_at.as_str()),
    ]);

    match bind_result {
        Ok(bound) => match bound.run().await {
            Ok(_) => {}
            Err(e) => {
                let msg = format!("{e:?}");
                if msg.contains("UNIQUE constraint failed") {
                    return Ok(false);
                }
                return Err(format!("D1 insert_onchain_event run: {e:?}"));
            }
        },
        Err(e) => return Err(format!("D1 insert_onchain_event bind: {e:?}")),
    }

    // Also insert dedup marker.
    let dedup_stmt = db.prepare("INSERT OR IGNORE INTO onchain_dedup (signature) VALUES (?1)");
    dedup_stmt
        .bind_refs(&[D1Type::Text(&event.signature)])
        .map_err(|e| format!("D1 insert_onchain_dedup bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 insert_onchain_dedup run: {e:?}"))?;

    Ok(true)
}

/// Upsert the polling cursor for an escrow address.
pub async fn save_cursor_to_d1(
    db: &D1Database,
    escrow_address: &str,
    signature: &str,
) -> Result<(), String> {
    let stmt = db.prepare(
        "INSERT INTO onchain_cursors (escrow_address, last_signature, updated_at) \
         VALUES (?1, ?2, datetime('now')) \
         ON CONFLICT (escrow_address) DO UPDATE SET last_signature = excluded.last_signature, updated_at = datetime('now')",
    );
    stmt.bind_refs(&[D1Type::Text(escrow_address), D1Type::Text(signature)])
        .map_err(|e| format!("D1 save_cursor bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 save_cursor run: {e:?}"))?;

    Ok(())
}

/// Read the last-processed signature for an escrow address. Returns `None` if no cursor exists.
pub async fn read_cursor_from_d1(
    db: &D1Database,
    escrow_address: &str,
) -> Result<Option<String>, String> {
    let stmt = db.prepare("SELECT last_signature FROM onchain_cursors WHERE escrow_address = ?1");
    let result = stmt
        .bind_refs(&[D1Type::Text(escrow_address)])
        .map_err(|e| format!("D1 read_cursor bind: {e:?}"))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| format!("D1 read_cursor query: {e:?}"))?;

    match result {
        Some(row) => row
            .get("last_signature")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "D1 onchain_cursors.last_signature missing or not a string".to_string())
            .map(Some),
        None => Ok(None),
    }
}

/// Delete dedup entries older than `days` days. Returns `Ok(0)` (D1 exec doesn't return row count).
pub async fn cleanup_old_dedup_entries(db: &D1Database, days: i64) -> Result<usize, String> {
    let sql =
        format!("DELETE FROM onchain_dedup WHERE indexed_at < datetime('now', '-{days} days')");
    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 cleanup_old_dedup_entries: {e:?}"))?;
    Ok(0) // D1 exec doesn't return rows affected
}
