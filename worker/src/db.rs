//! D1 database helpers for claim lock operations.
//!
//! Provides atomic claim lock primitives using SQLite `INSERT ON CONFLICT DO NOTHING`
//! to eliminate the TOCTOU race window present in the KV write-first-verify-after pattern.

use serde::Deserialize;
use worker::D1Database;
use worker::d1::D1Type;

/// A claim lock row read from D1.
///
/// Used by D1-first claim reads (Phase 2 migration).
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct ClaimLockRow {
    pub lock_id: String,
    pub event_id: String,
    pub token: String,
    pub wallet: String,
    pub expires_at: Option<String>,
    pub asset_id: Option<String>,
    pub signature: Option<String>,
    pub claimed_at: Option<String>,
}

/// Try to atomically acquire a claim lock via D1.
///
/// Uses `INSERT ... ON CONFLICT DO NOTHING` so the database serialises
/// concurrent attempts. Returns `Ok(true)` if the lock was acquired,
/// `Ok(false)` if a conflicting row already exists.
pub(crate) async fn acquire_claim_lock(
    db: &D1Database,
    event_id: &str,
    token: &str,
    lock_id: &str,
    wallet: &str,
    expires_at: &str,
) -> Result<bool, String> {
    let stmt = db.prepare(
        "INSERT INTO claim_locks (lock_id, event_id, token, wallet, expires_at) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT (event_id, token) DO NOTHING",
    );
    let result = stmt
        .bind_refs(&[
            D1Type::Text(lock_id),
            D1Type::Text(event_id),
            D1Type::Text(token),
            D1Type::Text(wallet),
            D1Type::Text(expires_at),
        ])
        .map_err(|e| format!("D1 acquire_claim_lock bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 acquire_claim_lock run: {e:?}"))?;

    // `changes` is 1 if a row was inserted, 0 if ON CONFLICT triggered.
    let changes = result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0);
    Ok(changes > 0)
}

/// Finalize a claim lock after successful mint — sets asset_id, signature, claimed_at.
pub(crate) async fn finalize_claim_lock(
    db: &D1Database,
    event_id: &str,
    token: &str,
    asset_id: &str,
    signature: &str,
    claimed_at: &str,
) -> Result<(), String> {
    let stmt = db.prepare(
        "UPDATE claim_locks \
         SET asset_id = ?1, signature = ?2, claimed_at = ?3, expires_at = NULL \
         WHERE event_id = ?4 AND token = ?5",
    );
    stmt.bind_refs(&[
        D1Type::Text(asset_id),
        D1Type::Text(signature),
        D1Type::Text(claimed_at),
        D1Type::Text(event_id),
        D1Type::Text(token),
    ])
    .map_err(|e| format!("D1 finalize_claim_lock bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 finalize_claim_lock run: {e:?}"))?;

    Ok(())
}

/// Release (delete) a claim lock so the attendee can retry.
pub(crate) async fn release_claim_lock(
    db: &D1Database,
    event_id: &str,
    token: &str,
) -> Result<(), String> {
    let stmt = db.prepare("DELETE FROM claim_locks WHERE event_id = ?1 AND token = ?2");
    stmt.bind_refs(&[D1Type::Text(event_id), D1Type::Text(token)])
        .map_err(|e| format!("D1 release_claim_lock bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 release_claim_lock run: {e:?}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Audit log helpers
// ---------------------------------------------------------------------------

/// Row read from the `audit_log` table.
#[derive(Debug, Deserialize)]
pub(crate) struct AuditRow {
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub description: String,
    pub metadata: Option<String>,
}

/// Append an audit entry to D1. Fire-and-forget style — callers ignore errors.
pub(crate) async fn append_audit(
    db: &D1Database,
    event_id: &str,
    actor: &str,
    action: &str,
    target: &str,
    description: &str,
    metadata: Option<&str>,
) -> Result<(), String> {
    let stmt = db.prepare(
        "INSERT INTO audit_log (event_id, actor, action, target, description, metadata) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    );
    stmt.bind_refs(&[
        D1Type::Text(event_id),
        D1Type::Text(actor),
        D1Type::Text(action),
        D1Type::Text(target),
        D1Type::Text(description),
        match metadata {
            Some(v) => D1Type::Text(v),
            None => D1Type::Null,
        },
    ])
    .map_err(|e| format!("D1 append_audit bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 append_audit run: {e:?}"))?;

    Ok(())
}

/// Get audit entries for a specific event, newest first, up to `limit`.
pub(crate) async fn get_audit_entries(
    db: &D1Database,
    event_id: &str,
    limit: usize,
) -> Result<Vec<AuditRow>, String> {
    let stmt = db.prepare(
        "SELECT timestamp, actor, action, target, description, metadata \
         FROM audit_log WHERE event_id = ?1 \
         ORDER BY timestamp DESC LIMIT ?2",
    );
    stmt.bind_refs(&[D1Type::Text(event_id), D1Type::Integer(limit as i32)])
        .map_err(|e| format!("D1 get_audit_entries bind: {e:?}"))?
        .all()
        .await
        .map_err(|e| format!("D1 get_audit_entries run: {e:?}"))?
        .results::<AuditRow>()
        .map_err(|e| format!("D1 get_audit_entries deserialize: {e:?}"))
}

/// Get global audit entries (all events), newest first, up to `limit`.
pub(crate) async fn get_global_audit_entries(
    db: &D1Database,
    limit: usize,
) -> Result<Vec<AuditRow>, String> {
    let stmt = db.prepare(
        "SELECT timestamp, actor, action, target, description, metadata \
         FROM audit_log \
         ORDER BY timestamp DESC LIMIT ?1",
    );
    stmt.bind_refs(&[D1Type::Integer(limit as i32)])
        .map_err(|e| format!("D1 get_global_audit_entries bind: {e:?}"))?
        .all()
        .await
        .map_err(|e| format!("D1 get_global_audit_entries run: {e:?}"))?
        .results::<AuditRow>()
        .map_err(|e| format!("D1 get_global_audit_entries deserialize: {e:?}"))
}

/// Look up a claim lock by event_id and token.
/// Returns `Ok(None)` if no row found.
///
/// Used by D1-first claim reads (Phase 2 migration).
#[allow(dead_code)]
pub(crate) async fn get_claim_lock(
    db: &D1Database,
    event_id: &str,
    token: &str,
) -> Result<Option<ClaimLockRow>, String> {
    let stmt = db.prepare(
        "SELECT lock_id, event_id, token, wallet, expires_at, \
         asset_id, signature, claimed_at \
         FROM claim_locks WHERE event_id = ?1 AND token = ?2",
    );
    stmt.bind_refs(&[D1Type::Text(event_id), D1Type::Text(token)])
        .map_err(|e| format!("D1 get_claim_lock bind: {e:?}"))?
        .first::<ClaimLockRow>(None)
        .await
        .map_err(|e| format!("D1 get_claim_lock first: {e:?}"))
}
