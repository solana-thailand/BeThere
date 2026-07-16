//! RPC polling: fetch signatures, fetch transactions, parse results.

use chrono::Utc;
use futures_util::StreamExt;
use serde::Deserialize;
use worker::D1Database;

use super::{
    EscrowInstruction, IndexSummary, OnChainEvent, POLL_BATCH_SIZE, escrow_program_id, save_cursor,
};

// ---------------------------------------------------------------------------
// RPC response types for polling
// ---------------------------------------------------------------------------

/// RPC response for `getSignaturesForAddress`.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RpcSignaturesForAddress {
    pub result: RpcSignaturesResult,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RpcSignaturesResult {
    #[serde(default)]
    pub signature_infos: Vec<RpcSignatureInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct RpcSignatureInfo {
    pub signature: String,
    pub slot: u64,
    #[serde(default)]
    pub block_time: Option<i64>,
    #[serde(default)]
    pub err: Option<serde_json::Value>,
    #[serde(default)]
    pub memo: Option<String>,
}

/// RPC response for `getTransaction`.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RpcTransactionResponse {
    pub result: Option<RpcTransactionResult>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcTransactionResult {
    pub slot: u64,
    #[serde(default)]
    pub block_time: Option<i64>,
    pub transaction: RpcTransactionData,
    pub meta: Option<RpcTransactionMeta>,
}

#[derive(Debug, Deserialize)]
pub struct RpcTransactionData {
    pub message: RpcTransactionMessage,
}

#[derive(Debug, Deserialize)]
pub struct RpcTransactionMessage {
    #[serde(default, rename = "accountKeys")]
    pub account_keys: Vec<String>,
    pub instructions: Vec<RpcInstruction>,
}

#[derive(Debug, Deserialize)]
pub struct RpcInstruction {
    #[serde(rename = "programIdIndex")]
    pub program_id_index: u8,
    pub accounts: Vec<u8>,
    pub data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct RpcTransactionMeta {
    #[serde(default)]
    pub inner_instructions: Vec<RpcInnerInstructions>,
    #[serde(default)]
    pub err: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RpcInnerInstructions {
    pub index: u8,
    pub instructions: Vec<RpcInstruction>,
}

// ---------------------------------------------------------------------------
// Fetch outcome
// ---------------------------------------------------------------------------

/// Outcome of fetching a single transaction during polling.
pub(crate) enum FetchOutcome {
    SkippedFailed,
    SkippedNoEvent,
    Event(OnChainEvent),
}

// ---------------------------------------------------------------------------
// RPC polling
// ---------------------------------------------------------------------------

/// Poll for new signatures for an escrow address and index them.
///
/// Uses `getSignaturesForAddress` RPC method to fetch recent signatures,
/// then fetches each transaction to extract events.
pub async fn poll_escrow_events(
    db: &D1Database,
    rpc_url: &str,
    escrow_address: &str,
    event_id: &str,
) -> Result<IndexSummary, String> {
    let mut summary = IndexSummary::default();

    // Always fetch the latest batch (no cursor filtering).
    // The dedup mechanism (onchain:sig:{sig}) handles skipping already-indexed events.
    // Using cursor-based `before` pagination misses newer signatures that arrive
    // after the cursor was set.
    let signatures = fetch_signatures_for_address(rpc_url, escrow_address, None).await?;

    if signatures.is_empty() {
        tracing::debug!(escrow = %escrow_address, "no new signatures found");
        return Ok(summary);
    }

    tracing::info!(
        escrow = %escrow_address,
        count = signatures.len(),
        "fetched signatures for polling"
    );

    // H4: Parallelize RPC fetches — process signatures concurrently (max 5 in-flight)
    // to reduce total polling time from O(n) sequential round-trips to ~O(n/5).
    let reversed_sigs: Vec<_> = signatures.into_iter().rev().collect();
    let fetch_futures = reversed_sigs.into_iter().map(|sig_info| {
        let rpc_url = rpc_url.to_string();
        async move {
            if sig_info.err.is_some() {
                return (sig_info, FetchOutcome::SkippedFailed);
            }
            match fetch_transaction(&rpc_url, &sig_info.signature).await {
                Ok(Some(tx)) => {
                    let event = parse_rpc_transaction(&tx, &sig_info);
                    match event {
                        Some(ev) => (sig_info, FetchOutcome::Event(ev)),
                        None => (sig_info, FetchOutcome::SkippedNoEvent),
                    }
                }
                Ok(None) => (sig_info, FetchOutcome::SkippedNoEvent),
                Err(e) => {
                    tracing::error!(
                        sig = %sig_info.signature,
                        error = %e,
                        "failed to fetch transaction"
                    );
                    (sig_info, FetchOutcome::SkippedNoEvent)
                }
            }
        }
    });

    // Execute with bounded concurrency (5 in-flight RPC calls)
    let results: Vec<(RpcSignatureInfo, FetchOutcome)> = futures_util::stream::iter(fetch_futures)
        .buffered(5)
        .collect()
        .await;

    // Process results sequentially for KV writes (must be ordered for cursor)
    for (sig_info, outcome) in results {
        match outcome {
            FetchOutcome::SkippedFailed => {
                summary.skipped_failed += 1;
            }
            FetchOutcome::SkippedNoEvent => {
                summary.skipped_no_event += 1;
            }
            FetchOutcome::Event(event) => {
                match super::store::save_onchain_event(db, event_id, event.clone()).await {
                    Ok(true) => {
                        tracing::info!(
                            sig = %sig_info.signature,
                            "indexed on-chain event via polling"
                        );
                        summary.indexed += 1;
                    }
                    Ok(false) => {
                        summary.duplicates += 1;
                    }
                    Err(e) => {
                        tracing::error!(sig = %sig_info.signature, error = %e, "failed to save polled event");
                        summary.errors += 1;
                    }
                }
            }
        }

        // Update cursor
        let _ = save_cursor(db, escrow_address, &sig_info.signature).await;
    }

    Ok(summary)
}

/// Fetch signatures for an address via RPC `getSignaturesForAddress`.
pub(crate) async fn fetch_signatures_for_address(
    rpc_url: &str,
    address: &str,
    before: Option<&str>,
) -> Result<Vec<RpcSignatureInfo>, String> {
    let mut params = serde_json::json!([
        address,
        { "limit": POLL_BATCH_SIZE, "commitment": "confirmed" }
    ]);

    if let Some(before_sig) = before {
        params[1]["before"] = serde_json::Value::String(before_sig.to_string());
    }

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-poll",
        "method": "getSignaturesForAddress",
        "params": params
    });

    let response_text = rpc_post(rpc_url, &body).await?;

    // Parse response — handle both possible formats
    let parsed: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("failed to parse signatures response: {e:?}"))?;

    let result = parsed
        .get("result")
        .ok_or_else(|| format!("no result in signatures response: {response_text}"))?;

    // The result can be either an array directly or have a signature_infos field
    let infos: Vec<RpcSignatureInfo> = if result.is_array() {
        serde_json::from_value(result.clone())
            .map_err(|e| format!("failed to parse signature infos: {e:?}"))?
    } else {
        // Try as object with signature_infos field
        #[derive(Deserialize)]
        struct Inner {
            #[serde(default, rename = "signatureInfos")]
            signature_infos: Vec<RpcSignatureInfo>,
        }
        let inner: Inner = serde_json::from_value(result.clone())
            .map_err(|e| format!("failed to parse signature infos (object): {e:?}"))?;
        inner.signature_infos
    };

    Ok(infos)
}

/// Fetch a full transaction via RPC `getTransaction`.
pub(crate) async fn fetch_transaction(
    rpc_url: &str,
    signature: &str,
) -> Result<Option<RpcTransactionResult>, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-tx",
        "method": "getTransaction",
        "params": [
            signature,
            {
                "encoding": "json",
                "maxSupportedTransactionVersion": 0
            }
        ]
    });

    let response_text = rpc_post(rpc_url, &body).await?;

    let parsed: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("failed to parse transaction response: {e:?}"))?;

    let result = parsed.get("result");

    match result {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(v) => {
            let tx: RpcTransactionResult = serde_json::from_value(v.clone())
                .map_err(|e| format!("failed to parse transaction result: {e:?}"))?;
            Ok(Some(tx))
        }
    }
}

/// Parse an RPC transaction response into an OnChainEvent.
pub(crate) fn parse_rpc_transaction(
    tx: &RpcTransactionResult,
    sig_info: &RpcSignatureInfo,
) -> Option<OnChainEvent> {
    // Skip if meta has error
    if let Some(meta) = &tx.meta
        && meta.err.is_some()
    {
        return None;
    }

    // Find instructions targeting the escrow program
    for instr in &tx.transaction.message.instructions {
        let program_id_index = instr.program_id_index as usize;
        let program_id = tx.transaction.message.account_keys.get(program_id_index)?;

        if program_id != escrow_program_id() {
            continue;
        }

        // Decode instruction data
        let data_bytes = match crate::solana_escrow::base58_decode(&instr.data) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };

        if data_bytes.is_empty() {
            continue;
        }

        let discriminator = data_bytes[0];
        let instruction = EscrowInstruction::from(discriminator);
        if instruction == EscrowInstruction::Unknown {
            continue;
        }

        // Resolve account addresses from indices
        let accounts: Vec<String> = instr
            .accounts
            .iter()
            .filter_map(|&idx| {
                tx.transaction
                    .message
                    .account_keys
                    .get(idx as usize)
                    .cloned()
            })
            .collect();

        let (organizer, attendee, amount, target_escrow) =
            instruction.extract_fields(&accounts, &data_bytes);

        // Escrow PDA is account index 1 for most instructions
        let escrow_address = accounts
            .get(1)
            .cloned()
            .unwrap_or_else(|| accounts.first().cloned().unwrap_or_default());

        return Some(OnChainEvent {
            signature: sig_info.signature.clone(),
            slot: tx.slot,
            block_time: tx.block_time.unwrap_or(sig_info.block_time.unwrap_or(0)),
            instruction,
            escrow_address,
            target_escrow_address: target_escrow,
            organizer,
            attendee,
            amount,
            indexed_at: Utc::now().to_rfc3339(),
        });
    }

    None
}

/// Execute an RPC POST request using worker::Fetch.
async fn rpc_post(rpc_url: &str, body: &serde_json::Value) -> Result<String, String> {
    let json_body =
        serde_json::to_string(body).map_err(|e| format!("failed to serialize RPC request: {e}"))?;

    let headers = worker::Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("failed to set content-type: {e:?}"))?;

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

    let status = response.status_code();
    if !(200..300).contains(&status) {
        let body_text = response
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read body>".to_string());
        return Err(format!("RPC returned HTTP {status}: {body_text}"));
    }

    response
        .text()
        .await
        .map_err(|e| format!("failed to read RPC response: {e:?}"))
}
