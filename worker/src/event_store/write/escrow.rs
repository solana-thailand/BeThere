//! Escrow → event reverse index writes.

use worker::KvStore;

use crate::event_store::schema::escrow_index_key;

/// Write the escrow → event reverse index entry — D1 only.
///
/// KV write removed: it was a redundant dual-write (D1 is the source of truth)
/// that consumed the scarce free-tier write quota. `get_event_id_by_escrow` is
/// D1-first with KV fallback, so reads are unaffected. The `kv` parameter is
/// retained in the signature for API stability; it is intentionally unused.
/// See issue #053 Phase 3b (on-chain index migration).
pub async fn save_escrow_index(
    d1: Option<&worker::D1Database>,
    _kv: Option<&KvStore>,
    escrow_address: &str,
    event_id: &str,
) -> Result<(), String> {
    if escrow_address.is_empty() {
        return Ok(()); // no escrow to index
    }

    // D1 write (primary, and now sole, store)
    if let Some(db) = d1
        && let Err(e) =
            crate::db::escrow_index::upsert_escrow_index_to_d1(db, escrow_address, event_id).await
    {
        tracing::warn!(escrow_address, error = %e, "D1 escrow index write failed");
    }

    Ok(())
}

/// Remove the escrow → event reverse index entry — D1 + KV.
pub async fn delete_escrow_index(
    d1: Option<&worker::D1Database>,
    kv: Option<&KvStore>,
    escrow_address: &str,
) -> Result<(), String> {
    if escrow_address.is_empty() {
        return Ok(());
    }

    // D1 delete
    if let Some(db) = d1
        && let Err(e) =
            crate::db::escrow_index::delete_escrow_index_from_d1(db, escrow_address).await
    {
        tracing::warn!(escrow_address, error = %e, "D1 escrow index delete failed");
    }

    // KV delete
    if let Some(kv_ref) = kv {
        kv_ref
            .delete(&escrow_index_key(escrow_address))
            .await
            .map_err(|e| format!("failed to delete escrow index: {e:?}"))?;
    }

    Ok(())
}
