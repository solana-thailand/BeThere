//! Generic short-lived advisory locks (D1-backed) for serializing critical
//! sections that touch a non-transactional external store (Google Sheets).
//!
//! Acquisition is a single atomic statement: insert the key, or steal it if the
//! existing lock has expired. `meta().changes > 0` ⇒ acquired. Callers should
//! `release` on the happy path; the TTL is the backstop for crashes/errors.
//!
//! See migration `0027_advisory_locks.sql` and
//! `docs/SECURITY-FINDINGS-2026-08-13.md` #4.
//!
//! NOTE: the credit-spend caller migrated to the atomic `credit_ledger` (a
//! single-statement conditional insert), so these are currently unused — kept as
//! a general-purpose primitive for other Sheets-touching critical sections.
#![allow(dead_code)]

use worker::{D1Database, D1Type};

/// Try to acquire `key` for `ttl_secs`. Returns `Ok(true)` if acquired (new key
/// or an expired lock stolen), `Ok(false)` if another holder still owns it.
pub async fn try_acquire(db: &D1Database, key: &str, ttl_secs: u32) -> Result<bool, String> {
    // Bound modifier for SQLite datetime(): e.g. "+30 seconds".
    let ttl_modifier = format!("+{ttl_secs} seconds");
    let sql = "INSERT INTO advisory_locks (lock_key, expires_at) \
               VALUES (?1, datetime('now', ?2)) \
               ON CONFLICT(lock_key) DO UPDATE SET expires_at = datetime('now', ?2) \
               WHERE advisory_locks.expires_at < datetime('now')";
    let result = db
        .prepare(sql)
        .bind_refs(&[D1Type::Text(key), D1Type::Text(&ttl_modifier)])
        .map_err(|e| format!("D1 advisory_locks acquire bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 advisory_locks acquire run: {e:?}"))?;
    let changes = result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0);
    Ok(changes > 0)
}

/// Release `key` (best-effort). Safe to call even if the lock already expired.
pub async fn release(db: &D1Database, key: &str) -> Result<(), String> {
    db.prepare("DELETE FROM advisory_locks WHERE lock_key = ?1")
        .bind_refs(&[D1Type::Text(key)])
        .map_err(|e| format!("D1 advisory_locks release bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 advisory_locks release run: {e:?}"))?;
    Ok(())
}
