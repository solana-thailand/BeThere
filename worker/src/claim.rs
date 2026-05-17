//! Claim service module — core business logic for NFT claim lookup and minting.
//!
//! Extracted from the HTTP handler layer so the claim flow can be tested
//! and reused independently of Axum/Workers request types.

use chrono::Utc;
use worker::KvStore;

use event_checkin_domain::models::adventure::AdventureStatus;
use event_checkin_domain::models::api::{EventConfig as ApiEventConfig, QuizStatus};
use event_checkin_domain::models::attendee::WalkinAttendee;
use event_checkin_domain::models::error::AppError;

use crate::handlers::ext::{resolve_event, resolve_kv};
use crate::solana::{self, MintRequest};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// TTL constants
// ---------------------------------------------------------------------------

/// TTL for finalized claim lock records (90 days).
/// Auto-cleanup post-event; permanent records are not needed indefinitely.
const CLAIM_LOCK_FINALIZE_TTL_SECS: u64 = 86400 * 90;

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
        tracing::warn!(claim_token = %token, "claim lock already held");
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

    tracing::info!(claim_token = %token, "claim lock acquired");
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
) -> Result<(), String> {
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
    pub api_id: String,
    pub event_id: String,
    pub deposit_enabled: bool,
    pub deposit_amount_usdc: u64,
    pub deposit_amount_thb: u64,
    pub participation_type: String,
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
    tracing::info!(claim_token = %token, "claim lookup");

    let event = resolve_event(state, event_id).await?;
    let kv = resolve_kv(state);

    // ── Walk-in path: check KV for walk-in attendee first ──
    if let Some(kv_ref) = kv
        && let Some(walkin) = lookup_walkin_by_claim_token(kv_ref, token).await
    {
        tracing::info!(claim_token = %token, email = %walkin.email, "claim lookup: found walk-in attendee");

        let nft_available = !event.nft_metadata_uri.is_empty()
            && !event.nft_image_url.is_empty()
            && !state.config.solana.api_key.is_empty();

        return Ok(ClaimLookup {
            name: walkin.name.clone(),
            checked_in_at: walkin.checked_in_at.clone(),
            claim_token: token.to_string(),
            claimed: walkin.claimed_at.is_some(),
            claimed_at: walkin.claimed_at.clone(),
            nft_available,
            locked_wallet: walkin.wallet_address.clone(),
            event: ApiEventConfig {
                event_name: event.name.clone(),
                event_tagline: event.tagline.clone(),
                event_link: event.link.clone(),
                event_start_ms: event.event_start_ms,
                event_end_ms: event.event_end_ms,
            },
            quiz_status: QuizStatus::NotRequired, // walk-ins skip quiz
            total_checked_in: 0,                  // walk-ins don't contribute to sheet stats
            total_claimed: 0,
            api_id: format!("walkin:{}", walkin.email),
            event_id: event.id.clone(),
            deposit_enabled: event.deposit_enabled,
            deposit_amount_usdc: event.deposit_amount_usdc,
            deposit_amount_thb: event.deposit_amount_thb,
            participation_type: "In-Person".to_string(), // walk-ins are always in-person
        });
    }

    // ── Pre-registered path: look up from Google Sheet ──
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
                tracing::warn!(claim_token = %token, "claim lookup: no attendee found");
                return Err(AppError::NotFound("claim token not found".into()));
            }
            Err(ref e) => {
                tracing::error!(claim_token = %token, error = %e, "claim lookup failed");
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
        api_id: attendee.api_id.clone(),
        event_id: event.id.clone(),
        deposit_enabled: event.deposit_enabled,
        deposit_amount_usdc: event.deposit_amount_usdc,
        deposit_amount_thb: event.deposit_amount_thb,
        participation_type: attendee.participation_type.clone(),
    })
}

// ---------------------------------------------------------------------------
// Execute claim logic (POST /api/claim/{token})
// ---------------------------------------------------------------------------

/// Execute the full claim flow: validate → gates → lock → mint → record.
/// This is the core business logic extracted from the handler.
/// Check if an online attendee has completed the required quest (quiz or adventure).
/// Returns true if at least one is passed, or if neither is required.
async fn verify_online_quest_completion(
    state: &AppState,
    event_id: &str,
    claim_token: &str,
) -> bool {
    let kv = match state.events_kv.as_ref().or(state.quiz_kv.as_ref()) {
        Some(kv) => kv,
        None => return true, // no KV = no quest required
    };

    // Check quiz status first
    match crate::quiz::get_quiz_status(kv, event_id, claim_token).await {
        Ok(QuizStatus::Passed) => true,
        Ok(QuizStatus::NotRequired) => {
            // Quiz not configured — check adventure
            match crate::adventure::get_adventure_status(kv, event_id, claim_token).await {
                Ok(AdventureStatus::Passed) => true,
                Ok(AdventureStatus::NotRequired) => true, // no quest configured at all
                _ => false,
            }
        }
        _ => false,
    }
}

pub async fn execute_claim(
    state: &AppState,
    token: &str,
    wallet_address: &str,
    event_id: Option<&str>,
) -> Result<ClaimResult, AppError> {
    tracing::info!(claim_token = %token, "claim mint request");

    // 1. Resolve event context
    let event = resolve_event(state, event_id).await?;
    let kv = resolve_kv(state);

    // 2. Check walk-in path first
    if let Some(kv_ref) = kv
        && let Some(walkin) = lookup_walkin_by_claim_token(kv_ref, token).await
    {
        return execute_walkin_claim(state, kv_ref, &event, token, wallet_address, walkin).await;
    }

    // 3. Pre-registered path: look up attendee by claim token from Google Sheet
    let mut attendee = match crate::sheets::get_attendee_by_claim_token(
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
            tracing::warn!(claim_token = %token, "claim mint: no attendee found");
            return Err(AppError::NotFound("claim token not found".into()));
        }
        Err(ref e) => {
            tracing::error!(claim_token = %token, error = %e, "claim mint lookup failed");
            return Err(AppError::Internal(format!("failed to look up claim: {e}")));
        }
    };

    let display_name = attendee.display_name().to_string();

    // 3. Check-in verification — with virtual check-in for online attendees
    if attendee.checked_in_at.is_none() {
        let is_online_attendee = !attendee.is_in_person();
        if is_online_attendee && event.event_format.has_online() {
            // Verify quiz/adventure completion (at least one must be passed)
            let quest_passed = verify_online_quest_completion(state, &event.id, token).await;
            if quest_passed {
                // Resolve column mapping
                let mapping = match crate::sheets::get_column_mapping(
                    state,
                    &event.sheet_id,
                    &event.sheet_name,
                    kv,
                )
                .await
                {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!(error = %e, "column mapping fallback to hardcoded");
                        event_checkin_domain::models::attendee::ColumnMapping::hardcoded()
                    }
                };

                // Auto virtual check-in — write to Google Sheet
                match crate::sheets::mark_virtual_checked_in(
                    attendee.row_index,
                    &mapping,
                    state,
                    &event.sheet_id,
                    &event.sheet_name,
                    kv,
                )
                .await
                {
                    Ok(ts) => {
                        tracing::info!(
                            claim_token = %token,
                            attendee_id = %attendee.api_id,
                            checked_in_at = %ts,
                            "virtual check-in auto-completed for online attendee"
                        );
                        attendee.checked_in_at = Some(ts);
                    }
                    Err(e) => {
                        tracing::error!(
                            claim_token = %token,
                            error = %e,
                            "failed to auto virtual check-in"
                        );
                        return Err(AppError::Internal(format!(
                            "failed to complete virtual check-in: {e}"
                        )));
                    }
                }
            } else {
                tracing::warn!(
                    claim_token = %token,
                    participation_type = %attendee.participation_type,
                    "online attendee not checked in and quest not completed"
                );
                return Err(AppError::Validation(
                    "you must complete the quiz or adventure before claiming your badge".into(),
                ));
            }
        } else {
            return Err(AppError::Validation(
                "attendee has not been checked in yet".into(),
            ));
        }
    }

    // 4+5. Quiz and Adventure gates — run concurrently to parallelize KV reads
    let quiz_kv = if let Some(ref kv) = state.events_kv {
        Some((kv, event.id.as_str()))
    } else {
        state.quiz_kv.as_ref().map(|kv| (kv, "default"))
    };
    let adv_kv = if let Some(ref kv) = state.events_kv {
        Some(kv)
    } else {
        state.quiz_kv.as_ref()
    };

    let quiz_fut = async {
        if let Some((kv, eid)) = quiz_kv {
            crate::quiz::get_quiz_status(kv, eid, token).await.ok()
        } else {
            None
        }
    };
    let adv_fut = async {
        if let Some(kv) = adv_kv {
            crate::adventure::get_adventure_status(kv, &event.id, token)
                .await
                .ok()
        } else {
            None
        }
    };

    let (quiz_result, adv_result) = futures::join!(quiz_fut, adv_fut);

    // Check quiz gate
    if let Some(quiz_status) = quiz_result {
        match quiz_status {
            QuizStatus::NotRequired | QuizStatus::Passed => {}
            QuizStatus::NotStarted => {
                tracing::warn!(claim_token = %token, "claim mint blocked: quiz not attempted");
                return Err(AppError::Validation(
                    "you must complete the quiz before claiming your badge".into(),
                ));
            }
            QuizStatus::InProgress => {
                tracing::warn!(claim_token = %token, "claim mint blocked: quiz not passed");
                return Err(AppError::Validation(
                    "you must pass the quiz before claiming your badge".into(),
                ));
            }
        }
    }

    // Check adventure gate
    if let Some(adv_status) = adv_result {
        match adv_status {
            AdventureStatus::NotRequired | AdventureStatus::Passed => {}
            AdventureStatus::NotStarted => {
                tracing::warn!(claim_token = %token, "claim mint blocked: adventure not attempted");
                return Err(AppError::Validation(
                    "you must complete the Rust Adventure before claiming your badge".into(),
                ));
            }
            AdventureStatus::InProgress => {
                tracing::warn!(claim_token = %token, "claim mint blocked: adventure not passed");
                return Err(AppError::Validation(
                    "you must complete the Rust Adventure before claiming your badge".into(),
                ));
            }
        }
    }

    // 6. Must not be already claimed
    if attendee.claimed_at.is_some() {
        let claimed_at = attendee.claimed_at.as_deref().unwrap_or("unknown");
        tracing::warn!(claim_token = %token, claimed_at = %claimed_at, "claim already fulfilled");
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
                    claim_token = %token,
                    registered = %mask_wallet(registered),
                    claiming = %mask_wallet(claiming),
                    "wallet mismatch"
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
            tracing::error!(claim_token = %token, error = %e, "mint failed");
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

    // 10. Mark as claimed in Google Sheet
    let claimed_at = Utc::now().to_rfc3339();

    // Resolve column mapping
    let mapping = match crate::sheets::get_column_mapping(
        state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
    )
    .await
    {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "column mapping fallback to hardcoded");
            event_checkin_domain::models::attendee::ColumnMapping::hardcoded()
        }
    };

    if let Err(ref e) = crate::sheets::mark_claimed(
        attendee.row_index,
        wallet_address,
        &claimed_at,
        &mapping,
        state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
    )
    .await
    {
        tracing::error!(claim_token = %token, error = %e, "mint succeeded but failed to mark claimed");
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
        tracing::warn!(error = %e, "claim lock finalize failed (non-blocking)");
    }

    tracing::info!(
        claim_token = %token,
        name = %display_name,
        asset_id = %mint_result.asset_id,
        wallet_address = %wallet_address,
        "claim fulfilled"
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

// ---------------------------------------------------------------------------
// Walk-in claim logic (KV-only, no Google Sheet dependency)
// ---------------------------------------------------------------------------

/// Execute the claim flow for a walk-in attendee.
/// Walk-ins skip quiz/adventure gates and record claims in KV (not Sheets).
async fn execute_walkin_claim(
    state: &AppState,
    kv: &KvStore,
    event: &event_checkin_domain::models::event::EventConfig,
    token: &str,
    wallet_address: &str,
    walkin: WalkinAttendee,
) -> Result<ClaimResult, AppError> {
    let display_name = walkin.name.clone();

    // Already claimed check
    if walkin.claimed_at.is_some() {
        tracing::warn!(claim_token = %token, "walk-in already claimed");
        return Err(AppError::Validation("NFT has already been claimed".into()));
    }

    // Claim dedup lock
    if let Err(e) = acquire_claim_lock(kv, &event.id, token, wallet_address).await {
        return Err(AppError::RateLimited(e));
    }

    // Mint compressed NFT via Helius
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

    let mint_result = match crate::solana::mint_compressed_nft(&mint_req).await {
        Ok(result) => result,
        Err(ref e) => {
            tracing::error!(claim_token = %token, error = %e, "walk-in mint failed");
            let _ = release_claim_lock(kv, &event.id, token).await;
            return Err(AppError::External {
                service: "helius".into(),
                status: 502,
                body: e.to_string(),
            });
        }
    };

    // Mark as claimed in KV (not Google Sheet — walk-ins are KV-only)
    let claimed_at = Utc::now().to_rfc3339();
    if let Err(e) = mark_walkin_claimed(
        kv,
        &walkin.event_id,
        &walkin.email,
        wallet_address,
        &claimed_at,
    )
    .await
    {
        tracing::error!(claim_token = %token, error = %e, "walk-in mint succeeded but failed to mark claimed");
        return Err(AppError::Internal(format!(
            "NFT minted but failed to record claim. Asset ID: {}. Error: {e}",
            mint_result.asset_id
        )));
    }

    // Finalize claim lock
    if let Err(e) =
        finalize_claim_lock(kv, &event.id, token, wallet_address, &mint_result.asset_id).await
    {
        tracing::warn!(error = %e, "walk-in claim lock finalize failed (non-blocking)");
    }

    tracing::info!(
        claim_token = %token,
        name = %display_name,
        asset_id = %mint_result.asset_id,
        wallet_address = %wallet_address,
        "walk-in claim fulfilled"
    );

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
