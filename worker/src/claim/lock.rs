//! Claim lock acquisition, release, and finalization.
//!
//! Provides a write-first/verify-after lock mechanism backed by KV (with D1
//! atomic fallback) to prevent concurrent double-claims.
//!
//! Phase 1 (Issue #050): Routes through Durable Object when available for
//! truly ACID claim lock operations. Falls back to D1 + KV when DO is not
//! configured.

use worker::{KvStore, Method, ObjectNamespace, Request, RequestInit, Response};

use event_checkin_domain::models::attendee::WalkinAttendee;

use crate::db;
use crate::durable_objects::DoRequest;

// ---------------------------------------------------------------------------
// TTL constants
// ---------------------------------------------------------------------------

/// TTL for finalized claim lock records (90 days).
/// Auto-cleanup post-event; permanent records are not needed indefinitely.
pub(crate) const CLAIM_LOCK_FINALIZE_TTL_SECS: u64 = 86400 * 90;

// ---------------------------------------------------------------------------
// DO routing helpers (Issue #050 Phase 1)
// ---------------------------------------------------------------------------

/// Send an RPC request to the EventDurableObject for a given event.
/// Returns the parsed DoResponse.
async fn do_rpc(
    namespace: &ObjectNamespace,
    event_id: &str,
    request: DoRequest,
) -> Result<DoResponseParsed, String> {
    let id = namespace
        .id_from_name(event_id)
        .map_err(|e| format!("DO id_from_name failed: {e:?}"))?;
    let stub = id
        .get_stub()
        .map_err(|e| format!("DO get_stub failed: {e:?}"))?;

    let body = serde_json::to_string(&request)
        .map_err(|e| format!("DO request serialize failed: {e:?}"))?;

    let req = Request::new_with_init(
        "http://internal/do",
        &RequestInit {
            method: Method::Post,
            body: Some(body.into()),
            ..Default::default()
        },
    )
    .map_err(|e| format!("DO request init failed: {e:?}"))?;

    let mut resp: Response = stub
        .fetch_with_request(req)
        .await
        .map_err(|e| format!("DO fetch failed: {e:?}"))?;

    let parsed: DoResponseParsed = resp
        .json()
        .await
        .map_err(|e| format!("DO response parse failed: {e:?}"))?;

    Ok(parsed)
}

#[derive(serde::Deserialize)]
struct DoResponseParsed {
    success: bool,
    error: Option<String>,
}

// ---------------------------------------------------------------------------
// Lock helpers (pub(crate) for reuse by handlers if needed)
// ---------------------------------------------------------------------------

/// KV key for claim dedup lock
pub(crate) fn claim_lock_key(event_id: &str, token: &str) -> String {
    format!("event:{event_id}:claim_lock:{token}")
}

/// Try to acquire a claim lock. Returns Ok(()) if acquired, Err if already locked.
/// Sets a 5-minute TTL as safety net.
///
/// Phase 1 (Issue #050): Routes through Durable Object when available for
/// truly ACID lock acquisition. Falls back to D1 + KV when DO is not configured.
pub(crate) async fn acquire_claim_lock(
    kv: &KvStore,
    event_id: &str,
    token: &str,
    wallet: &str,
    d1: Option<&worker::D1Database>,
    event_do: Option<&ObjectNamespace>,
) -> Result<(), String> {
    // DO path: truly ACID (single-threaded per event)
    if let Some(namespace) = event_do {
        let lock_id = uuid::Uuid::now_v7().to_string();
        let expires_at = (chrono::Utc::now() + chrono::Duration::seconds(300)).to_rfc3339();

        let resp = do_rpc(
            namespace,
            event_id,
            DoRequest::AcquireClaimLock {
                lock_id: lock_id.clone(),
                event_id: event_id.to_string(),
                token: token.to_string(),
                wallet: wallet.to_string(),
                expires_at: expires_at.clone(),
            },
        )
        .await?;

        if !resp.success {
            tracing::warn!(
                claim_token = %token,
                error = ?resp.error,
                "claim lock race: already locked (DO)"
            );
            return Err(resp.error.unwrap_or_else(|| {
                "claim is already being processed or has been completed".to_string()
            }));
        }

        // DO lock acquired — also write KV for read compatibility
        let key = claim_lock_key(event_id, token);
        let kv_lock = serde_json::json!({
            "lock_id": lock_id,
            "wallet": wallet,
            "started_at": chrono::Utc::now().to_rfc3339(),
        })
        .to_string();
        if let Ok(builder) = kv.put(&key, &kv_lock) {
            let _ = builder.expiration_ttl(300).execute().await;
        }

        tracing::info!(
            claim_token = %token,
            lock_id = %lock_id,
            "claim lock acquired (DO+KV)"
        );
        return Ok(());
    }

    // D1 path: atomic INSERT ON CONFLICT DO NOTHING
    if let Some(db) = d1 {
        let lock_id = uuid::Uuid::now_v7().to_string();
        let expires_at = (chrono::Utc::now() + chrono::Duration::seconds(300)).to_rfc3339();
        let acquired =
            db::acquire_claim_lock(db, event_id, token, &lock_id, wallet, &expires_at).await?;

        if !acquired {
            tracing::warn!(
                claim_token = %token,
                "claim lock race: already locked (D1)"
            );
            return Err("claim is already being processed or has been completed".to_string());
        }

        // D1 lock acquired — also write KV for read compatibility
        let key = claim_lock_key(event_id, token);
        let kv_lock = serde_json::json!({
            "lock_id": lock_id,
            "wallet": wallet,
            "started_at": chrono::Utc::now().to_rfc3339(),
        })
        .to_string();
        if let Ok(builder) = kv.put(&key, &kv_lock) {
            let _ = builder.expiration_ttl(300).execute().await;
        }

        tracing::info!(
            claim_token = %token,
            lock_id = %lock_id,
            "claim lock acquired (D1+KV)"
        );
        return Ok(());
    }

    // KV fallback: write-first, verify-after
    let key = claim_lock_key(event_id, token);

    // Generate a unique lock ID so we can verify we won the race.
    let lock_id = uuid::Uuid::now_v7().to_string();
    let lock_value = serde_json::json!({
        "lock_id": lock_id,
        "wallet": wallet,
        "started_at": chrono::Utc::now().to_rfc3339(),
    })
    .to_string();

    // Step 1: Write our lock unconditionally
    kv.put(&key, &lock_value)
        .map_err(|e| format!("claim lock put failed: {e:?}"))?
        .expiration_ttl(300) // 5 minutes TTL
        .execute()
        .await
        .map_err(|e| format!("claim lock write failed: {e:?}"))?;

    // Step 2: Read back and verify we see our own lock_id
    let read_back: Option<String> = kv
        .get(&key)
        .text()
        .await
        .map_err(|e| format!("claim lock verify read failed: {e:?}"))?;

    match read_back {
        Some(stored) => {
            let stored_id = serde_json::from_str::<serde_json::Value>(&stored)
                .ok()
                .and_then(|v| v.get("lock_id").and_then(|v| v.as_str()).map(String::from));

            if stored_id.as_deref() == Some(&lock_id) {
                tracing::info!(
                    claim_token = %token,
                    lock_id = %lock_id,
                    "claim lock acquired (write-first verify)"
                );
                Ok(())
            } else {
                // Another request won — the winner's lock is authoritative.
                tracing::warn!(
                    claim_token = %token,
                    our_lock_id = %lock_id,
                    ?stored_id,
                    "claim lock race: another request won"
                );
                Err("claim is already being processed or has been completed".to_string())
            }
        }
        None => {
            // Extremely unlikely: write succeeded but read returned nothing
            // (could happen with eventual consistency across regions).
            // Proceed optimistically — the lock was written.
            tracing::warn!(
                claim_token = %token,
                "claim lock write succeeded but verify read returned None"
            );
            Ok(())
        }
    }
}

/// Parameters for finalizing a claim lock after successful mint.
pub(crate) struct FinalizeClaimLockParams<'a> {
    pub(crate) event_id: &'a str,
    pub(crate) token: &'a str,
    pub(crate) wallet: &'a str,
    pub(crate) asset_id: &'a str,
    pub(crate) signature: &'a str,
}

/// Finalize the claim lock after successful mint (removes TTL, sets final data).
/// Phase 1 (Issue #050): Routes through DO when available.
pub(crate) async fn finalize_claim_lock(
    kv: &KvStore,
    params: FinalizeClaimLockParams<'_>,
    d1: Option<&worker::D1Database>,
    event_do: Option<&ObjectNamespace>,
) -> Result<(), String> {
    let FinalizeClaimLockParams {
        event_id,
        token,
        wallet,
        asset_id,
        signature,
    } = params;
    // DO path
    if let Some(namespace) = event_do {
        let claimed_at = chrono::Utc::now().to_rfc3339();
        let resp = do_rpc(
            namespace,
            event_id,
            DoRequest::FinalizeClaimLock {
                event_id: event_id.to_string(),
                token: token.to_string(),
                asset_id: asset_id.to_string(),
                signature: signature.to_string(),
                claimed_at: claimed_at.clone(),
            },
        )
        .await?;

        if !resp.success {
            return Err(resp
                .error
                .unwrap_or_else(|| "claim lock finalization failed (DO)".to_string()));
        }

        // Also finalize in KV for read compatibility
        let key = claim_lock_key(event_id, token);
        let lock_value = serde_json::json!({
            "wallet": wallet,
            "asset_id": asset_id,
            "signature": signature,
            "claimed_at": claimed_at,
        })
        .to_string();
        kv.put(&key, &lock_value)
            .map_err(|e| format!("claim lock finalize failed: {e:?}"))?
            .expiration_ttl(CLAIM_LOCK_FINALIZE_TTL_SECS)
            .execute()
            .await
            .map_err(|e| format!("claim lock finalize write failed: {e:?}"))?;

        tracing::info!(claim_token = %token, "claim lock finalized (DO+KV)");
        return Ok(());
    }

    // D1 path: UPDATE claim_locks
    if let Some(db) = d1 {
        let claimed_at = chrono::Utc::now().to_rfc3339();
        db::finalize_claim_lock(db, event_id, token, asset_id, signature, &claimed_at).await?;
    }

    // Always finalize in KV (dual-write for read compatibility)
    let key = claim_lock_key(event_id, token);

    let lock_value = serde_json::json!({
        "wallet": wallet,
        "asset_id": asset_id,
        "signature": signature,
        "claimed_at": chrono::Utc::now().to_rfc3339(),
    })
    .to_string();

    // Overwrite with 90-day TTL — auto-cleanup post-event
    kv.put(&key, &lock_value)
        .map_err(|e| format!("claim lock finalize failed: {e:?}"))?
        .expiration_ttl(CLAIM_LOCK_FINALIZE_TTL_SECS)
        .execute()
        .await
        .map_err(|e| format!("claim lock finalize write failed: {e:?}"))?;

    Ok(())
}

/// Release the claim lock on failure (delete the key so attendee can retry).
/// Phase 1 (Issue #050): Routes through DO when available.
pub(crate) async fn release_claim_lock(
    kv: &KvStore,
    event_id: &str,
    token: &str,
    d1: Option<&worker::D1Database>,
    event_do: Option<&ObjectNamespace>,
) -> Result<(), String> {
    // DO path
    if let Some(namespace) = event_do {
        let resp = do_rpc(
            namespace,
            event_id,
            DoRequest::ReleaseClaimLock {
                event_id: event_id.to_string(),
                token: token.to_string(),
            },
        )
        .await?;

        if !resp.success {
            tracing::warn!(
                claim_token = %token,
                error = ?resp.error,
                "DO release claim lock failed"
            );
        }

        // Always delete from KV
        let key = claim_lock_key(event_id, token);
        kv.delete(&key)
            .await
            .map_err(|e| format!("claim lock release failed: {e:?}"))?;
        tracing::info!(claim_token = %token, "claim lock released (DO+KV)");
        return Ok(());
    }

    // D1 path: DELETE
    if let Some(db) = d1 {
        db::release_claim_lock(db, event_id, token).await?;
        tracing::info!(claim_token = %token, "claim lock released (D1+KV)");
    }

    // Always delete from KV
    let key = claim_lock_key(event_id, token);
    kv.delete(&key)
        .await
        .map_err(|e| format!("claim lock release failed: {e:?}"))?;
    tracing::info!(claim_token = %token, "claim lock released");
    Ok(())
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

/// Mask a wallet address for safe display in error messages.
/// Shows first 4 and last 4 characters: "BxRW...3KjF".
/// Returns "****" if the address is too short to mask.
pub(crate) fn mask_wallet(addr: &str) -> String {
    if addr.len() > 8 {
        format!("{}...{}", &addr[..4], &addr[addr.len() - 4..])
    } else {
        "****".to_string()
    }
}

// ---------------------------------------------------------------------------
// Walk-in attendee lookup
// ---------------------------------------------------------------------------

/// KV key for walk-in reverse mapping (claim token → walk-in record).
const CLAIM_WALKIN_PREFIX: &str = "claim_walkin:";

/// KV key prefix for walk-in attendee records.
const WALKIN_PREFIX: &str = "walkin:";

/// Try to look up a walk-in attendee by claim token.
/// Returns None if not a walk-in (falls back to normal sheet lookup).
pub(crate) async fn lookup_walkin_by_claim_token(
    kv: &KvStore,
    token: &str,
) -> Option<WalkinAttendee> {
    let reverse_key = format!("{CLAIM_WALKIN_PREFIX}{token}");
    let mapping: Option<String> = kv.get(&reverse_key).text().await.ok().flatten();

    let mapping = mapping?;
    // mapping format: "{event_id}:{email_lower}"
    let walkin_key = format!("{WALKIN_PREFIX}{mapping}");

    let walkin: Option<WalkinAttendee> = kv.get(&walkin_key).json().await.ok().flatten();

    walkin
}

/// Mark a walk-in attendee as claimed (update KV record with wallet + timestamp).
pub(crate) async fn mark_walkin_claimed(
    kv: &KvStore,
    event_id: &str,
    email: &str,
    wallet_address: &str,
    claimed_at: &str,
) -> Result<(), String> {
    let email_lower = email.to_lowercase();
    let walkin_key = format!("{WALKIN_PREFIX}{event_id}:{email_lower}");

    let mut walkin: WalkinAttendee = kv
        .get(&walkin_key)
        .json()
        .await
        .ok()
        .flatten()
        .ok_or_else(|| format!("walk-in record not found: {walkin_key}"))?;

    walkin.wallet_address = Some(wallet_address.to_string());
    walkin.claimed_at = Some(claimed_at.to_string());

    kv.put(&walkin_key, serde_json::to_string(&walkin).unwrap())
        .map_err(|e| format!("walk-in claimed update failed: {e:?}"))?
        .expiration_ttl(86400 * 90) // 90 days
        .execute()
        .await
        .map_err(|e| format!("walk-in claimed write failed: {e:?}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // claim_lock_key
    // ==========================================================================

    #[test]
    fn claim_lock_key_format() {
        let key = claim_lock_key("evt-123", "tok-abc");
        assert_eq!(key, "event:evt-123:claim_lock:tok-abc");
    }

    #[test]
    fn claim_lock_key_with_special_chars() {
        let key = claim_lock_key("evt_2025", "token-uuid-v7");
        assert_eq!(key, "event:evt_2025:claim_lock:token-uuid-v7");
    }

    #[test]
    fn claim_lock_key_with_empty_inputs() {
        let key = claim_lock_key("", "");
        assert_eq!(key, "event::claim_lock:");
    }

    // ==========================================================================
    // mask_wallet
    // ==========================================================================

    #[test]
    fn mask_wallet_normal_address() {
        let addr = "BxRWqK3KjF8Mn2dTsUfMZ8xJbQHvYC3KjF";
        let masked = mask_wallet(addr);
        assert_eq!(masked, "BxRW...3KjF");
    }

    #[test]
    fn mask_wallet_exactly_8_chars() {
        let addr = "12345678";
        let masked = mask_wallet(addr);
        assert_eq!(masked, "****");
    }

    #[test]
    fn mask_wallet_9_chars() {
        let addr = "123456789";
        let masked = mask_wallet(addr);
        assert_eq!(masked, "1234...6789");
    }

    #[test]
    fn mask_wallet_short_address() {
        let masked = mask_wallet("short");
        assert_eq!(masked, "****");
    }

    #[test]
    fn mask_wallet_single_char() {
        let masked = mask_wallet("A");
        assert_eq!(masked, "****");
    }

    #[test]
    fn mask_wallet_empty_string() {
        let masked = mask_wallet("");
        assert_eq!(masked, "****");
    }
}
