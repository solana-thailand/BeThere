//! Lookup logic (GET /api/claim/{token}).

use event_checkin_domain::models::api::{EventConfig as ApiEventConfig, QuizStatus};
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::EventConfig;

use crate::handlers::ext::{resolve_event, resolve_kv};
use crate::state::AppState;

use super::helpers::{d1_claim_fallback, resolve_claim_context};
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
    // one this attendee registered for. The same D1 read also answers the
    // walk-in check and the D1 fallback below (plan 028 W6).
    let ctx = resolve_claim_context(state, token, event_id).await;
    let event = resolve_event(state, ctx.event_id.as_deref()).await?;
    let kv = resolve_kv(state);

    // ── Walk-in path: D1-only (walk-ins are stored in D1 as primary) ──
    if let Some(walkin) = ctx.walkin(&event.id) {
        tracing::info!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim lookup: found walk-in attendee");

        // Match the actual mint executor. Helius is used for reads, while NFT
        // minting requires Crossmint credentials, collection, and known host.
        let nft_available = state.config.solana.crossmint_minting_configured();

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
            // Response field only, and the one `api_id` in the codebase that
            // embeds a personal identifier: walk-ins have no attendee row, so
            // their claim response is keyed by address. Every other `api_id` is
            // an internal id, which is why eight log sites render it as
            // `attendee_id`. Never log this one (Issue 070).
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
                // Sheets returned nothing — use the D1 row (online attendees may
                // have claim_token in D1 but not yet synced to Sheets). The
                // event is already correctly resolved above, so this fallback
                // uses the attendee's real event. Counts are unavailable
                // without the event's rows; the claim page shows them as
                // informational only.
                tracing::info!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token), "claim lookup: Sheets miss, trying D1 fallback");
                (
                    d1_claim_fallback(state, token, ctx.d1, "lookup").await?,
                    0,
                    0,
                )
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

    // Match the actual mint executor. Helius is used for reads, while NFT
    // minting requires Crossmint credentials, collection, and known host.
    let nft_available = state.config.solana.crossmint_minting_configured();

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

    // Quiz status, the finalized claim lock and the linked profile wallet are
    // independent reads, so they run concurrently (plan 028 W6).
    let (
        quiz_status,
        (claimed_signature, claimed_asset_id, claimed_wallet, cluster),
        linked_wallet_display,
    ) = futures_util::join!(
        quiz_status_for(state, &event, token),
        claimed_explorer_fields(state, kv, &event.id, token, claimed),
        linked_wallet_display_for(state, &attendee.email, locked_wallet.is_none()),
    );

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

/// Quiz status (Issue 002 — activity-gated claim). If the event does NOT have
/// quiz enabled, skip quiz unconditionally.
async fn quiz_status_for(state: &AppState, event: &EventConfig, token: &str) -> QuizStatus {
    if !event.quiz_enabled {
        return QuizStatus::NotRequired;
    }
    let qs = crate::quiz::get_quiz_status(
        state.d1.as_deref(),
        state.events_kv.as_ref().or(state.quiz_kv.as_ref()),
        &event.id,
        token,
        event.quiz_enabled,
    )
    .await
    .unwrap_or(QuizStatus::NotRequired);

    // If quiz_enabled is true but no quiz config exists yet, treat as NotStarted
    // so the frontend shows the correct gate instead of letting the user claim.
    // The organizer must configure quiz questions before attendees can claim.
    match qs {
        QuizStatus::NotRequired => QuizStatus::NotStarted,
        other => other,
    }
}

type ExplorerFields = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Read the finalized claim lock KV for already-claimed attendees to retrieve
/// signature, asset_id, wallet and cluster for explorer links.
async fn claimed_explorer_fields(
    state: &AppState,
    kv: Option<&worker::KvStore>,
    event_id: &str,
    token: &str,
    claimed: bool,
) -> ExplorerFields {
    let none = (None, None, None, None);
    let Some(kv_ref) = kv.filter(|_| claimed) else {
        return none;
    };
    let lock_key = claim_lock_key(event_id, token);
    let Some(json_str) = kv_ref.get(&lock_key).text().await.ok().flatten() else {
        return none;
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) else {
        return none;
    };
    let cluster = match state.config.solana.rpc_url.contains("mainnet") {
        true => "mainnet-beta",
        false => "devnet",
    };
    let field = |name: &str| val.get(name).and_then(|v| v.as_str()).map(String::from);
    (
        field("signature"),
        field("asset_id"),
        field("wallet"),
        Some(cluster.to_string()),
    )
}

/// When there's no per-event lock, signal that the attendee has a verified
/// profile-bound wallet (via the SIWS bind flow) so the claim page can offer a
/// one-tap "mint to my linked wallet" path. We expose ONLY a masked display —
/// the full address stays server-side and the mint resolves it by email, so a
/// leaked claim link can neither read the wallet nor redirect the badge.
async fn linked_wallet_display_for(
    state: &AppState,
    email: &str,
    unlocked: bool,
) -> Option<String> {
    let db = state.d1.as_deref().filter(|_| unlocked)?;
    crate::db::developers::get_developer_profile(db, email)
        .await
        .ok()
        .flatten()
        .and_then(|p| p.wallet_address)
        .map(|w| w.trim().to_string())
        .filter(|w| !w.is_empty())
        .map(|w| mask_wallet(&w))
}
