//! D1 escrow index query helpers.
//!
//! P4: Escrow reverse index (escrow_address → event_id) stored in D1
//! so KV is optional.

use worker::D1Database;
use worker::d1::D1Type;

/// Look up the event ID for an escrow address from D1.
/// Returns `None` if not found.
pub async fn get_event_id_by_escrow_from_d1(
    db: &D1Database,
    escrow_address: &str,
) -> Result<Option<String>, String> {
    let stmt = db.prepare("SELECT event_id FROM escrow_index WHERE escrow_address = ?1");
    let result = stmt
        .bind_refs(&[D1Type::Text(escrow_address)])
        .map_err(|e| format!("D1 get_event_id_by_escrow bind: {e:?}"))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| format!("D1 get_event_id_by_escrow query: {e:?}"))?;

    match result {
        Some(row) => row
            .get("event_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "D1 escrow_index.event_id missing or not a string".to_string())
            .map(Some),
        None => Ok(None),
    }
}

/// Upsert an escrow address → event_id mapping into D1.
pub async fn upsert_escrow_index_to_d1(
    db: &D1Database,
    escrow_address: &str,
    event_id: &str,
) -> Result<(), String> {
    if escrow_address.is_empty() {
        return Ok(());
    }
    let stmt = db.prepare(
        "INSERT INTO escrow_index (escrow_address, event_id, updated_at) \
         VALUES (?1, ?2, datetime('now')) \
         ON CONFLICT (escrow_address) DO UPDATE SET event_id = excluded.event_id, updated_at = datetime('now')",
    );
    stmt.bind_refs(&[D1Type::Text(escrow_address), D1Type::Text(event_id)])
        .map_err(|e| format!("D1 upsert_escrow_index bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 upsert_escrow_index run: {e:?}"))?;

    Ok(())
}

/// Delete an escrow index entry from D1.
#[allow(dead_code)]
pub async fn delete_escrow_index_from_d1(
    db: &D1Database,
    escrow_address: &str,
) -> Result<(), String> {
    if escrow_address.is_empty() {
        return Ok(());
    }
    let stmt = db.prepare("DELETE FROM escrow_index WHERE escrow_address = ?1");
    stmt.bind_refs(&[D1Type::Text(escrow_address)])
        .map_err(|e| format!("D1 delete_escrow_index bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 delete_escrow_index run: {e:?}"))?;

    Ok(())
}
