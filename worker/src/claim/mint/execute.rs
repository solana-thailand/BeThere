//! Execute claim logic (POST /api/claim/{token}).

use chrono::Utc;
use worker::KvStore;

use event_checkin_domain::models::adventure::AdventureStatus;
use event_checkin_domain::models::api::QuizStatus;
use event_checkin_domain::models::attendee::WalkinAttendee;
use event_checkin_domain::models::error::AppError;

use crate::handlers::ext::{resolve_event, resolve_kv};
use crate::solana::{self, MintRequest};
use crate::state::AppState;

use super::helpers::{coalesce_event_id, crossmint_image_url, orb_nft_url};
use super::quest::verify_online_quest_completion;
use super::types::ClaimResult;
use super::walkin::execute_walkin_claim;
use crate::claim::lock::{
    FinalizeClaimLockParams, acquire_claim_lock, finalize_claim_lock, mask_wallet,
    release_claim_lock,
};

pub async fn execute_claim(
    state: &AppState,
    token: &str,
    requested_wallet: Option<&str>,
    use_linked: bool,
    event_id: Option<&str>,
) -> Result<ClaimResult, AppError> {
    tracing::info!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), use_linked, "claim mint request");

    // 1. Resolve event context. Same coalesce as lookup_claim: the public POST
    //    `/claim/{token}` carries no event_id, so recover the attendee's real
    //    event from D1 before minting — otherwise we could mint against the
    //    wrong event's collection/sheet.
    let resolved_event_id = coalesce_event_id(state, token, event_id).await;
    let event = resolve_event(state, resolved_event_id.as_deref()).await?;
    let kv = resolve_kv(state);

    // 2. Check walk-in path: D1-only
    let mut walkin: Option<WalkinAttendee> = None;
    if let Some(ref d1) = state.d1
        && let Ok(Some(a)) = crate::db::attendees::get_attendee_by_claim_token(d1, token).await
        && a.participation_type == "walkin"
    {
        walkin = Some(WalkinAttendee {
            event_id: event.id.clone(),
            email: a.email.clone(),
            name: a.name.clone(),
            phone: None,
            claim_token: a.claim_token.clone().unwrap_or_default(),
            checked_in_at: a.checked_in_at.clone().unwrap_or_default(),
            checked_in_by: a.checked_in_by.clone().unwrap_or_default(),
            wallet_address: None,
            claimed_at: a.claimed_at.clone(),
        });
    }
    if let Some(walkin) = walkin {
        return execute_walkin_claim(state, &event, token, requested_wallet, walkin).await;
    }

    // 3. Pre-registered path: look up attendee by claim token from Google Sheet, D1 fallback
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
            // Sheets returned nothing — try D1 fallback
            tracing::info!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim mint: Sheets miss, trying D1 fallback");
            if let Some(ref d1) = state.d1 {
                match crate::db::attendees::get_attendee_by_claim_token(d1, token).await {
                    Ok(Some(a)) => {
                        tracing::info!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim mint: found in D1 fallback");
                        a
                    }
                    Ok(None) => {
                        tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim mint: not found in Sheets or D1");
                        return Err(AppError::NotFound("claim token not found".into()));
                    }
                    Err(e) => {
                        tracing::error!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), error = %e, "claim mint D1 fallback failed");
                        return Err(AppError::NotFound("claim token not found".into()));
                    }
                }
            } else {
                tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim mint: no attendee found (no D1)");
                return Err(AppError::NotFound("claim token not found".into()));
            }
        }
        Err(ref e) => {
            tracing::error!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), error = %e, "claim mint lookup failed");
            return Err(AppError::Internal(format!("failed to look up claim: {e}")));
        }
    };

    let display_name = attendee.display_name().to_string();

    // Resolve column mapping once — reused for virtual check-in and mark_claimed (C2)
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

    // 2b. Online claim timing gate — online attendees can only claim after event ends
    // This prevents online attendees from completing everything before the event occurs.
    let is_online_attendee = !attendee.is_in_person();
    if is_online_attendee {
        let now_ms = chrono::Utc::now().timestamp_millis();
        if event.event_end_ms > 0 && now_ms < event.event_end_ms {
            let remaining_secs = (event.event_end_ms - now_ms) / 1000;
            let remaining_hours = remaining_secs / 3600;
            let remaining_mins = (remaining_secs % 3600) / 60;
            tracing::warn!(
                claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
                participation_type = %attendee.participation_type,
                event_end_ms = event.event_end_ms,
                now_ms = now_ms,
                remaining_hours = remaining_hours,
                "online claim blocked: event has not ended yet"
            );
            return Err(AppError::Validation(format!(
                "Online claims open after the event ends. {}h {}m remaining.",
                remaining_hours, remaining_mins
            )));
        }
    }

    // 3. Check-in verification — with virtual check-in for online attendees
    if attendee.checked_in_at.is_none() {
        let is_online_attendee = !attendee.is_in_person();
        if is_online_attendee && event.event_format.has_online() {
            // Verify quiz/adventure completion (at least one must be passed)
            let quest_passed =
                verify_online_quest_completion(state, &event.id, token, event.quiz_enabled).await;
            if quest_passed {
                // Single writer of the virtual check-in (plan 022 §2). It runs the
                // approval gate, which the inline copy this replaced did not — the
                // claim path used to read a set `checked_in_at` as proof that an
                // approval-gated path produced it. It also writes D1, which the
                // inline copy did not: `my-registration` reads D1-first, so a
                // Sheets-only write left the frontend showing "not checked in".
                let virtual_ts = crate::virtual_checkin::commit_virtual_check_in(
                    state, &attendee, &event, &mapping, kv, token,
                )
                .await?;
                tracing::info!(
                    claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
                    attendee_id = %attendee.api_id,
                    checked_in_at = %virtual_ts,
                    "virtual check-in auto-completed for online attendee"
                );
                attendee.checked_in_at = Some(virtual_ts);
            } else {
                tracing::warn!(
                    claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
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

    // 4+5. Quiz and Adventure gates — run concurrently to parallelize reads
    let d1_ref = state.d1.as_deref();
    let quiz_kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());

    let quiz_fut = async {
        crate::quiz::get_quiz_status(d1_ref, quiz_kv, &event.id, token)
            .await
            .ok()
    };
    let adv_fut = async {
        match d1_ref {
            Some(db) => crate::adventure::get_adventure_status(db, &event.id, token)
                .await
                .ok(),
            None => None,
        }
    };

    let (quiz_result, adv_result) = futures_util::join!(quiz_fut, adv_fut);

    // Check quiz gate
    if let Some(quiz_status) = quiz_result {
        match quiz_status {
            QuizStatus::Passed => {}
            QuizStatus::NotRequired => {
                // Quiz not configured — if quiz_enabled is true, the organizer
                // intends a quiz but hasn't set it up yet. Block the claim.
                if event.quiz_enabled {
                    tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim mint blocked: quiz enabled but not configured");
                    return Err(AppError::Validation(
                        "quiz is being set up — please try again later".into(),
                    ));
                }
            }
            QuizStatus::NotStarted => {
                tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim mint blocked: quiz not attempted");
                return Err(AppError::Validation(
                    "you must complete the quiz before claiming your badge".into(),
                ));
            }
            QuizStatus::InProgress => {
                tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim mint blocked: quiz not passed");
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
                tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim mint blocked: adventure not attempted");
                return Err(AppError::Validation(
                    "you must complete the Rust Adventure before claiming your badge".into(),
                ));
            }
            AdventureStatus::InProgress => {
                tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim mint blocked: adventure not passed");
                return Err(AppError::Validation(
                    "you must complete the Rust Adventure before claiming your badge".into(),
                ));
            }
        }
    }

    // 6. Must not be already claimed
    if attendee.claimed_at.is_some() {
        let claimed_at = attendee.claimed_at.as_deref().unwrap_or("unknown");
        tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), claimed_at = %claimed_at, "claim already fulfilled");
        return Err(AppError::Validation("NFT has already been claimed".into()));
    }

    // 7. Resolve the recipient wallet SERVER-SIDE. The client never dictates the
    //    mint destination for the locked or linked cases — it can only supply an
    //    explicit override wallet, which is the sole case a client address is used.
    //    Precedence: pre-registered (column P, authoritative) > verified linked
    //    profile wallet > explicit override.
    let requested = requested_wallet.map(str::trim).filter(|w| !w.is_empty());
    let recipient: String = if let Some(registered) = attendee
        .solana_address
        .as_deref()
        .map(str::trim)
        .filter(|w| !w.is_empty())
    {
        // Pre-registered: the claim is locked to this wallet. Reject a mismatched
        // explicit request (prevents claim theft via leaked URLs).
        if let Some(req) = requested
            && !req.eq_ignore_ascii_case(registered)
        {
            tracing::warn!(
                claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
                registered = %mask_wallet(registered),
                claiming = %mask_wallet(req),
                "wallet mismatch"
            );
            return Err(AppError::Validation(format!(
                "This claim is locked to a pre-registered wallet ({})",
                mask_wallet(registered)
            )));
        }
        registered.to_string()
    } else if use_linked {
        // Mint to the attendee's verified profile wallet — looked up server-side
        // by email, never sent by (or trusted from) the client.
        let linked = match state.d1.as_deref() {
            Some(db) => crate::db::developers::get_developer_profile(db, &attendee.email)
                .await
                .ok()
                .flatten()
                .and_then(|p| p.wallet_address)
                .map(|w| w.trim().to_string())
                .filter(|w| !w.is_empty()),
            None => None,
        };
        match linked {
            Some(w) => w,
            None => {
                return Err(AppError::Validation(
                    "no linked wallet on your profile — connect a wallet to claim".into(),
                ));
            }
        }
    } else {
        // Explicit override — the only path where a client-supplied wallet is used.
        match requested {
            Some(w) => w.to_string(),
            None => {
                return Err(AppError::Validation("a wallet address is required".into()));
            }
        }
    };

    if let Err(e) = crate::solana::validate_wallet_address(&recipient) {
        tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), error = %e, "resolved recipient wallet invalid");
        return Err(AppError::Validation(e));
    }
    let wallet_address: &str = &recipient;

    // 8. Claim dedup lock — prevent concurrent double-claim
    let lock_kv: Option<&KvStore> = resolve_kv(state);
    if let Some(kv) = lock_kv
        && let Err(e) = acquire_claim_lock(
            kv,
            &event.id,
            token,
            wallet_address,
            state.d1.as_deref(),
            state.event_do.as_ref(),
        )
        .await
    {
        return Err(AppError::RateLimited(e));
    }

    // 9. Mint compressed NFT via Crossmint (custodial signer + tree + fees)
    let config = &state.config;
    let mint_image = crossmint_image_url(&event.nft_image_url);
    let mint_req = MintRequest {
        wallet_address,
        host: &config.solana.crossmint_host,
        api_key: &config.solana.crossmint_api_key,
        collection_id: &config.solana.crossmint_collection_id,
        image_url: &mint_image,
        nft_name: &event.nft_name(),
        nft_description: &event.nft_description(),
        nft_external_url: &event.link,
        compressed: true,
        idempotency_key: token,
    };
    let mint_result = match solana::mint_compressed_nft(&mint_req, lock_kv).await {
        Ok(result) => result,
        Err(ref e) => {
            tracing::error!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), error = %e, "mint failed");
            // Release lock so attendee can retry
            if let Some(kv) = lock_kv {
                let _ = release_claim_lock(
                    kv,
                    &event.id,
                    token,
                    state.d1.as_deref(),
                    state.event_do.as_ref(),
                )
                .await;
            }
            return Err(AppError::External {
                service: "crossmint".into(),
                status: 502,
                body: e.to_string(),
            });
        }
    };

    // 10. Mark as claimed — D1 first, then detach Sheets write (Phase 2c)
    let claimed_at = Utc::now().to_rfc3339();

    // Compute cluster for Orb explorer proof URL
    let cluster = if config.solana.rpc_url.contains("mainnet") {
        "mainnet-beta"
    } else {
        "devnet"
    };
    let nft_proof_url = orb_nft_url(&mint_result.asset_id, cluster);

    // Write to D1 first (source of truth)
    if let Some(ref d1) = state.d1
        && let Err(e) = crate::db::attendees::claim_attendee(
            d1,
            token,
            &claimed_at,
            &mint_result.asset_id,
            &mint_result.signature,
        )
        .await
    {
        tracing::warn!(
            claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
            error = %e,
            "D1 claim write failed (non-fatal)"
        );
    }

    // Detach Sheets write — non-blocking (Phase 2c)
    if let Some(ctx) = &state.worker_ctx {
        ctx.wait_until(crate::sheets::bg_sync::mark_claimed(
            state.clone(),
            attendee.row_index,
            wallet_address.to_string(),
            claimed_at.clone(),
            nft_proof_url.clone(),
            mapping,
            event.sheet_id.clone(),
            event.sheet_name.clone(),
            kv.cloned(),
        ));
    } else if let Err(e) = crate::sheets::write::mark_claimed(
        attendee.row_index,
        wallet_address,
        &claimed_at,
        &nft_proof_url,
        &mapping,
        state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
    )
    .await
    {
        tracing::error!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), error = %e, "Sheets mark_claimed failed (non-fatal)");
    }

    // 11. Finalize claim lock (permanent record, no TTL) — non-blocking
    if let Some(kv) = lock_kv
        && let Err(e) = finalize_claim_lock(
            kv,
            FinalizeClaimLockParams {
                event_id: &event.id,
                token,
                wallet: wallet_address,
                asset_id: &mint_result.asset_id,
                signature: &mint_result.signature,
            },
            state.d1.as_deref(),
            state.event_do.as_ref(),
        )
        .await
    {
        tracing::warn!(error = %e, "claim lock finalize failed (non-blocking)");
    }

    tracing::info!(
        claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
        name = %display_name,
        asset_id = %mint_result.asset_id,
        wallet_address = %wallet_address,
        "claim fulfilled"
    );

    // 12. Return result (cluster already computed above for proof URL)

    Ok(ClaimResult {
        name: display_name,
        asset_id: mint_result.asset_id,
        signature: mint_result.signature,
        wallet_address: wallet_address.to_string(),
        claimed_at,
        cluster: cluster.to_string(),
    })
}
