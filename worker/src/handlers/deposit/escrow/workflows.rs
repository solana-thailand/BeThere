use event_checkin_domain::models::deposit::DepositStatus;

use super::types::BackfillDetail;

/// Resolve the attendee's wallet address from an on-chain deposit transaction.
///
/// Uses `getTransaction` RPC to fetch the TX and extracts the first account key
/// (which is the attendee/signer for deposit transactions built by this system).
pub(crate) async fn resolve_wallet_from_tx(
    rpc_url: &str,
    tx_signature: &str,
) -> Result<String, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-backfill",
        "method": "getTransaction",
        "params": [
            tx_signature,
            { "encoding": "json", "maxSupportedTransactionVersion": 0 }
        ]
    });

    let json_body = serde_json::to_string(&body)
        .map_err(|e| format!("failed to serialize getTransaction request: {e}"))?;

    let headers = worker::Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("failed to set header: {e:?}"))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = worker::Request::new_with_init(rpc_url, &init)
        .map_err(|e| format!("failed to create RPC request: {e:?}"))?;

    let mut response = worker::Fetch::Request(request)
        .send()
        .await
        .map_err(|e| format!("RPC request failed: {e:?}"))?;

    let body_text = response
        .text()
        .await
        .map_err(|e| format!("failed to read RPC response: {e:?}"))?;

    let parsed: serde_json::Value = serde_json::from_str(&body_text)
        .map_err(|e| format!("failed to parse RPC response: {e}"))?;

    // Check for RPC error
    if let Some(error) = parsed.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown");
        return Err(format!("RPC error: {msg}"));
    }

    // Navigate to result.transaction.message.accountKeys[0]
    let account_keys = parsed
        .get("result")
        .and_then(|r| r.get("transaction"))
        .and_then(|t| t.get("message"))
        .and_then(|m| m.get("accountKeys"))
        .and_then(|a| a.as_array())
        .ok_or_else(|| "TX not found or expired — account keys not available".to_string())?;

    let first_account = account_keys
        .first()
        .and_then(|k| k.as_str())
        .ok_or_else(|| "no account keys in transaction".to_string())?;

    // Validate it looks like a Solana pubkey (base58, ~32-44 chars)
    if first_account.len() < 32 || first_account.len() > 44 {
        return Err(format!("invalid pubkey length: {}", first_account.len()));
    }

    Ok(first_account.to_string())
}

/// Run the wallet backfill workflow:
/// 1. Collect all deposits that need wallet resolution
/// 2. Resolve wallets concurrently (bounded to 5 parallel RPC calls)
/// 3. Process results — save backfilled wallets
pub(crate) async fn run_backfill_workflow(
    kv: &worker::kv::KvStore,
    rpc_url: &str,
    event_ids: &[String],
) -> BackfillResult {
    use futures_util::stream::{self, StreamExt};

    // Phase 1: Collect all deposits that need wallet resolution (fast — KV reads)
    let mut to_resolve: Vec<(String, DepositStatus)> = Vec::new(); // (event_id, deposit)
    let mut scanned = 0usize;
    let mut already_present = 0usize;
    let mut skipped_details = Vec::new();

    for event_id in event_ids {
        let deposits = event_store::list_deposit_statuses(kv, event_id)
            .await
            .unwrap_or_default();

        for deposit in deposits {
            scanned += 1;

            if deposit.wallet_address.is_some() {
                already_present += 1;
                continue;
            }

            // Only USDC deposits with tx_signature can be backfilled
            if deposit.tx_signature.is_none() {
                skipped_details.push(BackfillDetail {
                    attendee_id: deposit.attendee_id.clone(),
                    result: "skipped".to_string(),
                    wallet_address: None,
                    error: Some("no tx_signature — cannot resolve wallet".to_string()),
                });
                continue;
            }

            to_resolve.push((event_id.clone(), deposit));
        }
    }

    let missing_wallet = to_resolve.len();

    // Phase 2: Resolve wallets concurrently (bounded to 5 parallel RPC calls)
    let resolve_futures = to_resolve.into_iter().map(|(event_id, deposit)| {
        let rpc_url = rpc_url.to_string();
        async move {
            let tx_sig = deposit.tx_signature.as_deref().unwrap();
            let result = resolve_wallet_from_tx(&rpc_url, tx_sig).await;
            (event_id, deposit, result)
        }
    });

    let resolved: Vec<(String, DepositStatus, Result<String, String>)> =
        stream::iter(resolve_futures)
            .buffer_unordered(5)
            .collect()
            .await;

    // Phase 3: Process results — save backfilled wallets
    let mut backfilled = 0usize;
    let mut failed = 0usize;
    let mut details: Vec<BackfillDetail> = skipped_details;

    for (event_id, mut deposit, result) in resolved {
        match result {
            Ok(wallet) => {
                tracing::info!(
                    attendee_id = %deposit.attendee_id,
                    event_id = %event_id,
                    wallet = %wallet,
                    "Backfilled wallet_address"
                );
                deposit.wallet_address = Some(wallet.clone());
                if let Err(e) = event_store::save_deposit_status(kv, &deposit).await {
                    tracing::warn!(
                        attendee_id = %deposit.attendee_id,
                        error = %e,
                        "Failed to save backfilled wallet"
                    );
                    failed += 1;
                    details.push(BackfillDetail {
                        attendee_id: deposit.attendee_id.clone(),
                        result: "save_failed".to_string(),
                        wallet_address: Some(wallet),
                        error: Some(e),
                    });
                } else {
                    backfilled += 1;
                    details.push(BackfillDetail {
                        attendee_id: deposit.attendee_id.clone(),
                        result: "backfilled".to_string(),
                        wallet_address: Some(wallet),
                        error: None,
                    });
                }
            }
            Err(e) => {
                tracing::warn!(
                    attendee_id = %deposit.attendee_id,
                    tx_signature = ?deposit.tx_signature,
                    error = %e,
                    "Failed to resolve wallet from TX"
                );
                failed += 1;
                details.push(BackfillDetail {
                    attendee_id: deposit.attendee_id.clone(),
                    result: "failed".to_string(),
                    wallet_address: None,
                    error: Some(e),
                });
            }
        }
    }

    tracing::info!(
        scanned = %scanned,
        missing_wallet = %missing_wallet,
        backfilled = %backfilled,
        failed = %failed,
        already_present = %already_present,
        "Wallet backfill complete"
    );

    BackfillResult {
        scanned,
        missing_wallet,
        backfilled,
        failed,
        already_present,
        details,
    }
}

pub(crate) struct BackfillResult {
    pub scanned: usize,
    pub missing_wallet: usize,
    pub backfilled: usize,
    pub failed: usize,
    pub already_present: usize,
    pub details: Vec<BackfillDetail>,
}

use crate::event_store;
