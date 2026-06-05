//! D1 quiz query helpers.
//!
//! P4: Quiz configs and progress are stored in D1 so KV is optional.
//! Handlers try D1 first, fall back to KV on miss.

use worker::D1Database;
use worker::d1::D1Type;

// ---------------------------------------------------------------------------
// Quiz config
// ---------------------------------------------------------------------------

/// Read quiz config JSON from D1. Returns `None` if not found.
pub async fn get_quiz_config_from_d1(
    db: &D1Database,
    event_id: &str,
) -> Result<Option<String>, String> {
    let stmt = db.prepare("SELECT config_json FROM quiz_configs WHERE event_id = ?1");
    let result = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 get_quiz_config bind: {e:?}"))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| format!("D1 get_quiz_config query: {e:?}"))?;

    match result {
        Some(row) => row
            .get("config_json")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "D1 quiz_configs.config_json missing or not a string".to_string())
            .map(Some),
        None => Ok(None),
    }
}

/// Upsert quiz config JSON into D1.
pub async fn upsert_quiz_config_to_d1(
    db: &D1Database,
    event_id: &str,
    config_json: &str,
) -> Result<(), String> {
    let stmt = db.prepare(
        "INSERT INTO quiz_configs (event_id, config_json, updated_at) \
         VALUES (?1, ?2, datetime('now')) \
         ON CONFLICT (event_id) DO UPDATE SET config_json = excluded.config_json, updated_at = datetime('now')",
    );
    stmt.bind_refs(&[D1Type::Text(event_id), D1Type::Text(config_json)])
        .map_err(|e| format!("D1 upsert_quiz_config bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 upsert_quiz_config run: {e:?}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Quiz progress
// ---------------------------------------------------------------------------

/// Read quiz progress JSON from D1. Returns `None` if not found.
pub async fn get_quiz_progress_from_d1(
    db: &D1Database,
    event_id: &str,
    claim_token: &str,
) -> Result<Option<String>, String> {
    let stmt = db.prepare(
        "SELECT progress_json FROM quiz_progress WHERE event_id = ?1 AND claim_token = ?2",
    );
    let result = stmt
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(claim_token)])
        .map_err(|e| format!("D1 get_quiz_progress bind: {e:?}"))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| format!("D1 get_quiz_progress query: {e:?}"))?;

    match result {
        Some(row) => row
            .get("progress_json")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "D1 quiz_progress.progress_json missing or not a string".to_string())
            .map(Some),
        None => Ok(None),
    }
}

/// Upsert quiz progress JSON into D1. Also stores denormalized `passed` and
/// `attempts` columns for indexed lookups.
pub async fn upsert_quiz_progress_to_d1(
    db: &D1Database,
    event_id: &str,
    claim_token: &str,
    progress_json: &str,
    passed: bool,
    attempts: u8,
) -> Result<(), String> {
    let stmt = db.prepare(
        "INSERT INTO quiz_progress (event_id, claim_token, progress_json, passed, attempts, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now')) \
         ON CONFLICT (event_id, claim_token) DO UPDATE SET \
         progress_json = excluded.progress_json, \
         passed = excluded.passed, \
         attempts = excluded.attempts, \
         updated_at = datetime('now')",
    );
    stmt.bind_refs(&[
        D1Type::Text(event_id),
        D1Type::Text(claim_token),
        D1Type::Text(progress_json),
        D1Type::Integer(if passed { 1 } else { 0 }),
        D1Type::Integer(attempts as i32),
    ])
    .map_err(|e| format!("D1 upsert_quiz_progress bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 upsert_quiz_progress run: {e:?}"))?;

    Ok(())
}
