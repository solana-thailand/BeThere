//! Claim lock acquisition, release, and finalization.
//!
//! Provides a write-first/verify-after lock mechanism backed by KV (with D1
//! atomic fallback) to prevent concurrent double-claims.
//!
//! Phase 1 (Issue #050): Routes through Durable Object when available for
//! truly ACID claim lock operations. Falls back to D1 + KV when DO is not
//! configured.

use worker::{KvStore, Method, ObjectNamespace, Request, RequestInit, Response};

use crate::db;
use crate::durable_objects::DoRequest;

// ---------------------------------------------------------------------------
// TTL constants
// ---------------------------------------------------------------------------

/// TTL for finalized claim lock records (90 days).
/// Auto-cleanup post-event; permanent records are not needed indefinitely.
pub(crate) const CLAIM_LOCK_FINALIZE_TTL_SECS: u64 = 86400 * 90;

/// Expiry stamp for a *finalized* claim lock row.
///
/// `claim_locks.expires_at` is `TEXT NOT NULL` in both the D1 migration
/// (`0001_initial.sql`) and the DO schema (`event_do/schema.rs`), so
/// finalization cannot null it out to "remove the TTL" — SQLite aborts the
/// whole `UPDATE`. The row carries the same 90-day retention horizon as the
/// finalized KV record instead, which keeps `idx_claim_locks_expires`
/// meaningful for any future sweeper. `claimed_at IS NOT NULL` is the marker
/// for "finalized"; `handle_acquire_claim_lock` already reads it that way.
pub(crate) fn finalized_expires_at() -> String {
    (chrono::Utc::now() + chrono::Duration::seconds(CLAIM_LOCK_FINALIZE_TTL_SECS as i64))
        .to_rfc3339()
}

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

/// Run a DO RPC and flatten both failure modes — a transport error and a
/// `success: false` response — into one `Option<String>`.
///
/// Used by the finalize/release paths, which must carry the DO error *past* a
/// mandatory KV write rather than `?`-ing out of it: the KV record is what the
/// attendee actually reads (finalize) and what actually blocks their retry
/// (release), so it has to be written even when the durable side failed.
/// `acquire` deliberately does not use this — there, a DO failure must abort.
async fn do_rpc_err(
    namespace: &ObjectNamespace,
    event_id: &str,
    request: DoRequest,
    fallback: &str,
) -> Option<String> {
    match do_rpc(namespace, event_id, request).await {
        Err(e) => Some(e),
        Ok(resp) => match resp.success {
            true => None,
            false => Some(resp.error.unwrap_or_else(|| fallback.to_string())),
        },
    }
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
                claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
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
            claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
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
                claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
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
            claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
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
                    claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
                    lock_id = %lock_id,
                    "claim lock acquired (write-first verify)"
                );
                Ok(())
            } else {
                // Another request won — the winner's lock is authoritative.
                tracing::warn!(
                    claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
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
                claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
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
        // The KV record is what the claim and attendee read paths use to show
        // the attendee their asset_id / signature, so it is written even when
        // the DO write failed: a failed durable write must not also cost the
        // attendee their proof link. That covers a transport failure too, hence
        // `do_rpc_err` rather than `?`. The DO error is reported after.
        let do_err = do_rpc_err(
            namespace,
            event_id,
            DoRequest::FinalizeClaimLock {
                event_id: event_id.to_string(),
                token: token.to_string(),
                asset_id: asset_id.to_string(),
                signature: signature.to_string(),
                claimed_at: claimed_at.clone(),
            },
            "claim lock finalization failed (DO)",
        )
        .await;
        if let Some(e) = do_err.as_deref() {
            tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), error = %e, "DO finalize claim lock failed");
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

        return match do_err {
            Some(e) => Err(e),
            None => {
                tracing::info!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim lock finalized (DO+KV)");
                Ok(())
            }
        };
    }

    // D1 path: UPDATE claim_locks. Same ordering rule as the DO path — the
    // error is held until after the KV record is written.
    let d1_err = match d1 {
        Some(db) => {
            let claimed_at = chrono::Utc::now().to_rfc3339();
            db::finalize_claim_lock(db, event_id, token, asset_id, signature, &claimed_at)
                .await
                .err()
        }
        None => None,
    };

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

    match d1_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
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
        // The KV key is what actually blocks the attendee's retry, so it is
        // deleted even when the DO side failed — including a transport failure,
        // hence `do_rpc_err` rather than `?`.
        let do_err = do_rpc_err(
            namespace,
            event_id,
            DoRequest::ReleaseClaimLock {
                event_id: event_id.to_string(),
                token: token.to_string(),
            },
            "claim lock release failed (DO)",
        )
        .await;
        if let Some(e) = do_err.as_deref() {
            tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), error = %e, "DO release claim lock failed");
        }

        // Always delete from KV
        let key = claim_lock_key(event_id, token);
        kv.delete(&key)
            .await
            .map_err(|e| format!("claim lock release failed: {e:?}"))?;
        return match do_err {
            Some(e) => Err(e),
            None => {
                tracing::info!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim lock released (DO+KV)");
                Ok(())
            }
        };
    }

    // D1 path: DELETE. Hold any error until after the KV delete below — the KV
    // key is what actually blocks a retry, so a failed D1 delete must not also
    // strand the attendee behind a lock for the rest of its 5-minute TTL.
    let mut d1_err = None;
    if let Some(db) = d1 {
        match db::release_claim_lock(db, event_id, token).await {
            Ok(()) => {
                tracing::info!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim lock released (D1+KV)")
            }
            Err(e) => {
                tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), error = %e, "D1 release claim lock failed");
                d1_err = Some(e);
            }
        }
    }

    // Always delete from KV
    let key = claim_lock_key(event_id, token);
    kv.delete(&key)
        .await
        .map_err(|e| format!("claim lock release failed: {e:?}"))?;
    tracing::info!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim lock released");

    match d1_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
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
    // finalized_expires_at
    // ==========================================================================

    #[test]
    fn finalized_expires_at_is_the_kv_retention_horizon() {
        let stamp = finalized_expires_at();
        let parsed = chrono::DateTime::parse_from_rfc3339(&stamp).expect(
            "finalized expiry must be RFC3339 — the column is TEXT and is compared as text",
        );
        let delta = parsed.with_timezone(&chrono::Utc) - chrono::Utc::now();

        // Same 90 days the finalized KV record gets, within a generous slack for
        // the clock ticking between the two calls.
        let expected = chrono::Duration::seconds(CLAIM_LOCK_FINALIZE_TTL_SECS as i64);
        assert!(
            (delta - expected).num_seconds().abs() < 60,
            "expected ~{} days out, got {stamp}",
            expected.num_days()
        );
    }

    #[test]
    fn finalized_expires_at_is_never_null_or_empty() {
        // The whole point: `claim_locks.expires_at` is NOT NULL, so finalization
        // must always have a real value to write.
        assert!(!finalized_expires_at().is_empty());
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
