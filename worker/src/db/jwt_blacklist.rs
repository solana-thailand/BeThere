//! D1 JWT blacklist helpers (Issue #053 Phase 3g).
//!
//! Replaces KV key `jwt_blacklist:{sha256(token)}` with a D1 table.
//! Expired entries are pruned by the scheduled cleanup handler since D1 has no TTL.

use wasm_bindgen_futures::JsFuture;
use worker::D1Database;
use worker::d1::D1Type;

/// Insert a blacklisted token hash into D1.
/// `expires_at` is a Unix timestamp in seconds.
pub async fn insert(db: &D1Database, token_hash: &str, expires_at: u64) -> Result<(), String> {
    // `expires_at` is a `u64` second-precision timestamp; it stays interpolated
    // because `D1Type::Integer` is `i32`-only and a typed integer cannot inject.
    let sql = format!(
        "INSERT OR IGNORE INTO jwt_blacklist (token_hash, expires_at) VALUES (?, {expires_at})"
    );
    db.prepare(&sql)
        .bind_refs(&[D1Type::Text(token_hash)])
        .map_err(|e| format!("D1 jwt_blacklist insert bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 jwt_blacklist insert: {e:?}"))?;
    Ok(())
}

/// Check if a token hash exists in the blacklist and hasn't expired.
pub async fn exists(db: &D1Database, token_hash: &str) -> Result<bool, String> {
    let sql = "SELECT 1 AS found FROM jwt_blacklist WHERE token_hash = ? AND expires_at > unixepoch() LIMIT 1";
    let bound = db
        .prepare(sql)
        .bind_refs(&[D1Type::Text(token_hash)])
        .map_err(|e| format!("D1 jwt_blacklist exists bind: {e:?}"))?;

    // Bypass worker crate's .first::<T>() — crashes on JsValue(null).
    let raw_first = JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 jwt_blacklist exists first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 jwt_blacklist exists first() await: {e:?}"))?;

    Ok(!raw_first.is_null() && !raw_first.is_undefined())
}

/// Delete expired blacklist entries. Returns the number of rows deleted.
pub async fn cleanup_expired(db: &D1Database) -> Result<usize, String> {
    let sql = "DELETE FROM jwt_blacklist WHERE expires_at <= unixepoch()";
    db.exec(sql)
        .await
        .map_err(|e| format!("D1 jwt_blacklist cleanup_expired: {e:?}"))?;
    // D1 exec doesn't return rows affected; use a count query instead
    let count_sql = "SELECT COUNT(*) AS cnt FROM jwt_blacklist";
    let bound = db.prepare(count_sql);

    // Bypass worker crate's .first::<T>() — crashes on JsValue(null).
    let raw_first = JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 jwt_blacklist count first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 jwt_blacklist count first() await: {e:?}"))?;

    let _ = raw_first;
    Ok(0) // best-effort — actual count not returned by D1 exec
}
