//! Durable orchestration around the external Crossmint call.

use worker::KvStore;

use crate::solana::{MintRequest, MintResult};
use crate::state::AppState;

const CONFIRMED_RECOVERY_TTL_SECS: u64 = 7 * 86_400;

fn recovery_key(token: &str) -> String {
    format!("crossmint:confirmed:{token}")
}

/// Return a prior confirmed result or perform one external mint and journal its
/// result before the caller updates the attendee projection.
pub(super) async fn mint_with_journal(
    state: &AppState,
    event_id: &str,
    token: &str,
    wallet: &str,
    request: &MintRequest<'_>,
    kv: Option<&KvStore>,
) -> Result<MintResult, String> {
    let provider_mint_id = crate::solana::crossmint_mint_id(request.idempotency_key)
        .ok_or_else(|| "durable mint requires an idempotency key".to_string())?;
    if let Some(db) = state.d1.as_deref()
        && let Some(confirmed) = crate::db::nft_mint_jobs::begin_or_resume(
            db,
            event_id,
            token,
            wallet,
            &provider_mint_id,
        )
        .await?
    {
        tracing::info!(
            claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
            "resuming confirmed mint from D1 journal"
        );
        return Ok(MintResult {
            asset_id: confirmed.asset_id,
            signature: confirmed.signature,
        });
    }

    // A confirmed KV copy covers the narrow case where Crossmint succeeded but
    // D1 was temporarily unavailable while recording the result.
    if let Some(kv) = kv
        && let Ok(Some(raw)) = kv.get(&recovery_key(token)).text().await
        && let Ok(result) = serde_json::from_str::<MintResult>(&raw)
    {
        if let Some(db) = state.d1.as_deref() {
            crate::db::nft_mint_jobs::mark_confirmed(
                db,
                event_id,
                token,
                &result.asset_id,
                &result.signature,
            )
            .await?;
        }
        return Ok(result);
    }

    let result = crate::solana::mint_compressed_nft(request, kv).await?;
    let serialized = serde_json::to_string(&result)
        .map_err(|error| format!("mint result serialize failed: {error}"))?;
    let kv_saved = if let Some(kv) = kv {
        match kv.put(&recovery_key(token), serialized) {
            Ok(builder) => builder
                .expiration_ttl(CONFIRMED_RECOVERY_TTL_SECS)
                .execute()
                .await
                .is_ok(),
            Err(_) => false,
        }
    } else {
        false
    };
    let d1_saved = match state.d1.as_deref() {
        Some(db) => crate::db::nft_mint_jobs::mark_confirmed(
            db,
            event_id,
            token,
            &result.asset_id,
            &result.signature,
        )
        .await
        .is_ok(),
        None => false,
    };
    if state.d1.is_some() && !d1_saved && !kv_saved {
        return Err(
            "mint confirmed externally but durable result recording failed; retry later".into(),
        );
    }
    Ok(result)
}

/// Mark the attendee projection complete and remove the short-lived KV rescue
/// copy. D1 journal cleanup is best-effort because `confirmed` remains safe to
/// replay if this final status update fails.
pub(super) async fn mark_projection_persisted(
    state: &AppState,
    event_id: &str,
    token: &str,
    kv: Option<&KvStore>,
) {
    if let Some(db) = state.d1.as_deref()
        && let Err(error) = crate::db::nft_mint_jobs::mark_persisted(db, event_id, token).await
    {
        tracing::warn!(
            claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
            error = %error,
            "mint journal persisted-state update failed"
        );
        return;
    }
    if let Some(kv) = kv {
        let _ = kv.delete(&recovery_key(token)).await;
        let _ = kv.delete(&format!("crossmint:pending:{token}")).await;
    }
}
