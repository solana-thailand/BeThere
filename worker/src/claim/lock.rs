//! Claim lock acquisition, release, and finalization.
//!
//! Provides a write-first/verify-after lock mechanism backed by KV (with D1
//! atomic fallback) to prevent concurrent double-claims.

use worker::KvStore;

use event_checkin_domain::models::attendee::WalkinAttendee;

use crate::db;

// ---------------------------------------------------------------------------
// TTL constants
// ---------------------------------------------------------------------------

/// TTL for finalized claim lock records (90 days).
/// Auto-cleanup post-event; permanent records are not needed indefinitely.
pub(crate) const CLAIM_LOCK_FINALIZE_TTL_SECS: u64 = 86400 * 90;

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
/// Uses write-first, verify-after pattern to eliminate the TOCTOU race between
/// checking for an existing lock and writing a new one. Each caller writes a
/// unique lock_id, then reads back to confirm it won the race.
pub(crate) async fn acquire_claim_lock(
    kv: &KvStore,
    event_id: &str,
    token: &str,
    wallet: &str,
    d1: Option<&worker::D1Database>,
) -> Result<(), String> {
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

/// Finalize the claim lock after successful mint (removes TTL, sets final data).
pub(crate) async fn finalize_claim_lock(
    kv: &KvStore,
    event_id: &str,
    token: &str,
    wallet: &str,
    asset_id: &str,
    signature: &str,
    d1: Option<&worker::D1Database>,
) -> Result<(), String> {
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
pub(crate) async fn release_claim_lock(
    kv: &KvStore,
    event_id: &str,
    token: &str,
    d1: Option<&worker::D1Database>,
) -> Result<(), String> {
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
