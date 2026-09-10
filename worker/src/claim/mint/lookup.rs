//! Lookup logic (GET /api/claim/{token}).

use event_checkin_domain::models::api::{EventConfig as ApiEventConfig, QuizStatus};
use event_checkin_domain::models::attendee::WalkinAttendee;
use event_checkin_domain::models::error::AppError;

use crate::handlers::ext::{resolve_event, resolve_kv};
use crate::state::AppState;

use super::helpers::coalesce_event_id;
use super::types::ClaimLookup;
use crate::claim::lock::{claim_lock_key, mask_wallet};

/// Look up claim status by token. Returns attendee info, event config, quiz/adventure status.
pub async fn lookup_claim(
    state: &AppState,
    token: &str,
    event_id: Option<&str>,
) -> Result<ClaimLookup, AppError> {
    tracing::info!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim lookup");

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
        tracing::info!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), email = %walkin.email, "claim lookup: found walk-in attendee");

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
                tracing::info!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim lookup: Sheets miss, trying D1 fallback");
                if let Some(ref d1) = state.d1 {
                    match crate::db::attendees::get_attendee_by_claim_token(d1, token).await {
                        Ok(Some(a)) => {
                            tracing::info!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim lookup: found in D1 fallback");
                            // Counts unavailable without event_id; claim page shows them as informational only
                            (a, 0, 0)
                        }
                        Ok(None) => {
                            tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim lookup: not found in Sheets or D1");
                            return Err(AppError::NotFound("claim token not found".into()));
                        }
                        Err(e) => {
                            tracing::error!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), error = %e, "claim lookup D1 fallback failed");
                            return Err(AppError::NotFound("claim token not found".into()));
                        }
                    }
                } else {
                    tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim lookup: no attendee found (no D1)");
                    return Err(AppError::NotFound("claim token not found".into()));
                }
            }
            Err(ref e) => {
                tracing::error!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), error = %e, "claim lookup failed");
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
