//! D1 adventure query helpers.
//!
//! Adventure configs and progress are stored exclusively in D1 (Phase 3a complete).

use worker::D1Database;
use worker::d1::D1Type;

// ---------------------------------------------------------------------------
// Adventure config
// ---------------------------------------------------------------------------

/// Read adventure config JSON from D1. Returns `None` if not found.
pub async fn get_adventure_config_from_d1(
    db: &D1Database,
    event_id: &str,
) -> Result<Option<String>, String> {
    let stmt = db.prepare("SELECT config_json FROM adventure_configs WHERE event_id = ?1");
    let result = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 get_adventure_config bind: {e:?}"))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| format!("D1 get_adventure_config query: {e:?}"))?;

    match result {
        Some(row) => row
            .get("config_json")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "D1 adventure_configs.config_json missing or not a string".to_string())
            .map(Some),
        None => Ok(None),
    }
}

/// Upsert adventure config JSON into D1. Sets `enabled` column (0/1).
pub async fn upsert_adventure_config_to_d1(
    db: &D1Database,
    event_id: &str,
    config_json: &str,
    enabled: bool,
) -> Result<(), String> {
    let stmt = db.prepare(
        "INSERT INTO adventure_configs (event_id, config_json, enabled, updated_at) \
         VALUES (?1, ?2, ?3, datetime('now')) \
         ON CONFLICT (event_id) DO UPDATE SET config_json = excluded.config_json, enabled = excluded.enabled, updated_at = datetime('now')",
    );
    stmt.bind_refs(&[
        D1Type::Text(event_id),
        D1Type::Text(config_json),
        D1Type::Integer(if enabled { 1 } else { 0 }),
    ])
    .map_err(|e| format!("D1 upsert_adventure_config bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 upsert_adventure_config run: {e:?}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Adventure progress
// ---------------------------------------------------------------------------

/// Read adventure progress JSON from D1. Returns `None` if not found.
pub async fn get_adventure_progress_from_d1(
    db: &D1Database,
    event_id: &str,
    claim_token: &str,
) -> Result<Option<String>, String> {
    let stmt = db.prepare(
        "SELECT progress_json FROM adventure_progress WHERE event_id = ?1 AND claim_token = ?2",
    );
    let result = stmt
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(claim_token)])
        .map_err(|e| format!("D1 get_adventure_progress bind: {e:?}"))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| format!("D1 get_adventure_progress query: {e:?}"))?;

    match result {
        Some(row) => row
            .get("progress_json")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                "D1 adventure_progress.progress_json missing or not a string".to_string()
            })
            .map(Some),
        None => Ok(None),
    }
}

/// Upsert adventure progress JSON into D1. Also stores denormalized columns
/// for indexed lookups.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_adventure_progress_to_d1(
    db: &D1Database,
    event_id: &str,
    claim_token: &str,
    progress_json: &str,
    passed: bool,
    total_moves: u32,
    total_time_seconds: u32,
    levels_completed_count: u32,
    last_played_at: Option<&str>,
) -> Result<(), String> {
    let stmt = db.prepare(
        "INSERT INTO adventure_progress (event_id, claim_token, progress_json, passed, total_moves, total_time_seconds, levels_completed_count, last_played_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now')) \
         ON CONFLICT (event_id, claim_token) DO UPDATE SET \
         progress_json = excluded.progress_json, \
         passed = excluded.passed, \
         total_moves = excluded.total_moves, \
         total_time_seconds = excluded.total_time_seconds, \
         levels_completed_count = excluded.levels_completed_count, \
         last_played_at = excluded.last_played_at, \
         updated_at = datetime('now')",
    );
    stmt.bind_refs(&[
        D1Type::Text(event_id),
        D1Type::Text(claim_token),
        D1Type::Text(progress_json),
        D1Type::Integer(if passed { 1 } else { 0 }),
        D1Type::Integer(total_moves as i32),
        D1Type::Integer(total_time_seconds as i32),
        D1Type::Integer(levels_completed_count as i32),
        D1Type::Text(last_played_at.unwrap_or("")),
    ])
    .map_err(|e| format!("D1 upsert_adventure_progress bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 upsert_adventure_progress run: {e:?}"))?;

    Ok(())
}
