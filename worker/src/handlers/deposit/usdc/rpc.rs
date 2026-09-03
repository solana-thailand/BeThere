//! Solana RPC calls for deposit verification and their pure response parsers.

use super::types::VerifyWithSignerOutcome;

/// Maximum wall-clock time to wait for a single RPC `getTransaction` response.
///
/// Cloudflare Workers on the free plan have a 30s wall-clock limit per request.
/// A hanging RPC subrequest can exhaust that budget and cause the runtime to
/// return an HTML error page instead of JSON — which the frontend surfaces as
/// "Failed to check deposit confirmation". The 8s timeout keeps the confirm
/// endpoint responsive even when the RPC is slow.
const RPC_TIMEOUT_MS: u32 = 8_000;

// NOTE: The legacy `verify_tx_on_chain` (getSignatureStatuses) and
// `verify_tx_on_chain_impl` helpers were removed in favor of the hardened
// `verify_tx_with_signer` below, which uses `getTransaction` so it can both
// confirm the TX AND cross-check the signer against the expected attendee
// wallet in a single RPC call.

/// Verify a transaction signature on-chain AND cross-check the signer's wallet.
///
/// Uses `getTransaction` to fetch the TX and extract the actual signer
/// (`message.accountKeys[0]`), then compares it against the expected attendee
/// wallet. This closes the impersonation gap where a malicious user could
/// submit someone else's TX signature to get verified. It also makes the
/// worker resilient to missed web2 events: if the deposit record exists with
/// a known `wallet_address` and the on-chain TX is confirmed and signed by
/// that wallet, the worker can confidently flip the deposit to verified.
///
/// # Args
/// - `rpc_url` — Solana RPC endpoint.
/// - `signature` — TX signature to verify.
/// - `expected_wallet` — The attendee wallet address (base58) that should have
///   signed the TX. If `None`, the signer is not cross-checked (caller must
///   handle the extracted signer via [`VerifyWithSignerOutcome::signer`]).
///
/// Each attempt is bounded by [`RPC_TIMEOUT_MS`]. On a transient `RpcError`
/// the call is retried once after a short backoff.
pub(crate) async fn verify_tx_with_signer(
    rpc_url: &str,
    signature: &str,
    expected_wallet: Option<&str>,
) -> VerifyWithSignerOutcome {
    let outcome =
        verify_tx_with_signer_impl(rpc_url, signature, expected_wallet, RPC_TIMEOUT_MS).await;
    if matches!(outcome, VerifyWithSignerOutcome::RpcError) {
        tracing::warn!(
            tx_signature = %signature,
            "RPC error on first verify-with-signer attempt, retrying after 500ms"
        );
        worker::Delay::from(std::time::Duration::from_millis(500)).await;
        return verify_tx_with_signer_impl(rpc_url, signature, expected_wallet, RPC_TIMEOUT_MS)
            .await;
    }
    outcome
}

/// Single-attempt implementation of [`verify_tx_with_signer`].
async fn verify_tx_with_signer_impl(
    rpc_url: &str,
    signature: &str,
    expected_wallet: Option<&str>,
    timeout_ms: u32,
) -> VerifyWithSignerOutcome {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-confirm-signer",
        "method": "getTransaction",
        "params": [
            signature,
            {
                "encoding": "json",
                "maxSupportedTransactionVersion": 0,
                "commitment": "confirmed"
            }
        ]
    });

    let json_body = match serde_json::to_string(&body) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("failed to serialize getTransaction request: {e}");
            return VerifyWithSignerOutcome::RpcError;
        }
    };

    let headers = worker::Headers::new();
    if let Err(e) = headers.set("Content-Type", "application/json") {
        tracing::error!("failed to set header: {e:?}");
        return VerifyWithSignerOutcome::RpcError;
    }

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = match worker::Request::new_with_init(rpc_url, &init) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("failed to create RPC request: {e:?}");
            return VerifyWithSignerOutcome::RpcError;
        }
    };

    // Race the fetch against a timeout (same pattern as verify_tx_on_chain_impl).
    // A hanging RPC would otherwise exhaust the Worker's wall-clock budget.
    let fetch = worker::Fetch::Request(request);
    let fetch_fut = futures_util::FutureExt::fuse(fetch.send());
    let timeout = futures_util::FutureExt::fuse(worker::Delay::from(
        std::time::Duration::from_millis(timeout_ms as u64),
    ));
    futures_util::pin_mut!(fetch_fut);
    futures_util::pin_mut!(timeout);

    let mut response = futures_util::select! {
        result = fetch_fut => match result {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = ?e, "RPC getTransaction request failed");
                return VerifyWithSignerOutcome::RpcError;
            }
        },
        _ = timeout => {
            tracing::warn!(
                tx_signature = %signature,
                timeout_ms,
                "RPC getTransaction timed out"
            );
            return VerifyWithSignerOutcome::RpcError;
        }
    };

    let body_text = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("failed to read RPC response: {e:?}");
            return VerifyWithSignerOutcome::RpcError;
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&body_text) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "failed to parse RPC response");
            return VerifyWithSignerOutcome::RpcError;
        }
    };

    // Delegate to the pure parsing function so the decision logic is unit-testable
    // without needing to mock the Cloudflare Workers Fetch/Delay runtime.
    parse_get_transaction_response(&parsed, signature, expected_wallet)
}

/// Pure parser for a `getTransaction` RPC response.
///
/// Extracted from [`verify_tx_with_signer_impl`] so the decision logic (TX
/// confirmed? signer matches expected wallet?) can be unit-tested without
/// needing the Cloudflare Workers runtime. Callers feed in the already-parsed
/// JSON and the expected attendee wallet; the function returns the
/// [`VerifyWithSignerOutcome`] decision.
///
/// # Decision matrix
///
/// | Condition                                       | Outcome                                  |
/// |-------------------------------------------------|------------------------------------------|
/// | RPC-level `error` field present                 | `RpcError`                               |
/// | `result` missing or `null` (TX not found)       | `Pending`                                |
/// | `meta.err` present (TX failed on-chain)         | `Pending`                                |
/// | `confirmationStatus` not confirmed/finalized    | `Pending`                                |
/// | Confirmed but `accountKeys` missing/malformed   | `RpcError`                               |
/// | Confirmed, signer extracted, compared to wallet | `Confirmed { signer_matched, signer }`   |
pub(crate) fn parse_get_transaction_response(
    parsed: &serde_json::Value,
    signature: &str,
    expected_wallet: Option<&str>,
) -> VerifyWithSignerOutcome {
    // Check for RPC-level error.
    if let Some(error) = parsed.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown");
        tracing::warn!(
            tx_signature = %signature,
            rpc_error = %msg,
            "RPC error on getTransaction"
        );
        return VerifyWithSignerOutcome::RpcError;
    }

    // `result` is `null` if the TX is not found.
    let Some(result) = parsed.get("result") else {
        tracing::debug!(
            tx_signature = %signature,
            "getTransaction returned no result — TX not found"
        );
        return VerifyWithSignerOutcome::Pending;
    };
    if result.is_null() {
        tracing::debug!(
            tx_signature = %signature,
            "getTransaction result is null — TX not found"
        );
        return VerifyWithSignerOutcome::Pending;
    }

    // Treat failed TXs as Pending (caller keeps polling).
    let has_error = result
        .get("meta")
        .and_then(|m| m.get("err"))
        .is_some_and(|e| !e.is_null());
    if has_error {
        let err = result.get("meta").and_then(|m| m.get("err"));
        tracing::warn!(
            tx_signature = %signature,
            tx_err = ?err,
            "TX failed on-chain"
        );
        return VerifyWithSignerOutcome::Pending;
    }

    // Confirmation status. `meta.confirmationStatus` is present when a
    // commitment level is requested in params.
    let confirmation = result
        .get("meta")
        .and_then(|m| m.get("confirmationStatus"))
        .and_then(|s| s.as_str())
        .unwrap_or("confirmed");
    let confirmed = confirmation == "confirmed" || confirmation == "finalized";

    if !confirmed {
        tracing::debug!(
            tx_signature = %signature,
            confirmation_status = %confirmation,
            "TX not yet confirmed"
        );
        return VerifyWithSignerOutcome::Pending;
    }

    // Extract the signer (fee payer = message.accountKeys[0]).
    let account_keys = result
        .get("transaction")
        .and_then(|t| t.get("message"))
        .and_then(|m| m.get("accountKeys"))
        .and_then(|a| a.as_array());

    let Some(account_keys) = account_keys else {
        tracing::warn!(
            tx_signature = %signature,
            "TX confirmed but accountKeys missing — treating as RPC error"
        );
        return VerifyWithSignerOutcome::RpcError;
    };

    let Some(first_key) = account_keys.first().and_then(|k| k.as_str()) else {
        tracing::warn!(
            tx_signature = %signature,
            "TX confirmed but accountKeys[0] missing — treating as RPC error"
        );
        return VerifyWithSignerOutcome::RpcError;
    };

    let signer = first_key.to_string();
    let signer_matched = match expected_wallet {
        Some(expected) => {
            // Base58 pubkeys are case-sensitive, but we use case-insensitive
            // comparison defensively in case of encoding quirks.
            let matched = signer.eq_ignore_ascii_case(expected);
            if !matched {
                tracing::warn!(
                    tx_signature = %signature,
                    signer = %signer,
                    expected_wallet = %expected,
                    "Signer mismatch — TX confirmed but does not match expected wallet"
                );
            } else {
                tracing::info!(
                    tx_signature = %signature,
                    signer = %signer,
                    "TX confirmed and signer matches expected wallet"
                );
            }
            matched
        }
        None => {
            tracing::info!(
                tx_signature = %signature,
                signer = %signer,
                "TX confirmed with signer extraction (no expected wallet provided)"
            );
            true
        }
    };

    VerifyWithSignerOutcome::Confirmed {
        signer_matched,
        signer,
    }
}

/// Pure parser for a `getSignaturesForAddress` RPC response.
///
/// Extracted for unit-testability. Returns the most recent (first in the
/// array) signature that has no on-chain error. Returns `None` if the response
/// is malformed, contains an RPC error, or has no usable signatures.
///
/// The Solana RPC spec orders the `result` array newest-first, so the first
/// error-free entry is the most recent successful transaction touching the
/// queried account (the AttendeeDeposit PDA in the deposit-discovery flow).
pub(crate) fn parse_signatures_for_address_response(parsed: &serde_json::Value) -> Option<String> {
    // Check for RPC-level error.
    if let Some(error) = parsed.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown");
        tracing::warn!(rpc_error = %msg, "RPC error on getSignaturesForAddress");
        return None;
    }

    let signatures = parsed.get("result").and_then(|r| r.as_array())?;

    // Find the most recent signature without an on-chain error.
    // Array is ordered newest-first per Solana RPC spec.
    for entry in signatures {
        // Skip entries with on-chain errors (failed TXs).
        if entry.get("err").is_some_and(|e| !e.is_null()) {
            continue;
        }
        if let Some(sig) = entry.get("signature").and_then(|s| s.as_str()) {
            return Some(sig.to_string());
        }
    }
    None
}
