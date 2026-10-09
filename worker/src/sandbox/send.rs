//! Send a Worker-signed transaction and wait for `confirmed`.
//!
//! The rest of the Worker never sends: wallets do. The sandbox signs as its
//! own organizer and faucet, so it needs `sendTransaction` and a short
//! `getSignatureStatuses` poll. Errors never include the RPC URL (it carries
//! the provider key).

use base64::Engine;

use crate::solana_escrow::EscrowError;
use crate::solana_escrow::json_rpc::{post_request, rpc_result};

/// Polls before giving up; a devnet `confirmed` usually lands in 1–3 s.
const CONFIRM_POLLS: u32 = 30;
const CONFIRM_POLL_MS: u64 = 1_000;

/// Where one signature stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureState {
    /// Unknown to the cluster yet, or only `processed`.
    Pending,
    Confirmed,
    /// Landed with an error; the text is the cluster's error object.
    Failed(String),
}

/// Read one `getSignatureStatuses` response for a single signature.
pub fn signature_state(json: &serde_json::Value) -> Result<SignatureState, String> {
    let result = rpc_result(json)?;
    let status = result
        .get("value")
        .and_then(|v| v.get(0))
        .ok_or_else(|| "getSignatureStatuses: no value".to_string())?;
    if status.is_null() {
        return Ok(SignatureState::Pending);
    }
    match status.get("err") {
        Some(err) if !err.is_null() => return Ok(SignatureState::Failed(err.to_string())),
        _ => {}
    }
    match status.get("confirmationStatus").and_then(|s| s.as_str()) {
        Some("confirmed" | "finalized") => Ok(SignatureState::Confirmed),
        _ => Ok(SignatureState::Pending),
    }
}

async fn call(
    rpc_url: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, EscrowError> {
    let request =
        post_request(rpc_url, "sandbox", method, params).map_err(EscrowError::RpcFailed)?;
    let mut response = worker::Fetch::Request(request)
        .send()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("{method} fetch: {e:?}")))?;
    let status = response.status_code();
    if !(200..300).contains(&status) {
        return Err(EscrowError::RpcFailed(format!("{method}: HTTP {status}")));
    }
    response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("{method} body: {e:?}")))
}

/// Send signed `tx` (preflight on, so a failing transaction is refused before
/// it costs a fee) and poll until it is `confirmed`. Returns the signature.
pub(crate) async fn send_and_confirm(rpc_url: &str, tx: &[u8]) -> Result<String, EscrowError> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(tx);
    let sent = call(
        rpc_url,
        "sendTransaction",
        serde_json::json!([encoded, { "encoding": "base64", "preflightCommitment": "confirmed" }]),
    )
    .await?;
    let signature = rpc_result(&sent)
        .map_err(EscrowError::RpcFailed)?
        .as_str()
        .ok_or_else(|| EscrowError::RpcFailed("sendTransaction: no signature".to_string()))?
        .to_string();

    for _ in 0..CONFIRM_POLLS {
        worker::Delay::from(std::time::Duration::from_millis(CONFIRM_POLL_MS)).await;
        let json = call(
            rpc_url,
            "getSignatureStatuses",
            serde_json::json!([[signature], { "searchTransactionHistory": false }]),
        )
        .await?;
        match signature_state(&json).map_err(EscrowError::RpcFailed)? {
            SignatureState::Confirmed => return Ok(signature),
            SignatureState::Failed(err) => {
                return Err(EscrowError::RpcFailed(format!("transaction failed: {err}")));
            }
            SignatureState::Pending => {}
        }
    }
    Err(EscrowError::RpcFailed(format!(
        "not confirmed after {CONFIRM_POLLS} s"
    )))
}
