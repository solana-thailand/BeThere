//! NFT minting orchestration: claim lookup, execution, and walk-in claims.
//!
//! Contains the core business logic for looking up claim status and executing
//! the full claim flow (validate → gates → lock → mint → record).

use chrono::Utc;
use worker::KvStore;

use event_checkin_domain::models::adventure::AdventureStatus;
use event_checkin_domain::models::api::{EventConfig as ApiEventConfig, QuizStatus};
use event_checkin_domain::models::attendee::WalkinAttendee;
use event_checkin_domain::models::error::AppError;

use crate::handlers::ext::{resolve_event, resolve_kv};
use crate::solana::{self, MintRequest};
use crate::state::AppState;

use super::lock::{
    FinalizeClaimLockParams, acquire_claim_lock, claim_lock_key, finalize_claim_lock, mask_wallet,
    release_claim_lock,
};

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
    /// Per-event pre-registered wallet (Sheet column P). When set, the claim is
    /// LOCKED to this address (often the on-chain depositor).
    pub locked_wallet: Option<String>,
    /// Masked display (e.g. `7Xk9…Qm3p`) of the attendee's profile-bound wallet
    /// (developer_profiles.wallet_address), surfaced only when there is no
    /// per-event lock. The full address is NEVER sent to the client — the mint
    /// resolves it server-side by email. Presence signals "one-tap linked-wallet
    /// claim is available"; the attendee may still opt to use a different wallet.
    pub linked_wallet_display: Option<String>,
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
    /// Transaction signature from the finalized claim lock KV (if available).
    pub claimed_signature: Option<String>,
    /// Asset ID from the finalized claim lock KV (if available).
    pub claimed_asset_id: Option<String>,
    /// Wallet address from the finalized claim lock KV (if available).
    pub claimed_wallet: Option<String>,
    /// Solana cluster for explorer links (e.g. "devnet", "mainnet-beta").
    pub cluster: Option<String>,
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
// Internal helpers
// ---------------------------------------------------------------------------

/// Resolve the correct `event_id` for a claim token when the caller did not
/// provide one.
///
/// Public claim URLs (`/claim/{token}`) carry no event context. Without this
/// lookup, `resolve_event(None)` falls back to the "first active event" —
/// which may NOT be the attendee's event, causing the claim page to show the
/// wrong event's name, NFT image, quiz, and deposit config, and (worse) the
/// mint POST to potentially target the wrong collection/sheet.
///
/// Returns `Some(event_id)` when the token maps to a known attendee in D1;
/// `None` when not found or D1 is unavailable (caller then falls back to the
/// active-event default).
async fn resolve_event_id_from_token(state: &AppState, token: &str) -> Option<String> {
    let d1 = state.d1.as_ref()?;
    match crate::db::attendees::get_attendee_event_id_by_claim_token(d1, token).await {
        Ok(Some(id)) => {
            tracing::info!(
                claim_token = %token,
                resolved_event_id = %id,
                "claim: resolved event_id from D1 by claim token"
            );
            Some(id)
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(
                claim_token = %token,
                error = %e,
                "claim: could not peek event_id from D1, falling back to active event"
            );
            None
        }
    }
}

/// Coalesce the caller-provided `event_id` with a D1-derived one.
///
/// If the caller passed a non-empty event_id, it wins (explicit context).
/// Otherwise we try to recover the attendee's real event_id from D1.
///
/// Exposed so the token-bearing quiz endpoints resolve the SAME authoritative
/// event_id the claim gate uses — otherwise quiz progress can be written under
/// the active-event fallback and the claim gate (which coalesces from the token)
/// never finds it.
pub(crate) async fn coalesce_event_id(
    state: &AppState,
    token: &str,
    event_id: Option<&str>,
) -> Option<String> {
    if let Some(id) = event_id.filter(|s| !s.is_empty()) {
        return Some(id.to_string());
    }
    resolve_event_id_from_token(state, token).await
}

/// Rewrite a badge image URL to a Crossmint-safe raster form.
///
/// Crossmint (and other minters) reject SVG image URLs. Our badge SVGs are
/// served with PNG twins at the same path (`/api/badge-hd.svg` →
/// `/api/badge-hd.png`), so swapping the extension yields a supported image.
/// Non-SVG URLs pass through unchanged.
fn crossmint_image_url(url: &str) -> String {
    match url.strip_suffix(".svg") {
        Some(stem) => format!("{stem}.png"),
        None => url.to_string(),
    }
}

/// Build an Orb Markets explorer URL for the claimed NFT.
fn orb_nft_url(asset_id: &str, cluster: &str) -> String {
    let cluster_param = if cluster == "mainnet-beta" {
        "?cluster=mainnet"
    } else {
        "?cluster=devnet"
    };
    format!("https://orbmarkets.io/token/{asset_id}/metadata{cluster_param}")
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

    // Resolve the correct event BEFORE any other logic. The public claim URL
    // `/claim/{token}` carries no event_id; without this coalesce, the fallback
    // picks the "first active event", which may be a different event than the
    // one this attendee registered for.
    let resolved_event_id = coalesce_event_id(state, token, event_id).await;
    let event = resolve_event(state, resolved_event_id.as_deref()).await?;
    let kv = resolve_kv(state);

    // ── Walk-in path: D1-only (walk-ins are stored in D1 as primary) ──
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
        tracing::info!(claim_token = %token, email = %walkin.email, "claim lookup: found walk-in attendee");

        // API key + image URL are required; metadata_uri/collection_mint are optional
        // enhancements passed to Helius when set.
        let nft_available =
            !event.nft_image_url.is_empty() && !state.config.solana.api_key.is_empty();

        return Ok(ClaimLookup {
            name: walkin.name.clone(),
            checked_in_at: walkin.checked_in_at.clone(),
            claim_token: token.to_string(),
            claimed: walkin.claimed_at.is_some(),
            claimed_at: walkin.claimed_at.clone(),
            nft_available,
            locked_wallet: walkin.wallet_address.clone(),
            linked_wallet_display: None, // walk-ins have no developer profile
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
            claimed_signature: None,
            claimed_asset_id: None,
            claimed_wallet: None,
            cluster: None,
        });
    }

    // ── Pre-registered path: look up from Google Sheet, D1 fallback ──
    let (attendee, total_checked_in, total_claimed) =
        match crate::sheets::get_attendee_with_claim_counts(
            token,
            state,
            &event.sheet_id,
            &event.sheet_name,
            kv,
            Some(&event.id),
        )
        .await
        {
            Ok((Some(a), checked_in, claimed)) => (a, checked_in, claimed),
            Ok((None, _, _)) => {
                // Sheets returned nothing — try D1 fallback (online attendees may
                // have claim_token in D1 but not yet synced to Sheets). The
                // event is already correctly resolved above via coalesce_event_id,
                // so this fallback uses the attendee's real event.
                tracing::info!(claim_token = %token, "claim lookup: Sheets miss, trying D1 fallback");
                if let Some(ref d1) = state.d1 {
                    match crate::db::attendees::get_attendee_by_claim_token(d1, token).await {
                        Ok(Some(a)) => {
                            tracing::info!(claim_token = %token, "claim lookup: found in D1 fallback");
                            // Counts unavailable without event_id; claim page shows them as informational only
                            (a, 0, 0)
                        }
                        Ok(None) => {
                            tracing::warn!(claim_token = %token, "claim lookup: not found in Sheets or D1");
                            return Err(AppError::NotFound("claim token not found".into()));
                        }
                        Err(e) => {
                            tracing::error!(claim_token = %token, error = %e, "claim lookup D1 fallback failed");
                            return Err(AppError::NotFound("claim token not found".into()));
                        }
                    }
                } else {
                    tracing::warn!(claim_token = %token, "claim lookup: no attendee found (no D1)");
                    return Err(AppError::NotFound("claim token not found".into()));
                }
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

    // API key + image URL are required; metadata_uri/collection_mint are optional
    // enhancements passed to Helius when set.
    let nft_available = !event.nft_image_url.is_empty() && !state.config.solana.api_key.is_empty();

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
    let quiz_status = crate::quiz::get_quiz_status(
        state.d1.as_deref(),
        state.events_kv.as_ref().or(state.quiz_kv.as_ref()),
        &event.id,
        token,
    )
    .await
    .unwrap_or(QuizStatus::NotRequired);

    // If quiz_enabled is true but no quiz config exists yet, treat as NotStarted
    // so the frontend shows the correct gate instead of letting the user claim.
    // The organizer must configure quiz questions before attendees can claim.
    let quiz_status = if event.quiz_enabled && quiz_status == QuizStatus::NotRequired {
        QuizStatus::NotStarted
    } else {
        quiz_status
    };

    // Read finalized claim lock KV for already-claimed attendees
    // to retrieve signature, asset_id, wallet for explorer links
    let (claimed_signature, claimed_asset_id, claimed_wallet, cluster) = if claimed {
        let lock_key = claim_lock_key(&event.id, token);
        let lock_data: Option<String> = if let Some(kv_ref) = kv {
            kv_ref.get(&lock_key).text().await.ok().flatten()
        } else {
            None
        };
        if let Some(json_str) = lock_data {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                let cluster_val = if state.config.solana.rpc_url.contains("mainnet") {
                    "mainnet-beta".to_string()
                } else {
                    "devnet".to_string()
                };
                (
                    val.get("signature")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    val.get("asset_id")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    val.get("wallet").and_then(|v| v.as_str()).map(String::from),
                    Some(cluster_val),
                )
            } else {
                (None, None, None, None)
            }
        } else {
            (None, None, None, None)
        }
    } else {
        (None, None, None, None)
    };

    // When there's no per-event lock, signal that the attendee has a verified
    // profile-bound wallet (via the SIWS bind flow) so the claim page can offer a
    // one-tap "mint to my linked wallet" path. We expose ONLY a masked display —
    // the full address stays server-side and the mint resolves it by email, so a
    // leaked claim link can neither read the wallet nor redirect the badge.
    let linked_wallet_display = if locked_wallet.is_none() {
        match state.d1.as_deref() {
            Some(db) => crate::db::developers::get_developer_profile(db, &attendee.email)
                .await
                .ok()
                .flatten()
                .and_then(|p| p.wallet_address)
                .map(|w| w.trim().to_string())
                .filter(|w| !w.is_empty())
                .map(|w| mask_wallet(&w)),
            None => None,
        }
    } else {
        None
    };

    Ok(ClaimLookup {
        name: display_name,
        checked_in_at,
        claim_token: token.to_string(),
        claimed,
        claimed_at,
        nft_available,
        locked_wallet,
        linked_wallet_display,
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
        claimed_signature,
        claimed_asset_id,
        claimed_wallet,
        cluster,
    })
}

// ---------------------------------------------------------------------------
// Execute claim logic (POST /api/claim/{token})
// ---------------------------------------------------------------------------

/// Check if an online attendee has completed the required quest (quiz or adventure).
/// Returns true if at least one is passed, or if neither is required.
///
/// When `quiz_enabled = true` but no quiz config exists in D1/KV yet, returns `false`
/// to prevent claiming before the organizer finishes setting up the quiz.
async fn verify_online_quest_completion(
    state: &AppState,
    event_id: &str,
    claim_token: &str,
    quiz_enabled: bool,
) -> bool {
    let d1 = state.d1.as_deref();
    let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());

    // Check quiz status first
    match crate::quiz::get_quiz_status(d1, kv, event_id, claim_token).await {
        Ok(QuizStatus::Passed) => true,
        Ok(QuizStatus::NotRequired) => {
            // Quiz not configured — check adventure (D1 only)
            let Some(db) = d1 else {
                // No D1 available — treat adventure as not required
                return !quiz_enabled;
            };
            match crate::adventure::get_adventure_status(db, event_id, claim_token).await {
                Ok(AdventureStatus::Passed) => true,
                Ok(AdventureStatus::NotRequired) => {
                    // Neither quiz nor adventure configured.
                    // If quiz_enabled is true, the organizer intends a quest but
                    // hasn't set it up yet — block claiming.
                    !quiz_enabled
                }
                _ => false,
            }
        }
        _ => false,
    }
}

pub async fn execute_claim(
    state: &AppState,
    token: &str,
    requested_wallet: Option<&str>,
    use_linked: bool,
    event_id: Option<&str>,
) -> Result<ClaimResult, AppError> {
    tracing::info!(claim_token = %token, use_linked, "claim mint request");

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
            tracing::info!(claim_token = %token, "claim mint: Sheets miss, trying D1 fallback");
            if let Some(ref d1) = state.d1 {
                match crate::db::attendees::get_attendee_by_claim_token(d1, token).await {
                    Ok(Some(a)) => {
                        tracing::info!(claim_token = %token, "claim mint: found in D1 fallback");
                        a
                    }
                    Ok(None) => {
                        tracing::warn!(claim_token = %token, "claim mint: not found in Sheets or D1");
                        return Err(AppError::NotFound("claim token not found".into()));
                    }
                    Err(e) => {
                        tracing::error!(claim_token = %token, error = %e, "claim mint D1 fallback failed");
                        return Err(AppError::NotFound("claim token not found".into()));
                    }
                }
            } else {
                tracing::warn!(claim_token = %token, "claim mint: no attendee found (no D1)");
                return Err(AppError::NotFound("claim token not found".into()));
            }
        }
        Err(ref e) => {
            tracing::error!(claim_token = %token, error = %e, "claim mint lookup failed");
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
                claim_token = %token,
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
                // Auto virtual check-in — generate timestamp locally, detach Sheets write
                let virtual_ts = chrono::Utc::now().to_rfc3339();
                tracing::info!(
                    claim_token = %token,
                    attendee_id = %attendee.api_id,
                    checked_in_at = %virtual_ts,
                    "virtual check-in auto-completed for online attendee"
                );
                attendee.checked_in_at = Some(virtual_ts.clone());

                // Detach Sheets write
                if let Some(ctx) = &state.worker_ctx {
                    ctx.wait_until(crate::sheets::bg_sync::mark_virtual_checked_in(
                        state.clone(),
                        attendee.row_index,
                        mapping.clone(),
                        event.sheet_id.clone(),
                        event.sheet_name.clone(),
                        kv.cloned(),
                        virtual_ts,
                    ));
                } else if let Err(e) = crate::sheets::write::mark_virtual_checked_in(
                    attendee.row_index,
                    &mapping,
                    state,
                    &event.sheet_id,
                    &event.sheet_name,
                    kv,
                )
                .await
                {
                    tracing::error!(claim_token = %token, error = %e, "virtual check-in sheet write failed (non-fatal)");
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
                    tracing::warn!(claim_token = %token, "claim mint blocked: quiz enabled but not configured");
                    return Err(AppError::Validation(
                        "quiz is being set up — please try again later".into(),
                    ));
                }
            }
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
                claim_token = %token,
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
        tracing::warn!(claim_token = %token, error = %e, "resolved recipient wallet invalid");
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
            tracing::error!(claim_token = %token, error = %e, "mint failed");
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
            claim_token = %token,
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
        tracing::error!(claim_token = %token, error = %e, "Sheets mark_claimed failed (non-fatal)");
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
        claim_token = %token,
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

// ---------------------------------------------------------------------------
// Walk-in claim logic (D1 + KV best-effort)
// ---------------------------------------------------------------------------

/// Execute the claim flow for a walk-in attendee.
/// Walk-ins skip quiz/adventure gates. Claims are recorded in D1 (primary)
/// with KV best-effort mirror for legacy compatibility.
async fn execute_walkin_claim(
    state: &AppState,
    event: &event_checkin_domain::models::event::EventConfig,
    token: &str,
    requested_wallet: Option<&str>,
    walkin: WalkinAttendee,
) -> Result<ClaimResult, AppError> {
    let display_name = walkin.name.clone();
    let kv = resolve_kv(state);

    // Walk-ins have no developer profile, so there's no linked wallet to mint to:
    // an explicit wallet address is required and validated server-side.
    let recipient = match requested_wallet.map(str::trim).filter(|w| !w.is_empty()) {
        Some(w) => w.to_string(),
        None => return Err(AppError::Validation("a wallet address is required".into())),
    };
    if let Err(e) = crate::solana::validate_wallet_address(&recipient) {
        tracing::warn!(claim_token = %token, error = %e, "walk-in recipient wallet invalid");
        return Err(AppError::Validation(e));
    }
    let wallet_address: &str = &recipient;

    // Note: walk-ins are always in-person — no online claim timing gate needed

    // Already claimed check
    if walkin.claimed_at.is_some() {
        tracing::warn!(claim_token = %token, "walk-in already claimed");
        return Err(AppError::Validation("NFT has already been claimed".into()));
    }

    // Claim dedup lock
    if let Some(kv) = kv
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

    // Mint compressed NFT via Crossmint (custodial signer + tree + fees)
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

    let mint_result = match crate::solana::mint_compressed_nft(&mint_req, kv).await {
        Ok(result) => result,
        Err(ref e) => {
            tracing::error!(claim_token = %token, error = %e, "walk-in mint failed");
            if let Some(kv) = kv {
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
                service: "helius".into(),
                status: 502,
                body: e.to_string(),
            });
        }
    };

    // Mark as claimed in D1 (primary)
    let claimed_at = Utc::now().to_rfc3339();
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
        tracing::error!(
            claim_token = %token,
            error = %e,
            "walk-in D1 claim write failed (mint succeeded, data may be inconsistent)"
        );
        // Don't fail the response — the NFT was already minted.
    }

    // Finalize claim lock
    if let Some(kv) = kv
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
        tracing::warn!(error = %e, "walk-in claim lock finalize failed");
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── crossmint_image_url: SVG→PNG rewrite for the mint payload ──
    #[test]
    fn image_url_rewrites_hd_svg_to_png() {
        assert_eq!(
            crossmint_image_url("https://x.dev/api/badge-hd.svg"),
            "https://x.dev/api/badge-hd.png"
        );
    }

    #[test]
    fn image_url_rewrites_plain_svg_to_png() {
        assert_eq!(crossmint_image_url("/api/badge.svg"), "/api/badge.png");
    }

    #[test]
    fn image_url_passes_through_png() {
        assert_eq!(crossmint_image_url("https://x.dev/a.png"), "https://x.dev/a.png");
    }

    #[test]
    fn image_url_passes_through_empty_and_non_svg() {
        assert_eq!(crossmint_image_url(""), "");
        assert_eq!(crossmint_image_url("https://x.dev/img"), "https://x.dev/img");
        // Only a trailing .svg is rewritten — a mid-string ".svg" is untouched.
        assert_eq!(crossmint_image_url("https://x.dev/a.svg.jpg"), "https://x.dev/a.svg.jpg");
    }

    // ── orb_nft_url: explorer link cluster mapping ──
    #[test]
    fn orb_url_mainnet_uses_mainnet_param() {
        let u = orb_nft_url("AID", "mainnet-beta");
        assert!(u.contains("AID"));
        assert!(u.contains("cluster=mainnet"));
        assert!(!u.contains("cluster=devnet"));
    }

    #[test]
    fn orb_url_devnet_default() {
        assert!(orb_nft_url("AID", "devnet").contains("cluster=devnet"));
    }
}
