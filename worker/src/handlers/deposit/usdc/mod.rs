//! USDC deposit handlers — Solana Pay flow, on-chain verification, Helius webhook.

mod handlers;

use chrono::Utc;
use event_checkin_domain::models::event::EventConfig;

use crate::event_store;
use crate::state::AppState;

// Re-export all public handlers so `deposit/mod.rs` can reference them as `usdc::<handler>`.
pub use handlers::{
    confirm_deposit_handler, deposit_usdc_handler, deposit_usdc_tx_handler,
    deposit_webhook_handler, get_deposit_status_handler,
};

// ---------------------------------------------------------------------------
// Types (shared between handlers and helpers)
// ---------------------------------------------------------------------------

/// Query parameters for the Solana Pay TX callback.
#[derive(Debug, serde::Deserialize)]
pub struct DepositTxQuery {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// Attendee's Solana wallet address (base58).
    pub wallet: String,
}

/// Solana Pay Transaction Request response.
///
/// Returned when a wallet calls the callback URL from a Solana Pay QR code.
/// Contains a base64-encoded serialized transaction for the wallet to sign and submit.
#[derive(Debug, serde::Serialize)]
pub struct DepositTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet adds signature).
    pub transaction: String,
    /// Human-readable message shown in the wallet confirmation UI.
    pub message: String,
}

/// Query parameters for checking deposit confirmation.
#[derive(Debug, serde::Deserialize)]
pub struct ConfirmDepositQuery {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID.
    pub attendee_id: String,
}

/// Response for deposit confirmation check.
#[derive(Debug, serde::Serialize)]
pub struct ConfirmDepositResponse {
    /// Whether the deposit has been confirmed on-chain.
    pub confirmed: bool,
    /// Transaction signature if confirmed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_signature: Option<String>,
    /// Solana Pay URL to retry (if not yet confirmed and TX not sent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solana_pay_url: Option<String>,
}

/// Helius webhook payload for transaction notification.
/// See: https://docs.helius.dev/webhooks/webhook-payload
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct HeliusWebhookPayload {
    /// Array of transaction notifications.
    #[serde(default)]
    pub data: Vec<HeliusTransactionData>,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct HeliusTransactionData {
    /// Transaction signature.
    pub signature: String,
    /// Transaction type.
    #[serde(default)]
    pub r#type: String,
    /// Description (human-readable).
    #[serde(default)]
    pub description: String,
}

/// Response body for updating deposit status with TX signature.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UpdateDepositSignatureRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID.
    pub attendee_id: String,
    /// On-chain transaction signature.
    pub tx_signature: String,
}

/// Outcome of an on-chain TX verification with signer cross-check.
///
/// Returned by [`verify_tx_with_signer`] so callers can distinguish between:
/// - **Confirmed (matched)** — TX verified AND signer matches expected wallet,
///   safe to mark deposit as verified.
/// - **Confirmed (mismatch)** — TX verified but signer is NOT the expected
///   attendee wallet — caller should refuse verification (impersonation guard).
/// - **Pending** — TX not found or not yet confirmed, keep polling.
/// - **RpcError** — RPC infrastructure failure (timeout, network, parse);
///   callers should keep polling but log it differently, since this is a
///   transient infrastructure issue rather than a TX-state issue.
///
/// Callers should gate verification on [`Self::is_confirmed_and_matched`] to
/// close the impersonation gap where a malicious user submits someone else's
/// TX signature to get verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VerifyWithSignerOutcome {
    /// TX confirmed/finalized on-chain. `signer_matched` indicates whether the
    /// extracted fee-payer (`message.accountKeys[0]`) matched the expected
    /// attendee wallet. The caller decides how to act on a mismatch.
    Confirmed {
        /// Whether the actual signer matched the expected attendee wallet.
        signer_matched: bool,
        /// The actual signer (base58 pubkey) extracted from the TX.
        signer: String,
    },
    /// TX not found yet, or found but not yet confirmed — caller should keep polling.
    Pending,
    /// RPC infrastructure failure (timeout, network, parse) — caller should retry.
    RpcError,
}

impl VerifyWithSignerOutcome {
    /// Returns `true` only when the TX is confirmed on-chain (regardless of signer match).
    pub fn is_confirmed(&self) -> bool {
        matches!(self, Self::Confirmed { .. })
    }

    /// Returns `true` only when the TX is confirmed AND the signer matched
    /// the expected attendee wallet. This is the gate that deposit verification
    /// should use before marking the deposit as verified.
    pub fn is_confirmed_and_matched(&self) -> bool {
        matches!(
            self,
            Self::Confirmed {
                signer_matched: true,
                ..
            }
        )
    }

    /// Returns the extracted signer address, if confirmed.
    pub fn signer(&self) -> Option<&str> {
        match self {
            Self::Confirmed { signer, .. } => Some(signer.as_str()),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Check if the deposit deadline has passed and auto-switch participation_type.
/// Returns `true` if the deadline expired and the switch was performed.
pub(crate) async fn check_and_switch_deadline(
    state: &AppState,
    kv: Option<&worker::KvStore>,
    event: &EventConfig,
    attendee: &event_checkin_domain::models::attendee::Attendee,
    registration_date_str: &str,
) -> bool {
    let Some(deadline_hours) = event.deposit_deadline_hours else {
        return false;
    };

    // Parse registration_date (ISO 8601)
    let reg_time = match chrono::DateTime::parse_from_rfc3339(registration_date_str) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(e) => {
            tracing::warn!(
                attendee_id = %attendee.api_id,
                error = %e,
                raw = registration_date_str,
                "deposit deadline: failed to parse registration_date"
            );
            return false;
        }
    };

    let deadline = reg_time + chrono::Duration::hours(i64::from(deadline_hours));
    let now = Utc::now();

    if now <= deadline {
        return false; // Still within deadline
    }

    // Deadline passed — auto-switch participation_type in the sheet
    tracing::info!(
        attendee_id = %attendee.api_id,
        event_id = %event.id,
        deadline = %deadline.to_rfc3339(),
        "deposit deadline expired: auto-switching participation_type to Online"
    );

    // Get column mapping for the sheet
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
            tracing::warn!(error = %e, "deadline switch: failed to get column mapping");
            return true; // Deadline expired even if we can't switch
        }
    };

    if let Some(ctx) = &state.worker_ctx {
        ctx.wait_until(crate::sheets::bg_sync::update_participation_type(
            state.clone(),
            attendee.row_index,
            "Online".to_string(),
            mapping,
            event.sheet_id.clone(),
            event.sheet_name.clone(),
            kv.cloned(),
        ));
        tracing::info!(
            attendee_id = %attendee.api_id,
            "deposit deadline: participation_type switched to Online (bg)"
        );
    } else {
        match crate::sheets::write::update_participation_type(
            attendee.row_index,
            "Online",
            &mapping,
            state,
            &event.sheet_id,
            &event.sheet_name,
            kv,
        )
        .await
        {
            Ok(()) => tracing::info!(
                attendee_id = %attendee.api_id,
                "deposit deadline: participation_type switched to Online"
            ),
            Err(e) => tracing::warn!(
                attendee_id = %attendee.api_id,
                error = %e,
                "deposit deadline: failed to update sheet participation_type"
            ),
        }
    }

    true
}

/// Check if in-person capacity is still available for the event.
/// Returns `true` if spots are available (or unlimited), `false` if full.
pub(crate) async fn check_in_person_capacity(
    state: &AppState,
    event: &EventConfig,
    kv: Option<&worker::KvStore>,
) -> bool {
    // No capacity limit = always available (handled by has_in_person_capacity)
    if event.in_person_capacity.is_none() {
        return true;
    }

    // Count in-person attendees from sheet
    let attendees = match crate::sheets::get_attendees_for_event(
        state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
        &event.id,
    )
    .await
    {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(error = %e, "reclaim capacity: failed to get attendees");
            return false; // Assume full on error
        }
    };

    let in_person_count = attendees.iter().filter(|a| a.is_in_person()).count() as u32;

    // Count walk-in attendees from D1
    let mut walkin_count: u32 = 0;
    if let Some(db) = state.d1.as_deref() {
        match crate::db::attendees::count_walkin_attendees(db, &event.id).await {
            Ok(count) => walkin_count = count,
            Err(e) => {
                tracing::warn!(error = %e, "reclaim capacity: failed to count D1 walkins");
            }
        }
    }

    let total = in_person_count + walkin_count;
    event.has_in_person_capacity(total)
}

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
pub(crate) fn parse_signatures_for_address_response(
    parsed: &serde_json::Value,
) -> Option<String> {
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
) -> Option<String> {
    use crate::solana_escrow::{pubkey_from_base58, pubkey_to_base58, ESCROW_PROGRAM_ID};

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
    let program_id = match pubkey_from_base58(ESCROW_PROGRAM_ID) {
        Ok(pk) => pk,
        Err(e) => {
            tracing::error!(error = %e, "invalid ESCROW_PROGRAM_ID constant");
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

    tracing::debug!(
        deposit_pda = %deposit_pda_b58,
        escrow = %escrow_address,
        attendee = %attendee_wallet,
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
            signature = %sig,
            deposit_pda = %deposit_pda_b58,
            "Discovered deposit TX signature on-chain via PDA history"
        );
    } else {
        tracing::debug!(
            deposit_pda = %deposit_pda_b58,
            "No deposit signatures found for AttendeeDeposit PDA"
        );
    }
    signature
}

/// Background task: verify deposit TX on-chain and update status (H8).
///
/// Detached via `wait_until` so the webhook response returns immediately.
/// Owns all data — no borrows on the handler's stack.
pub(crate) async fn verify_and_confirm_deposit(
    state: &AppState,
    body: &UpdateDepositSignatureRequest,
) {
    let kv = state.events_kv.as_ref();
    let d1 = state.d1.as_deref();

    let rpc_url = state.config.solana.full_rpc_url();

    // Load deposit status first to get the expected wallet for signer cross-check.
    // A single `verify_tx_with_signer` call (getTransaction) replaces the previous
    // two-step pattern (getSignatureStatuses pre-check + getTransaction) — same
    // security, half the RPC calls on the success path.
    match event_store::get_deposit_status_with_fallback(
        kv,
        d1,
        &body.event_id,
        &body.attendee_id,
    )
    .await
    {
        Ok(Some(mut deposit_status)) => {
            // Single RPC call via getTransaction: confirms the TX on-chain
            // AND cross-checks the signer against the expected attendee
            // wallet. Closes the impersonation gap (someone submitting
            // another user's TX signature) and makes the worker resilient
            // to missed web2 events: if the deposit record has a known
            // wallet and the on-chain TX is signed by that wallet, we
            // confidently flip the deposit to verified.
            let expected_wallet = deposit_status.wallet_address.as_deref();
            let signer_outcome =
                verify_tx_with_signer(&rpc_url, &body.tx_signature, expected_wallet).await;
            if !signer_outcome.is_confirmed_and_matched() {
                if signer_outcome.is_confirmed() {
                    tracing::warn!(
                        attendee_id = %body.attendee_id,
                        tx_signature = %body.tx_signature,
                        "USDC deposit TX confirmed on-chain but signer does not match expected wallet (background)"
                    );
                } else {
                    tracing::info!(
                        attendee_id = %body.attendee_id,
                        tx_signature = %body.tx_signature,
                        "USDC deposit not yet confirmed on-chain (background check)"
                    );
                }
                return;
            }

            // Backfill the wallet_address if missing (older record or web2
            // hiccup at deposit creation time). Future refunds/check-ins
            // depend on having the depositor's wallet on hand.
            if deposit_status.wallet_address.as_deref().is_none_or(|w| w.is_empty())
                && let Some(signer) = signer_outcome.signer()
            {
                deposit_status.wallet_address = Some(signer.to_string());
            }

            deposit_status.verified = true;
            if let Err(e) =
                event_store::save_deposit_status_with_fallback(kv, d1, &deposit_status).await
            {
                tracing::error!(
                    attendee_id = %body.attendee_id,
                    error = %e,
                    "failed to save verified deposit status in background"
                );
                return;
            }

            tracing::info!(
                attendee_id = %body.attendee_id,
                tx_signature = %body.tx_signature,
                "USDC deposit verified in background"
            );

            // D1 dual-write for verified deposit
            if let Some(db) = d1
                && let Err(e) = crate::db::attendees::verify_deposit(
                    db,
                    &body.attendee_id,
                    "verified",
                    &body.tx_signature,
                    deposit_status.amount as i64,
                    &chrono::Utc::now().to_rfc3339(),
                    "usdc_on_chain_bg",
                )
                .await
            {
                tracing::warn!(
                    attendee_id = %body.attendee_id,
                    error = %e,
                    "D1 USDC deposit verify failed in background (non-fatal)"
                );
            }

            // Audit log
            if let Some(kv) = kv {
                let _ = crate::audit_store::append_event_audit(
                    kv,
                    &body.event_id,
                    crate::audit_store::create_entry_with_meta(
                        "system",
                        crate::audit_store::AuditAction::DepositConfirmed,
                        &body.attendee_id,
                        "USDC deposit confirmed on-chain (background)",
                        serde_json::json!({
                            "tx_signature": body.tx_signature,
                            "confirmed": true,
                        }),
                    ),
                    state.d1.as_deref(),
                )
                .await;
            }

            // Write deposit columns to sheet + auto-generate QR (non-blocking)
            let deposit_amount_str = deposit_status.amount.to_string();
            // Can't collapse: inner lookups depend on event_config bound by this pattern
            #[allow(clippy::collapsible_if)]
            if let Ok(Some(event_config)) =
                event_store::get_event_config_with_fallback(kv, d1, &body.event_id).await
            {
                let (mapping_result, attendee_result) = futures_util::join!(
                    crate::sheets::get_column_mapping(
                        state,
                        &event_config.sheet_id,
                        &event_config.sheet_name,
                        kv,
                    ),
                    crate::sheets::get_attendee_by_id(
                        &body.attendee_id,
                        state,
                        &event_config.sheet_id,
                        &event_config.sheet_name,
                        kv,
                    )
                );
                if let (Ok(mapping), Ok(Some(attendee))) = (mapping_result, attendee_result) {
                    let ctx = crate::sheets::write::SheetContext {
                        mapping: &mapping,
                        state,
                        sheet_id: &event_config.sheet_id,
                        sheet_name: &event_config.sheet_name,
                        kv,
                    };

                    if let Err(e) = crate::sheets::write::write_deposit_verification(
                        attendee.row_index,
                        "USDC",
                        &deposit_amount_str,
                        true,
                        &ctx,
                    )
                    .await
                    {
                        tracing::warn!(error = %e, "failed to write deposit verification to sheet in background");
                    }
                    if attendee.qr_code_url.as_ref().is_none_or(|u| u.is_empty()) {
                        let server_url = &state.config.server.url;
                        let qr_url = format!("{server_url}/staff/?scan={}", attendee.api_id);

                        // D1 write — inline so the ticket page sees the QR immediately.
                        if let Some(ref d1) = state.d1
                            && let Err(e) =
                                crate::db::attendees::set_qr_url(d1, &attendee.api_id, &qr_url).await
                        {
                            tracing::warn!(
                                attendee_id = %attendee.api_id,
                                error = %e,
                                "D1 set_qr_url failed on USDC verify_and_confirm (non-fatal)"
                            );
                        }

                        if let Err(e) = crate::sheets::write::update_qr_urls(
                            &[(attendee.row_index, qr_url)],
                            &mapping,
                            state,
                            &event_config.sheet_id,
                            &event_config.sheet_name,
                            kv,
                        )
                        .await
                        {
                            tracing::warn!(error = %e, "failed to auto-generate QR in background");
                        }
                    }
                }
            }
        }
        Ok(None) => {
            tracing::warn!(
                attendee_id = %body.attendee_id,
                "deposit record disappeared before background verification"
            );
        }
        Err(e) => {
            tracing::error!(
                attendee_id = %body.attendee_id,
                error = %e,
                "failed to reload deposit status for background verification"
            );
        }
    }
}

// ================================================================================================
// Tests
// ================================================================================================
//
// The decision logic for `verify_tx_with_signer` is extracted into
// `parse_get_transaction_response` so it can be unit-tested without needing
// the Cloudflare Workers `Fetch`/`Delay` runtime. The fixtures below mirror
// the shape of real `getTransaction` RPC responses observed on devnet.
//
// Run: `cargo test -p worker --lib parse_get_transaction` (or `verify_signer`)
// from the repo root.

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a confirmed `getTransaction` result with the given fee-payer.
    fn confirmed_tx(signer: &str, confirmation: &str) -> serde_json::Value {
        serde_json::json!({
            "result": {
                "transaction": {
                    "message": {
                        "accountKeys": [signer, "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"]
                    }
                },
                "meta": {
                    "err": null,
                    "confirmationStatus": confirmation
                }
            }
        })
    }

    /// Build a failed-on-chain TX result (e.g., insufficient funds, program error).
    fn failed_tx(_err_msg: &str) -> serde_json::Value {
        serde_json::json!({
            "result": {
                "transaction": {
                    "message": {
                        "accountKeys": ["AqdrF1bMEayzZC72R7SxsC2KFqybT5rHPYswkFWe5Mkn"]
                    }
                },
                "meta": {
                    "err": { "InstructionError": [0, "Custom(1)"] },
                    "confirmationStatus": "confirmed"
                }
            }
        })
    }

    // ─── VerifyWithSignerOutcome methods ──────────────────────────────────

    #[test]
    fn test_outcome_is_confirmed_and_matched_true() {
        let outcome = VerifyWithSignerOutcome::Confirmed {
            signer_matched: true,
            signer: "WalletA".to_string(),
        };
        assert!(outcome.is_confirmed());
        assert!(outcome.is_confirmed_and_matched());
        assert_eq!(outcome.signer(), Some("WalletA"));
    }

    #[test]
    fn test_outcome_is_confirmed_but_not_matched() {
        let outcome = VerifyWithSignerOutcome::Confirmed {
            signer_matched: false,
            signer: "WalletB".to_string(),
        };
        assert!(outcome.is_confirmed());
        assert!(!outcome.is_confirmed_and_matched());
        assert_eq!(outcome.signer(), Some("WalletB"));
    }

    #[test]
    fn test_outcome_pending_neither_confirmed_nor_matched() {
        let outcome = VerifyWithSignerOutcome::Pending;
        assert!(!outcome.is_confirmed());
        assert!(!outcome.is_confirmed_and_matched());
        assert_eq!(outcome.signer(), None);
    }

    #[test]
    fn test_outcome_rpc_error_neither_confirmed_nor_matched() {
        let outcome = VerifyWithSignerOutcome::RpcError;
        assert!(!outcome.is_confirmed());
        assert!(!outcome.is_confirmed_and_matched());
        assert_eq!(outcome.signer(), None);
    }

    // ─── parse_get_transaction_response — happy path ─────────────────────

    #[test]
    fn test_parse_confirmed_signer_matches_expected_wallet() {
        let parsed = confirmed_tx("AqdrF1bMEayzZC72R7SxsC2KFqybT5rHPYswkFWe5Mkn", "confirmed");
        let outcome = parse_get_transaction_response(
            &parsed,
            "sig123",
            Some("AqdrF1bMEayzZC72R7SxsC2KFqybT5rHPYswkFWe5Mkn"),
        );
        assert!(outcome.is_confirmed_and_matched());
        assert_eq!(
            outcome.signer(),
            Some("AqdrF1bMEayzZC72R7SxsC2KFqybT5rHPYswkFWe5Mkn")
        );
    }

    #[test]
    fn test_parse_finalized_signer_matches() {
        // "finalized" should also count as confirmed.
        let parsed = confirmed_tx("WalletX", "finalized");
        let outcome =
            parse_get_transaction_response(&parsed, "sig", Some("WalletX"));
        assert!(outcome.is_confirmed_and_matched());
    }

    #[test]
    fn test_parse_confirmed_no_expected_wallet_backfills_signer() {
        // No expected_wallet → signer is extracted, signer_matched defaults to true.
        let parsed = confirmed_tx("ResolvedSignerABC", "confirmed");
        let outcome = parse_get_transaction_response(&parsed, "sig", None);
        assert!(outcome.is_confirmed_and_matched());
        assert_eq!(outcome.signer(), Some("ResolvedSignerABC"));
    }

    // ─── parse_get_transaction_response — signer mismatch (security) ─────

    #[test]
    fn test_parse_confirmed_signer_does_not_match_expected_wallet() {
        let parsed = confirmed_tx("AttackerWallet", "confirmed");
        let outcome = parse_get_transaction_response(
            &parsed,
            "sig",
            Some("ExpectedAttendeeWallet"),
        );
        // Confirmed, but NOT matched — caller must refuse verification.
        assert!(outcome.is_confirmed());
        assert!(!outcome.is_confirmed_and_matched());
        assert_eq!(outcome.signer(), Some("AttackerWallet"));
    }

    #[test]
    fn test_parse_signer_match_is_case_insensitive() {
        // Defensive case-insensitive comparison handles base58 encoding quirks.
        let parsed = confirmed_tx("WalletABC", "confirmed");
        let outcome = parse_get_transaction_response(&parsed, "sig", Some("walletabc"));
        assert!(outcome.is_confirmed_and_matched());
    }

    // ─── parse_get_transaction_response — Pending paths ──────────────────

    #[test]
    fn test_parse_result_null_is_pending() {
        let parsed = serde_json::json!({ "result": null });
        let outcome = parse_get_transaction_response(&parsed, "sig", Some("WalletA"));
        assert!(matches!(outcome, VerifyWithSignerOutcome::Pending));
    }

    #[test]
    fn test_parse_result_missing_is_pending() {
        let parsed = serde_json::json!({});
        let outcome = parse_get_transaction_response(&parsed, "sig", Some("WalletA"));
        assert!(matches!(outcome, VerifyWithSignerOutcome::Pending));
    }

    #[test]
    fn test_parse_tx_failed_on_chain_is_pending() {
        let parsed = failed_tx("InstructionError");
        let outcome =
            parse_get_transaction_response(&parsed, "sig", Some("WalletA"));
        assert!(matches!(outcome, VerifyWithSignerOutcome::Pending));
    }

    #[test]
    fn test_parse_tx_processed_but_not_confirmed_is_pending() {
        // Some RPCs return "processed" before it reaches "confirmed".
        let parsed = confirmed_tx("WalletA", "processed");
        let outcome =
            parse_get_transaction_response(&parsed, "sig", Some("WalletA"));
        assert!(matches!(outcome, VerifyWithSignerOutcome::Pending));
    }

    // ─── parse_get_transaction_response — RpcError paths ─────────────────

    #[test]
    fn test_parse_rpc_level_error_is_rpc_error() {
        let parsed = serde_json::json!({
            "error": { "code": -32000, "message": "memory allocation failed" }
        });
        let outcome = parse_get_transaction_response(&parsed, "sig", Some("WalletA"));
        assert!(matches!(outcome, VerifyWithSignerOutcome::RpcError));
    }

    #[test]
    fn test_parse_confirmed_but_account_keys_missing_is_rpc_error() {
        // Malformed response: TX is confirmed but no account keys present.
        let parsed = serde_json::json!({
            "result": {
                "transaction": { "message": {} },
                "meta": { "err": null, "confirmationStatus": "confirmed" }
            }
        });
        let outcome = parse_get_transaction_response(&parsed, "sig", Some("WalletA"));
        assert!(matches!(outcome, VerifyWithSignerOutcome::RpcError));
    }

    #[test]
    fn test_parse_confirmed_but_account_keys_empty_is_rpc_error() {
        // Edge case: accountKeys is present but an empty array.
        let parsed = serde_json::json!({
            "result": {
                "transaction": { "message": { "accountKeys": [] } },
                "meta": { "err": null, "confirmationStatus": "confirmed" }
            }
        });
        let outcome = parse_get_transaction_response(&parsed, "sig", Some("WalletA"));
        assert!(matches!(outcome, VerifyWithSignerOutcome::RpcError));
    }

    // ─── Realistic fixture from islanddao-v4-demo (organizer self-deposit) ─

    #[test]
    fn test_parse_realistic_devnet_self_deposit_signer_matches() {
        // Mimics the islanddao-v4-demo scenario from the live investigation:
        // organizer deposited on devnet from their own wallet, so the fee-payer
        // (accountKeys[0]) equals the recorded attendee wallet. This is exactly
        // the "web2 missed the event but on-chain is done" case the signer
        // cross-check is designed to recover from.
        let organizer_wallet = "AqdrF1bMEayzZC72R7SxsC2KFqybT5rHPYswkFWe5Mkn";
        let parsed = confirmed_tx(organizer_wallet, "finalized");
        let outcome =
            parse_get_transaction_response(&parsed, "real-sig", Some(organizer_wallet));
        assert!(outcome.is_confirmed_and_matched());
        assert_eq!(outcome.signer(), Some(organizer_wallet));
    }

    // ─── parse_signatures_for_address_response — discovery path ──────────
    //
    // Used by `discover_deposit_tx_on_chain` to find the most recent
    // successful TX touching an AttendeeDeposit PDA when the deposit record
    // has no tx_signature stored (web2 missed the event, on-chain is done).

    fn sig_entry(sig: &str, err: Option<&str>, confirmation: &str) -> serde_json::Value {
        let err_val = match err {
            Some(msg) => serde_json::json!({ "InstructionError": [0, msg] }),
            None => serde_json::Value::Null,
        };
        serde_json::json!({
            "signature": sig,
            "err": err_val,
            "confirmationStatus": confirmation,
            "slot": 12345
        })
    }

    #[test]
    fn test_parse_signatures_returns_most_recent_successful() {
        // Array is ordered newest-first per Solana RPC spec.
        let parsed = serde_json::json!({
            "result": [
                sig_entry("sig-newest", None, "finalized"),
                sig_entry("sig-older", None, "confirmed")
            ]
        });
        let result = parse_signatures_for_address_response(&parsed);
        assert_eq!(result.as_deref(), Some("sig-newest"));
    }

    #[test]
    fn test_parse_signatures_skips_failed_txs() {
        // Newest entry has an on-chain error — should fall through to the
        // next successful one rather than returning the failed signature.
        let parsed = serde_json::json!({
            "result": [
                sig_entry("sig-failed", Some("Custom(1)"), "confirmed"),
                sig_entry("sig-good", None, "confirmed")
            ]
        });
        let result = parse_signatures_for_address_response(&parsed);
        assert_eq!(result.as_deref(), Some("sig-good"));
    }

    #[test]
    fn test_parse_signatures_all_failed_returns_none() {
        let parsed = serde_json::json!({
            "result": [
                sig_entry("sig-1", Some("InsufficientFunds"), "confirmed"),
                sig_entry("sig-2", Some("Custom(1)"), "confirmed")
            ]
        });
        let result = parse_signatures_for_address_response(&parsed);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_signatures_empty_result_returns_none() {
        let parsed = serde_json::json!({ "result": [] });
        let result = parse_signatures_for_address_response(&parsed);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_signatures_missing_result_returns_none() {
        let parsed = serde_json::json!({});
        let result = parse_signatures_for_address_response(&parsed);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_signatures_rpc_error_returns_none() {
        let parsed = serde_json::json!({
            "error": { "code": -32000, "message": "rate limit exceeded" }
        });
        let result = parse_signatures_for_address_response(&parsed);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_signatures_null_result_returns_none() {
        // Defensive: some RPCs return null for "no history" rather than [].
        let parsed = serde_json::json!({ "result": null });
        let result = parse_signatures_for_address_response(&parsed);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_signatures_single_successful_entry() {
        let parsed = serde_json::json!({
            "result": [sig_entry("only-sig", None, "finalized")]
        });
        let result = parse_signatures_for_address_response(&parsed);
        assert_eq!(result.as_deref(), Some("only-sig"));
    }
}
