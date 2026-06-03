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

    let changes = result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0);
    Ok(changes > 0)
}

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
        .map_err(|e| format!("D1 get_claim_lock first: {e:?}"))
}
