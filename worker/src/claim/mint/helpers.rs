//! Internal helpers: event-id coalescing and NFT URL rewriting.

use crate::state::AppState;

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
pub(super) async fn resolve_event_id_from_token(state: &AppState, token: &str) -> Option<String> {
    let d1 = state.d1.as_ref()?;
    match crate::db::attendees::get_attendee_event_id_by_claim_token(d1, token).await {
        Ok(Some(id)) => {
            tracing::info!(
                claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
                resolved_event_id = %id,
                "claim: resolved event_id from D1 by claim token"
            );
            Some(id)
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(
                claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(token),
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
pub(super) fn crossmint_image_url(url: &str) -> String {
    match url.strip_suffix(".svg") {
        Some(stem) => format!("{stem}.png"),
        None => url.to_string(),
    }
}

/// Build an Orb Markets explorer URL for the claimed NFT.
pub(super) fn orb_nft_url(asset_id: &str, cluster: &str) -> String {
    let cluster_param = if cluster == "mainnet-beta" {
        "?cluster=mainnet"
    } else {
        "?cluster=devnet"
    };
    format!("https://orbmarkets.io/token/{asset_id}/metadata{cluster_param}")
}
