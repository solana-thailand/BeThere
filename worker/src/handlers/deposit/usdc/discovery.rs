//! On-chain discovery of a missing deposit TX signature, and the claim-binding guard.

use super::rpc::parse_signatures_for_address_response;
use crate::crypto::LogRedactor;

/// Discover the most recent deposit TX signature for an attendee on-chain.
///
/// Recovery path for the scenario where the deposit record exists with a
/// known `wallet_address` but no recorded `tx_signature` — e.g., the webhook
/// that normally records the signature was never called (network drop, worker
/// restart, frontend bug) but the on-chain deposit succeeded. This derives
/// the AttendeeDeposit PDA from `[b"deposit", escrow, attendee]` and queries
/// `getSignaturesForAddress` to find the deposit transaction.
///
/// Returns `Some(signature)` if a confirmed, error-free signature exists, or
/// `None` if the PDA has no transaction history or the RPC failed. The caller
/// is expected to feed the returned signature back through
/// [`verify_tx_with_signer`] to confirm the signer matches the expected
/// attendee wallet before marking the deposit as verified.
pub(crate) async fn discover_deposit_tx_on_chain(
    rpc_url: &str,
    escrow_address: &str,
    attendee_wallet: &str,
    redactor: LogRedactor<'_>,
) -> Option<String> {
    use crate::solana_escrow::{escrow_program_id, pubkey_from_base58, pubkey_to_base58};

    // Derive the AttendeeDeposit PDA: seeds = [b"deposit", escrow, attendee].
    let escrow_pubkey = match pubkey_from_base58(escrow_address) {
        Ok(pk) => pk,
        Err(e) => {
            tracing::warn!(error = %e, "invalid escrow_address for deposit discovery");
            return None;
        }
    };
    let attendee_pubkey = match pubkey_from_base58(attendee_wallet) {
        Ok(pk) => pk,
        Err(e) => {
            tracing::warn!(error = %e, "invalid attendee_wallet for deposit discovery");
            return None;
        }
    };
    let program_id = match pubkey_from_base58(escrow_program_id()) {
        Ok(pk) => pk,
        Err(e) => {
            tracing::error!(error = %e, "invalid escrow program id for active cluster");
            return None;
        }
    };

    let (deposit_pda, _) = match crate::solana_escrow::crypto::find_program_address(
        &[
            b"deposit",
            escrow_pubkey.as_slice(),
            attendee_pubkey.as_slice(),
        ],
        &program_id,
    )
    .await
    {
        Ok(pda) => pda,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "PDA derivation failed for AttendeeDeposit during discovery"
            );
            return None;
        }
    };

    let deposit_pda_b58 = pubkey_to_base58(&deposit_pda);

    // Issue 070: the AttendeeDeposit PDA is derived from the attendee wallet,
    // so publishing it raw re-exposes the wallet to anyone reading the log.
    // The escrow address is event-level rather than personal, so it stays
    // readable — it is what makes these lines diagnosable at all.
    let deposit_pda_fingerprint = redactor.fingerprint(&deposit_pda_b58);

    tracing::debug!(
        deposit_pda_fingerprint = %deposit_pda_fingerprint,
        escrow = %escrow_address,
        attendee_fingerprint = %redactor.fingerprint(attendee_wallet),
        "Querying getSignaturesForAddress for AttendeeDeposit PDA"
    );

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-discover",
        "method": "getSignaturesForAddress",
        "params": [
            deposit_pda_b58,
            { "limit": 5 }
        ]
    });

    let json_body = match serde_json::to_string(&body) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "failed to serialize getSignaturesForAddress request");
            return None;
        }
    };

    let headers = worker::Headers::new();
    if let Err(e) = headers.set("Content-Type", "application/json") {
        tracing::warn!(error = ?e, "failed to set header");
        return None;
    }

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = match worker::Request::new_with_init(rpc_url, &init) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = ?e, "failed to create RPC request");
            return None;
        }
    };

    let mut response = match worker::Fetch::Request(request).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = ?e, "RPC getSignaturesForAddress request failed");
            return None;
        }
    };

    let body_text = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = ?e, "failed to read RPC response");
            return None;
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&body_text) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "failed to parse RPC response");
            return None;
        }
    };

    let signature = parse_signatures_for_address_response(&parsed);
    if let Some(ref sig) = signature {
        tracing::info!(
            tx_signature_fingerprint = %redactor.fingerprint(sig),
            deposit_pda_fingerprint = %deposit_pda_fingerprint,
            "Discovered deposit TX signature on-chain via PDA history"
        );
    } else {
        tracing::debug!(
            deposit_pda_fingerprint = %deposit_pda_fingerprint,
            "No deposit signatures found for AttendeeDeposit PDA"
        );
    }
    signature
}

/// Cooldown (in seconds) between on-chain deposit discovery attempts on public
/// read paths. Prevents a malicious caller from triggering unbounded
/// `getSignaturesForAddress` RPC calls by hammering `/public/ticket` for an
/// attendee whose deposit has no on-chain TX yet. 5 minutes balances abuse
/// protection with a reasonable retry window for legitimate deposits.
pub(super) const DISCOVERY_COOLDOWN_SECS: u64 = 300;

/// Pure decision helper for the claim-binding guard (plan 003).
///
/// Returns `true` iff either `wallet_owner` or `tx_owner` is bound to an id
/// *different* from `current_attendee_id`. Both `Some(current_attendee_id)`
/// (self-match on an idempotent re-recovery) and `None` (no binding recorded
/// yet) are treated as no conflict.
pub(crate) fn binding_conflict(
    current_attendee_id: &str,
    wallet_owner: Option<&str>,
    tx_owner: Option<&str>,
) -> bool {
    let wallet_conflict = wallet_owner.is_some_and(|id| id != current_attendee_id);
    let tx_conflict = tx_owner.is_some_and(|id| id != current_attendee_id);
    wallet_conflict || tx_conflict
}
