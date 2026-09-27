use serde::Deserialize;
use worker::D1Database;
use worker::d1::D1Type;

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

/// What an attempt to insert a claim lock found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClaimLockInsert {
    Acquired,
    /// This token already holds a lock: in flight, or claimed.
    TokenHeld,
    /// Another token in this event already locked this recipient wallet
    /// (0055, one badge per wallet per event).
    WalletUsed,
}

#[derive(Deserialize)]
struct LockHolder {
    same_token: i64,
}

pub(crate) async fn acquire_claim_lock(
    db: &D1Database,
    event_id: &str,
    token: &str,
    lock_id: &str,
    wallet: &str,
    expires_at: &str,
) -> Result<ClaimLockInsert, String> {
    // Targetless DO NOTHING: a clash on the (event_id, token) key or on the
    // 0055 (event_id, wallet) index both insert nothing instead of raising.
    // The lookup below says which one held.
    let stmt = db.prepare(
        "INSERT INTO claim_locks (lock_id, event_id, token, wallet, expires_at) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT DO NOTHING",
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

    let changes = result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0);
    if changes > 0 {
        return Ok(ClaimLockInsert::Acquired);
    }

    // The token's own row wins over a wallet match, so a retry of the same
    // claim still reads as "already being processed".
    let holder = db
        .prepare(
            "SELECT (token = ?2) AS same_token FROM claim_locks \
             WHERE event_id = ?1 AND (token = ?2 OR wallet = ?3) \
             ORDER BY same_token DESC LIMIT 1",
        )
        .bind_refs(&[
            D1Type::Text(event_id),
            D1Type::Text(token),
            D1Type::Text(wallet),
        ])
        .map_err(|e| format!("D1 acquire_claim_lock holder bind: {e:?}"))?
        .first::<LockHolder>(None)
        .await
        .map_err(|e| format!("D1 acquire_claim_lock holder query: {e:?}"))?;
    Ok(classify_holder(holder.map(|h| h.same_token != 0)))
}

/// `None` means the conflicting row was released between the insert and the
/// lookup; report it as busy so the client retries.
fn classify_holder(same_token: Option<bool>) -> ClaimLockInsert {
    match same_token {
        Some(false) => ClaimLockInsert::WalletUsed,
        Some(true) | None => ClaimLockInsert::TokenHeld,
    }
}

pub(crate) async fn finalize_claim_lock(
    db: &D1Database,
    event_id: &str,
    token: &str,
    asset_id: &str,
    signature: &str,
    claimed_at: &str,
) -> Result<(), String> {
    // `expires_at` is NOT NULL, so finalization moves it out to the 90-day
    // retention horizon rather than nulling it; see `claim::finalized_expires_at`.
    let expires_at = crate::claim::finalized_expires_at();
    let stmt = db.prepare(
        "UPDATE claim_locks \
         SET asset_id = ?1, signature = ?2, claimed_at = ?3, expires_at = ?4 \
         WHERE event_id = ?5 AND token = ?6",
    );
    stmt.bind_refs(&[
        D1Type::Text(asset_id),
        D1Type::Text(signature),
        D1Type::Text(claimed_at),
        D1Type::Text(&expires_at),
        D1Type::Text(event_id),
        D1Type::Text(token),
    ])
    .map_err(|e| format!("D1 finalize_claim_lock bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 finalize_claim_lock run: {e:?}"))?;

    Ok(())
}

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
        .map_err(|e| format!("D1 get_claim_lock query: {e:?}"))
}
