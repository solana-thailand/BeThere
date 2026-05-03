//! Claim service module — core business logic for NFT claim lookup and minting.
//!
//! Extracted from the HTTP handler layer so the claim flow can be tested
//! and reused independently of Axum/Workers request types.

use chrono::Utc;
use worker::KvStore;

use event_checkin_domain::models::adventure::AdventureStatus;
use event_checkin_domain::models::api::{EventConfig as ApiEventConfig, QuizStatus};
use event_checkin_domain::models::error::AppError;

use crate::handlers::ext::{resolve_event, resolve_kv};
use crate::solana::{self, MintRequest};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Lock helpers (pub(crate) for reuse by handlers if needed)
// ---------------------------------------------------------------------------

/// KV key for claim dedup lock
pub(crate) fn claim_lock_key(event_id: &str, token: &str) -> String {
    format!("event:{event_id}:claim_lock:{token}")
}

/// Try to acquire a claim lock. Returns Ok(()) if acquired, Err if already locked.
/// Sets a 5-minute TTL as safety net.
pub(crate) async fn acquire_claim_lock(
    kv: &KvStore,
    event_id: &str,
    token: &str,
    wallet: &str,
) -> Result<(), String> {
    let key = claim_lock_key(event_id, token);

    // Check if lock already exists
    let existing: Option<String> = kv
        .get(&key)
        .text()
        .await
        .map_err(|e| format!("claim lock read failed: {e:?}"))?;

    if existing.is_some() {
        tracing::warn!("claim lock already held for token {token}");
        return Err("claim is already being processed or has been completed".to_string());
    }

    // Acquire lock with 5-minute TTL (safety net for failed mints)
    let lock_value = serde_json::json!({
        "wallet": wallet,
        "started_at": chrono::Utc::now().to_rfc3339(),
    })
    .to_string();

    kv.put(&key, &lock_value)
        .map_err(|e| format!("claim lock put failed: {e:?}"))?
        .expiration_ttl(300) // 5 minutes TTL
        .execute()
        .await
        .map_err(|e| format!("claim lock write failed: {e:?}"))?;

    tracing::info!("claim lock acquired for token {token}");
    Ok(())
}

/// Finalize the claim lock after successful mint (removes TTL, sets final data).
pub(crate) async fn finalize_claim_lock(
    kv: &KvStore,
    event_id: &str,
    token: &str,
    wallet: &str,
    asset_id: &str,
) -> Result<(), String> {
    let key = claim_lock_key(event_id, token);

    let lock_value = serde_json::json!({
        "wallet": wallet,
        "asset_id": asset_id,
        "claimed_at": chrono::Utc::now().to_rfc3339(),
    })
    .to_string();

    // Overwrite without TTL — permanent record of claim
    kv.put(&key, &lock_value)
        .map_err(|e| format!("claim lock finalize failed: {e:?}"))?
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
) -> Result<(), String> {
    let key = claim_lock_key(event_id, token);
    kv.delete(&key)
        .await
        .map_err(|e| format!("claim lock release failed: {e:?}"))?;
    tracing::info!("claim lock released for token {token}");
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
// Result types
// ---------------------------------------------------------------------------

/// Result of a successful claim lookup (GET).
pub struct ClaimLookup {
    pub name: String,
    pub checked_in_at: String,
    pub claim_token: String,
    pub claimed: bool,
    pub claimed_at: Option<String>,
    pub nft_available: bool,
    pub locked_wallet: Option<String>,
    pub event: ApiEventConfig,
    pub quiz_status: QuizStatus,
    pub total_checked_in: usize,
    pub total_claimed: usize,
}

/// Result of a successful NFT claim (POST).
pub struct ClaimResult {
    pub name: String,
    pub asset_id: String,
    pub signature: String,
    pub wallet_address: String,
    pub claimed_at: String,
    pub cluster: String,
}

// ---------------------------------------------------------------------------
// Lookup logic (GET /api/claim/{token})
// ---------------------------------------------------------------------------

/// Look up claim status by token. Returns attendee info, event config, quiz/adventure status.
pub async fn lookup_claim(
    state: &AppState,
    token: &str,
    event_id: Option<&str>,
) -> Result<ClaimLookup, AppError> {
    tracing::info!("claim lookup for token: {token}");

    let event = resolve_event(state, event_id).await?;

    let kv = resolve_kv(state);
    let (attendee, total_checked_in, total_claimed) =
        match crate::sheets::get_attendee_with_claim_counts(
            token,
            state,
            &event.sheet_id,
            &event.sheet_name,
            kv,
        )
        .await
        {
            Ok((Some(a), checked_in, claimed)) => (a, checked_in, claimed),
            Ok((None, _, _)) => {
                tracing::warn!("claim lookup: no attendee found for token {token}");
                return Err(AppError::NotFound("claim token not found".into()));
            }
            Err(ref e) => {
                tracing::error!("claim lookup failed for token {token}: {e}");
                return Err(AppError::Internal(format!("failed to look up claim: {e}")));
            }
        };

    let display_name = attendee.display_name().to_string();
    let checked_in_at = attendee.checked_in_at.clone().unwrap_or_default();
    let claimed = attendee.claimed_at.is_some();
    let claimed_at = attendee.claimed_at.clone();

    // Check if NFT minting is fully configured (all required secrets present)
    let nft_available = !event.nft_metadata_uri.is_empty()
        && !event.nft_image_url.is_empty()
        && !state.config.solana.api_key.is_empty();

    let api_event = ApiEventConfig {
        event_name: event.name.clone(),
        event_tagline: event.tagline.clone(),
        event_link: event.link.clone(),
        event_start_ms: event.event_start_ms,
        event_end_ms: event.event_end_ms,
    };

    // Pre-registered wallet from column P — locks claim to this address if present
    let locked_wallet = attendee
        .solana_address
        .as_ref()
        .map(|w| w.trim().to_string())
        .filter(|w| !w.is_empty());

    // Determine quiz status (Issue 002 — activity-gated claim)
    let quiz_status = match &state.events_kv {
        Some(kv) => {
            let eid = &event.id;
            crate::quiz::get_quiz_status(kv, eid, token)
                .await
                .unwrap_or(QuizStatus::NotRequired)
        }
        None => match &state.quiz_kv {
            Some(kv) => crate::quiz::get_quiz_status(kv, "default", token)
                .await
                .unwrap_or(QuizStatus::NotRequired),
            None => QuizStatus::NotRequired,
        },
    };

    Ok(ClaimLookup {
        name: display_name,
        checked_in_at,
        claim_token: token.to_string(),
        claimed,
        claimed_at,
        nft_available,
        locked_wallet,
        event: api_event,
        quiz_status,
        total_checked_in,
        total_claimed,
    })
}

// ---------------------------------------------------------------------------
// Execute claim logic (POST /api/claim/{token})
// ---------------------------------------------------------------------------

/// Execute the full claim flow: validate → gates → lock → mint → record.
/// This is the core business logic extracted from the handler.
pub async fn execute_claim(
    state: &AppState,
    token: &str,
    wallet_address: &str,
    event_id: Option<&str>,
) -> Result<ClaimResult, AppError> {
    tracing::info!("claim mint request for token: {token}");

    // 1. Resolve event context
    let event = resolve_event(state, event_id).await?;

    // 2. Look up attendee by claim token
    let kv = resolve_kv(state);
    let attendee = match crate::sheets::get_attendee_by_claim_token(
        token,
        state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
    )
    .await
    {
        Ok(Some(a)) => a,
        Ok(None) => {
            tracing::warn!("claim mint: no attendee found for token {token}");
            return Err(AppError::NotFound("claim token not found".into()));
        }
        Err(ref e) => {
            tracing::error!("claim mint lookup failed for token {token}: {e}");
            return Err(AppError::Internal(format!("failed to look up claim: {e}")));
        }
    };

    let display_name = attendee.display_name().to_string();

    // 3. Must be checked in
    if attendee.checked_in_at.is_none() {
        return Err(AppError::Validation(
            "attendee has not been checked in yet".into(),
        ));
    }

    // 4. Quiz gate — must pass quiz before claiming (Issue 002)
    let quiz_check = if let Some(ref kv) = state.events_kv {
        Some((kv, event.id.as_str()))
    } else {
        state.quiz_kv.as_ref().map(|kv| (kv, "default"))
    };

    if let Some((kv, eid)) = quiz_check {
        let quiz_status = match crate::quiz::get_quiz_status(kv, eid, token).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("claim mint: failed to check quiz status for token {token}: {e}");
                return Err(AppError::Internal(format!(
                    "failed to verify quiz status: {e}"
                )));
            }
        };
        match quiz_status {
            QuizStatus::NotRequired | QuizStatus::Passed => {}
            QuizStatus::NotStarted => {
                tracing::warn!("claim mint blocked: quiz not attempted for token {token}");
                return Err(AppError::Validation(
                    "you must complete the quiz before claiming your badge".into(),
                ));
            }
            QuizStatus::InProgress => {
                tracing::warn!("claim mint blocked: quiz not passed for token {token}");
                return Err(AppError::Validation(
                    "you must pass the quiz before claiming your badge".into(),
                ));
            }
        }
    }

    // 5. Adventure gate — must complete adventure before claiming
    let adv_kv = if let Some(ref kv) = state.events_kv {
        Some(kv)
    } else {
        state.quiz_kv.as_ref()
    };

    if let Some(kv) = adv_kv {
        let adv_status = match crate::adventure::get_adventure_status(kv, &event.id, token).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(
                    "claim mint: failed to check adventure status for token {token}: {e}"
                );
                return Err(AppError::Internal(format!(
                    "failed to verify adventure status: {e}"
                )));
            }
        };
        match adv_status {
            AdventureStatus::NotRequired | AdventureStatus::Passed => {}
            AdventureStatus::NotStarted => {
                tracing::warn!("claim mint blocked: adventure not attempted for token {token}");
                return Err(AppError::Validation(
                    "you must complete the Rust Adventure before claiming your badge".into(),
                ));
            }
            AdventureStatus::InProgress => {
                tracing::warn!("claim mint blocked: adventure not passed for token {token}");
                return Err(AppError::Validation(
                    "you must complete the Rust Adventure before claiming your badge".into(),
                ));
            }
        }
    }

    // 6. Must not be already claimed
    if attendee.claimed_at.is_some() {
        let claimed_at = attendee.claimed_at.as_deref().unwrap_or("unknown");
        tracing::warn!("claim already fulfilled for token {token} at {claimed_at}");
        return Err(AppError::Validation("NFT has already been claimed".into()));
    }

    // 7. Wallet match guard: if attendee pre-registered a Solana address (column P),
    // the claiming wallet must match exactly. Prevents claim theft via leaked URLs.
    if let Some(ref registered) = attendee.solana_address {
        let registered = registered.trim();
        if !registered.is_empty() {
            let claiming = wallet_address.trim();
            if registered != claiming {
                tracing::warn!(
                    "wallet mismatch for token {token}: registered={} claiming={}",
                    mask_wallet(registered),
                    mask_wallet(claiming)
                );
                return Err(AppError::Validation(format!(
                    "This claim is locked to a pre-registered wallet ({})",
                    mask_wallet(registered)
                )));
            }
        }
    }

    // 8. Claim dedup lock — prevent concurrent double-claim (KV-based mutex)
    let lock_kv: Option<&KvStore> = resolve_kv(state);
    if let Some(kv) = lock_kv
        && let Err(e) = acquire_claim_lock(kv, &event.id, token, wallet_address).await
    {
        return Err(AppError::RateLimited(e));
    }

    // 9. Mint compressed NFT via Helius
    let config = &state.config;
    let mint_req = MintRequest {
        wallet_address,
        rpc_url: &config.solana.rpc_url,
        api_key: &config.solana.api_key,
        collection_mint: &event.nft_collection_mint,
        metadata_uri: &event.nft_metadata_uri,
        image_url: &event.nft_image_url,
        nft_name: &event.nft_name(),
        nft_symbol: &event.nft_symbol,
        nft_description: &event.nft_description(),
        nft_external_url: &event.link,
        merkle_tree: &event.merkle_tree,
    };
    let mint_result = match solana::mint_compressed_nft(&mint_req).await {
        Ok(result) => result,
        Err(ref e) => {
            tracing::error!("mint failed for token {token}: {e}");
            // Release lock so attendee can retry
            if let Some(kv) = lock_kv {
                let _ = release_claim_lock(kv, &event.id, token).await;
            }
            return Err(AppError::External {
                service: "helius".into(),
                status: 502,
                body: e.to_string(),
            });
        }
    };

    // 10. Mark as claimed in Google Sheet (column G = wallet, column M = claimed_at)
    let claimed_at = Utc::now().to_rfc3339();
    if let Err(ref e) = crate::sheets::mark_claimed(
        attendee.row_index,
        wallet_address,
        &claimed_at,
        state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
    )
    .await
    {
        tracing::error!("mint succeeded but failed to mark claimed for token {token}: {e}");
        // Lock will expire via TTL — don't release (mint already happened)
        return Err(AppError::Internal(format!(
            "NFT minted but failed to record claim. Asset ID: {}. Error: {e}",
            mint_result.asset_id
        )));
    }

    // 11. Finalize claim lock (permanent record, no TTL) — non-blocking
    if let Some(kv) = lock_kv
        && let Err(e) =
            finalize_claim_lock(kv, &event.id, token, wallet_address, &mint_result.asset_id).await
    {
        tracing::warn!("claim lock finalize failed (non-blocking): {e}");
    }

    tracing::info!(
        "claim fulfilled: token={token} name={display_name} asset_id={} wallet={}",
        mint_result.asset_id,
        wallet_address
    );

    // 12. Return result
    let cluster = if config.solana.rpc_url.contains("mainnet") {
        "mainnet-beta"
    } else {
        "devnet"
    };

    Ok(ClaimResult {
        name: display_name,
        asset_id: mint_result.asset_id,
        signature: mint_result.signature,
        wallet_address: wallet_address.to_string(),
        claimed_at,
        cluster: cluster.to_string(),
    })
}
