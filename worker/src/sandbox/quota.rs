//! Rolling 24 h caps on the two things that spend sandbox SOL and USDC.
//!
//! One statement on `advisory_locks` (migration 0027) both claims a per-item
//! key and counts the live keys under its prefix, so "this wallet already got
//! a grant" and "the faucet gave 50 today" are decided together, atomically,
//! with no new table. A key lives 24 h; the count of live keys is the
//! rolling-day total.

use worker::{D1Database, D1Type};

/// Key TTL: one rolling day.
const DAY_SECONDS: u32 = 24 * 3600;

/// What claiming a slot decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Granted,
    /// This item already holds a live slot (e.g. the wallet got its grant).
    AlreadyHeld,
    /// The rolling-day cap is reached.
    CapReached,
}

/// The claim: insert `prefix || item` unless the prefix already has `cap`
/// live keys; steal the key only if it expired. The SELECT keeps its WHERE,
/// which SQLite needs to tell `ON CONFLICT` from a join constraint.
pub const CLAIM_SQL: &str = "INSERT INTO advisory_locks (lock_key, expires_at) \
     SELECT ?1, datetime('now', ?2) \
     WHERE (SELECT COUNT(*) FROM advisory_locks \
            WHERE lock_key >= ?3 AND lock_key < ?4 AND expires_at > datetime('now')) < ?5 \
     ON CONFLICT(lock_key) DO UPDATE SET expires_at = excluded.expires_at \
     WHERE advisory_locks.expires_at < datetime('now')";

/// Whether `item` already holds a live key; tells the two refusals apart.
const HELD_SQL: &str =
    "SELECT 1 AS held FROM advisory_locks WHERE lock_key = ?1 AND expires_at > datetime('now')";

/// The exclusive upper bound of the key range under `prefix` (`prefix` ends
/// in `:`, so bumping that byte to `;` covers exactly the prefix).
pub fn prefix_end(prefix: &str) -> String {
    let mut end = prefix.trim_end_matches(':').to_string();
    end.push(';');
    end
}

/// Claim a slot for `item` under `prefix` (e.g. `sandbox:faucet:`).
pub async fn claim(db: &D1Database, prefix: &str, item: &str, cap: u32) -> Result<Slot, String> {
    let key = format!("{prefix}{item}");
    let ttl = format!("+{DAY_SECONDS} seconds");
    let end = prefix_end(prefix);
    let result = db
        .prepare(CLAIM_SQL)
        .bind_refs(&[
            D1Type::Text(&key),
            D1Type::Text(&ttl),
            D1Type::Text(prefix),
            D1Type::Text(&end),
            D1Type::Integer(cap as i32),
        ])
        .map_err(|e| format!("sandbox quota bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("sandbox quota run: {e:?}"))?;
    let changes = result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0);
    if changes > 0 {
        return Ok(Slot::Granted);
    }
    let held = db
        .prepare(HELD_SQL)
        .bind_refs(&[D1Type::Text(&key)])
        .map_err(|e| format!("sandbox quota held bind: {e:?}"))?
        .all()
        .await
        .map_err(|e| format!("sandbox quota held run: {e:?}"))?
        .results::<serde_json::Value>()
        .map_err(|e| format!("sandbox quota held rows: {e:?}"))?;
    Ok(match held.is_empty() {
        false => Slot::AlreadyHeld,
        true => Slot::CapReached,
    })
}

/// Give a slot back when the thing it paid for did not happen.
pub async fn release(db: &D1Database, prefix: &str, item: &str) {
    let key = format!("{prefix}{item}");
    if let Err(error) = crate::db::advisory_locks::release(db, &key).await {
        tracing::warn!(error = %error, "sandbox quota release failed");
    }
}
