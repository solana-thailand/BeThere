//! Wire format serialization, blockhash management, and on-chain verification.

use worker::KvStore;

use super::crypto::{find_program_address, pubkey_from_base58, pubkey_to_base58};
use super::{EscrowError, PubkeyBytes, escrow_program_id};

// join_all fires the per-candidate getAccountInfo checks concurrently. Helius
// devnet (free/dev tier) rejects the batched `getMultipleAccountsInfo` method
// with {"code":-32603,"message":"Method not found"}, so we fall back to N
// individual `getAccountInfo` calls run in parallel.
use futures_util::future::join_all;

// ---------------------------------------------------------------------------
// Blockhash cache constants
// ---------------------------------------------------------------------------

/// KV key for caching the latest Solana blockhash.
const BLOCKHASH_CACHE_KEY: &str = "cache:blockhash";

/// TTL for the cached blockhash in seconds (30s).
/// Solana blockhashes expire after ~60s on mainnet; 30s gives a good
/// trade-off between RPC call reduction and freshness.
const BLOCKHASH_CACHE_TTL_SECS: u64 = 30;

// ---------------------------------------------------------------------------
// RPC helpers
// ---------------------------------------------------------------------------

/// Recent blockhash from Solana RPC.
pub(crate) struct RecentBlockhash {
    /// The blockhash as base58 string.
    pub(crate) value: String,
}

/// Fetch the latest blockhash, using KV cache when available.
///
/// If `kv` is `Some`, checks KV for a cached blockhash. If present and
/// younger than [`BLOCKHASH_CACHE_TTL_SECS`], returns the cached value.
/// Otherwise fetches from RPC, stores in KV with the configured TTL,
/// and returns the fresh value.
pub(crate) async fn get_latest_blockhash(
    rpc_url: &str,
    kv: Option<&KvStore>,
) -> Result<RecentBlockhash, EscrowError> {
    // Try KV cache first
    if let Some(kv) = kv {
        let cached: Option<String> = kv
            .get(BLOCKHASH_CACHE_KEY)
            .text()
            .await
            .map_err(|e| {
                tracing::warn!("blockhash cache read failed: {e:?}");
                e
            })
            .ok()
            .flatten();

        if let Some(blockhash) = cached
            && !blockhash.is_empty()
        {
            tracing::debug!("using cached blockhash");
            return Ok(RecentBlockhash { value: blockhash });
        }
    }

    // Cache miss or no KV — fetch from RPC
    let blockhash = fetch_blockhash_from_rpc(rpc_url).await?;

    // Store in KV cache (best-effort — don't fail the tx build on cache write errors)
    if let Some(kv) = kv
        && let Err(e) = cache_blockhash(kv, &blockhash.value).await
    {
        tracing::warn!("blockhash cache write failed: {e:?}");
    }

    Ok(blockhash)
}

/// Fetch the latest blockhash directly from the Solana JSON-RPC endpoint.
async fn fetch_blockhash_from_rpc(rpc_url: &str) -> Result<RecentBlockhash, EscrowError> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-deposit",
        "method": "getLatestBlockhash",
        "params": [{ "commitment": "finalized" }]
    });

    let json_body = serde_json::to_string(&body)
        .map_err(|e| EscrowError::RpcFailed(format!("serialize: {e}")))?;

    let headers = worker::Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| EscrowError::RpcFailed(format!("headers: {e:?}")))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = worker::Request::new_with_init(rpc_url, &init)
        .map_err(|e| EscrowError::RpcFailed(format!("request: {e:?}")))?;

    let mut response = worker::Fetch::Request(request)
        .send()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("fetch: {e:?}")))?;

    let status = response.status_code();
    if !(200..300).contains(&status) {
        let text = response.text().await.unwrap_or_default();
        return Err(EscrowError::RpcFailed(format!("HTTP {status}: {text}")));
    }

    let text = response
        .text()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("read body: {e:?}")))?;

    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| EscrowError::RpcFailed(format!("parse json: {e}")))?;

    let blockhash = json["result"]["value"]["blockhash"]
        .as_str()
        .ok_or_else(|| EscrowError::RpcFailed("no blockhash in response".to_string()))?;

    Ok(RecentBlockhash {
        value: blockhash.to_string(),
    })
}

/// Store a blockhash in KV with the configured TTL.
async fn cache_blockhash(kv: &KvStore, blockhash: &str) -> Result<(), EscrowError> {
    kv.put(BLOCKHASH_CACHE_KEY, blockhash)
        .map_err(|e| EscrowError::RpcFailed(format!("blockhash cache put: {e:?}")))?
        .expiration_ttl(BLOCKHASH_CACHE_TTL_SECS)
        .execute()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("blockhash cache execute: {e:?}")))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Escrow PDA address derivation
// ---------------------------------------------------------------------------

/// Derive the escrow PDA address for a given organizer and on-chain event ID.
/// Returns the base58-encoded address.
///
/// This is a local operation (no RPC needed) — PDA derivation uses SHA-256.
pub async fn derive_escrow_address(
    organizer_pubkey: &str,
    event_id: u64,
) -> Result<String, EscrowError> {
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let program_id = pubkey_from_base58(escrow_program_id())?;

    let (event_escrow, _) = find_program_address(
        &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
        &program_id,
    )
    .await?;

    Ok(pubkey_to_base58(&event_escrow))
}

// ---------------------------------------------------------------------------
// On-chain escrow verification
// ---------------------------------------------------------------------------

/// Verify that an escrow account exists on-chain by calling getAccountInfo.
/// Returns Ok(()) if the account exists and is owned by the escrow program,
/// Err if not found or wrong owner.
pub async fn verify_escrow_account_exists(
    rpc_url: &str,
    organizer_pubkey: &str,
    event_id: u64,
) -> Result<(), EscrowError> {
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let program_id = pubkey_from_base58(escrow_program_id())?;

    let (event_escrow, _) = find_program_address(
        &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
        &program_id,
    )
    .await?;

    let escrow_b58 = pubkey_to_base58(&event_escrow);

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-verify-escrow",
        "method": "getAccountInfo",
        "params": [
            escrow_b58,
            { "encoding": "base64", "commitment": "confirmed" }
        ]
    });

    let json_body = serde_json::to_string(&body)
        .map_err(|e| EscrowError::RpcFailed(format!("serialize: {e}")))?;

    let headers = worker::Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| EscrowError::RpcFailed(format!("headers: {e:?}")))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = worker::Request::new_with_init(rpc_url, &init)
        .map_err(|e| EscrowError::RpcFailed(format!("request: {e:?}")))?;

    let mut response = worker::Fetch::Request(request)
        .send()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("fetch: {e:?}")))?;

    let status = response.status_code();
    if !(200..300).contains(&status) {
        let text = response.text().await.unwrap_or_default();
        return Err(EscrowError::RpcFailed(format!("HTTP {status}: {text}")));
    }

    let text = response
        .text()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("read body: {e:?}")))?;

    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| EscrowError::RpcFailed(format!("parse json: {e}")))?;

    let account_info = json.get("result").and_then(|v| v.get("value"));

    match account_info {
        None | Some(serde_json::Value::Null) => Err(EscrowError::AccountNotFound(
            "escrow account does not exist on-chain — it may have already been closed".to_string(),
        )),
        Some(info) => {
            let owner = info.get("owner").and_then(|v| v.as_str()).unwrap_or("");
            if owner != escrow_program_id() {
                return Err(EscrowError::AccountNotFound(format!(
                    "account exists but is not owned by escrow program (owner: {owner})"
                )));
            }
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Forfeitable deposit filtering (defense against indexer lag)
// ---------------------------------------------------------------------------

/// Query a single account via `getAccountInfo` and return whether it exists
/// on-chain and is owned by `expected_owner_b58`.
///
/// Mirrors the RPC pattern of `verify_escrow_account_exists` but returns a
/// boolean instead of short-circuiting. Uses `getAccountInfo` (not the batched
/// `getMultipleAccountsInfo`) because the configured Helius devnet RPC rejects
/// the batched method with `{"code":-32603,"message":"Method not found"}`.
async fn account_exists_owned_by_program(
    rpc_url: &str,
    account_b58: &str,
    expected_owner_b58: &str,
) -> Result<bool, EscrowError> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-account-owner-check",
        "method": "getAccountInfo",
        "params": [
            account_b58,
            { "encoding": "base64", "commitment": "confirmed" }
        ]
    });

    let json_body = serde_json::to_string(&body)
        .map_err(|e| EscrowError::RpcFailed(format!("serialize: {e}")))?;

    let headers = worker::Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| EscrowError::RpcFailed(format!("headers: {e:?}")))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = worker::Request::new_with_init(rpc_url, &init)
        .map_err(|e| EscrowError::RpcFailed(format!("request: {e:?}")))?;

    let mut response = worker::Fetch::Request(request)
        .send()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("fetch: {e:?}")))?;

    let status = response.status_code();
    if !(200..300).contains(&status) {
        let text = response.text().await.unwrap_or_default();
        return Err(EscrowError::RpcFailed(format!("HTTP {status}: {text}")));
    }

    let text = response
        .text()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("read body: {e:?}")))?;

    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| EscrowError::RpcFailed(format!("parse json: {e}")))?;

    Ok(match json.get("result").and_then(|v| v.get("value")) {
        None | Some(serde_json::Value::Null) => false,
        Some(v) => v
            .get("owner")
            .and_then(|o| o.as_str())
            .map(|o| o == expected_owner_b58)
            .unwrap_or(false),
    })
}

/// Verify each candidate attendee's `AttendeeDeposit` PDA still exists on-chain
/// and is owned by the escrow program.
///
/// Returns the subset of wallets whose deposit PDAs are still open. Deposits
/// that were already closed (e.g. refunded-and-closed via `refund_and_close`)
/// are dropped, because claiming them would fail on-chain with `IllegalOwner`
/// at the account-load phase (a closed PDA reverts to SystemProgram ownership,
/// so Quasar's `Account<AttendeeDeposit>` owner check fails in ~195 CU before
/// any instruction logic runs).
///
/// This is a defense against indexer lag: `claim_forfeited_tx_handler` computes
/// candidates from the D1 on-chain event index, which may trail the chain if
/// the Helius webhook / RPC poller is behind. Without this check, a stale index
/// builds a doomed transaction that fails with a cryptic on-chain `IllegalOwner`
/// instead of a clear, actionable message. With it, settled attendees are
/// silently filtered out and only genuinely forfeitable deposits remain.
///
/// Uses N individual `getAccountInfo` calls (fired concurrently via `join_all`)
/// rather than the batched `getMultipleAccountsInfo`, which the configured
/// Helius devnet RPC rejects with "Method not found". The forfeited list is
/// typically small (no-shows per event), so the per-call overhead is acceptable.
pub async fn filter_forfeitable_deposits(
    rpc_url: &str,
    organizer_pubkey: &str,
    event_id: u64,
    attendee_wallets: &[String],
) -> Result<Vec<String>, EscrowError> {
    if attendee_wallets.is_empty() {
        return Ok(Vec::new());
    }

    let program_id = pubkey_from_base58(escrow_program_id())?;
    let organizer = pubkey_from_base58(organizer_pubkey)?;

    let (event_escrow, _) = find_program_address(
        &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
        &program_id,
    )
    .await?;

    // Derive each attendee's AttendeeDeposit PDA; remember the (wallet, pda_b58) pair.
    // Derivation order matches EscrowCtx::attendee_deposit: ["deposit", event_escrow, attendee].
    let mut pairs: Vec<(String, String)> = Vec::with_capacity(attendee_wallets.len());
    for w in attendee_wallets {
        let attendee = pubkey_from_base58(w)?;
        let (deposit_pda, _) = find_program_address(
            &[b"deposit", event_escrow.as_slice(), attendee.as_slice()],
            &program_id,
        )
        .await?;
        pairs.push((w.clone(), pubkey_to_base58(&deposit_pda)));
    }

    // Fire all getAccountInfo checks concurrently. Each future owns its
    // (wallet, pda_b58) clone so the futures don't borrow `pairs`; only
    // rpc_url and the program id are shared by reference and outlive the join.
    let program_id_b58 = escrow_program_id();
    let checks = pairs.iter().map(|(wallet, pda_b58)| {
        let wallet = wallet.clone();
        let pda_b58 = pda_b58.clone();
        async move {
            let live = account_exists_owned_by_program(rpc_url, &pda_b58, program_id_b58).await?;
            Ok::<_, EscrowError>((wallet, live))
        }
    });
    let results = join_all(checks).await;

    let mut live: Vec<String> = Vec::with_capacity(pairs.len());
    for r in results {
        let (wallet, is_live) = r?;
        if is_live {
            live.push(wallet);
        }
    }

    Ok(live)
}

// ---------------------------------------------------------------------------
// Escrow PDA collision detection
// ---------------------------------------------------------------------------

/// Check that the escrow PDA does NOT already exist on-chain.
///
/// Derives the escrow PDA from the organizer pubkey and event ID, then calls
/// `getAccountInfo` to verify the account is **not** already initialized.
///
/// Returns `Ok(escrow_address_b58)` if the account is free to initialize.
/// Returns `Err(EscrowError::AccountNotFound)` if the account already exists
/// (collision) — the error message is repurposed to signal the collision.
pub async fn check_escrow_pda_available(
    rpc_url: &str,
    organizer_pubkey: &str,
    event_id: u64,
) -> Result<String, EscrowError> {
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let program_id = pubkey_from_base58(escrow_program_id())?;

    let (event_escrow, _) = find_program_address(
        &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
        &program_id,
    )
    .await?;

    let escrow_b58 = pubkey_to_base58(&event_escrow);

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-check-pda-available",
        "method": "getAccountInfo",
        "params": [
            escrow_b58,
            { "encoding": "base64", "commitment": "confirmed" }
        ]
    });

    let json_body = serde_json::to_string(&body)
        .map_err(|e| EscrowError::RpcFailed(format!("serialize: {e}")))?;

    let headers = worker::Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| EscrowError::RpcFailed(format!("headers: {e:?}")))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = worker::Request::new_with_init(rpc_url, &init)
        .map_err(|e| EscrowError::RpcFailed(format!("request: {e:?}")))?;

    let mut response = worker::Fetch::Request(request)
        .send()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("fetch: {e:?}")))?;

    let status = response.status_code();
    if !(200..300).contains(&status) {
        let text = response.text().await.unwrap_or_default();
        return Err(EscrowError::RpcFailed(format!("HTTP {status}: {text}")));
    }

    let text = response
        .text()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("read body: {e:?}")))?;

    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| EscrowError::RpcFailed(format!("parse json: {e}")))?;

    let account_info = json.get("result").and_then(|v| v.get("value"));

    match account_info {
        None | Some(serde_json::Value::Null) => {
            // Account does not exist — PDA is available for initialization
            Ok(escrow_b58)
        }
        Some(info) => {
            // Account exists — check if it's owned by the escrow program
            let owner = info.get("owner").and_then(|v| v.as_str()).unwrap_or("");
            if owner == escrow_program_id() {
                // True collision: escrow already initialized
                Err(EscrowError::AccountNotFound(format!(
                    "escrow PDA {escrow_b58} already initialized on-chain for this organizer+event_id"
                )))
            } else {
                // Account exists but not owned by escrow program — still a collision
                Err(EscrowError::AccountNotFound(format!(
                    "account {escrow_b58} already exists on-chain (owner: {owner}), cannot initialize escrow"
                )))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Transaction serialization
// ---------------------------------------------------------------------------

/// Account metadata for transaction building.
#[derive(Clone)]
pub(crate) struct AccountMeta {
    pub(crate) pubkey: PubkeyBytes,
    pub(crate) is_signer: bool,
    pub(crate) is_writable: bool,
}

/// A compiled instruction (post account resolution).
pub(crate) struct CompiledInstruction {
    pub(crate) program_id_index: u8,
    pub(crate) accounts: Vec<u8>,
    pub(crate) data: Vec<u8>,
}

/// Serialize an unsigned Solana transaction in wire format.
///
/// Wire format:
/// 1. Compact-u16: number of signatures (0 for unsigned)
/// 2. Message header: [num_required_signatures, num_readonly_signed, num_readonly_unsigned]
/// 3. Compact-u16: number of account keys
/// 4. Account keys (32 bytes each)
/// 5. Recent blockhash (32 bytes)
/// 6. Compact-u16: number of instructions
/// 7. For each instruction:
///    a. Compact-u16: program ID index
///    b. Compact-u16: number of account indices
///    c. Account indices (1 byte each)
///    d. Compact-u16: data length
///    e. Data bytes
pub(crate) fn serialize_transaction(
    account_metas: &[AccountMeta],
    instructions: &[CompiledInstruction],
    recent_blockhash: &PubkeyBytes,
) -> Vec<u8> {
    let num_signatures = account_metas.iter().filter(|m| m.is_signer).count() as u8;

    let num_readonly_signed = account_metas
        .iter()
        .filter(|m| m.is_signer && !m.is_writable)
        .count() as u8;

    let num_readonly_unsigned = account_metas
        .iter()
        .filter(|m| !m.is_signer && !m.is_writable)
        .count() as u8;

    let mut buf = Vec::new();

    // 1. Signature count — must match num_signatures, with zero-filled placeholders
    // Wallet adapter fills in actual signatures when signing
    encode_compact_u16(num_signatures as usize, &mut buf);
    for _ in 0..num_signatures {
        buf.extend_from_slice(&[0u8; 64]); // zero-filled signature placeholder
    }

    // 2. Message header
    buf.push(num_signatures);
    buf.push(num_readonly_signed);
    buf.push(num_readonly_unsigned);

    // 3. Account keys count
    encode_compact_u16(account_metas.len(), &mut buf);

    // 4. Account keys (32 bytes each)
    for meta in account_metas {
        buf.extend_from_slice(&meta.pubkey);
    }

    // 5. Recent blockhash
    buf.extend_from_slice(recent_blockhash);

    // 6. Instruction count
    encode_compact_u16(instructions.len(), &mut buf);

    // 7. Instructions
    for ix in instructions {
        encode_compact_u16(ix.program_id_index as usize, &mut buf);
        encode_compact_u16(ix.accounts.len(), &mut buf);
        for &idx in &ix.accounts {
            buf.push(idx);
        }
        encode_compact_u16(ix.data.len(), &mut buf);
        buf.extend_from_slice(&ix.data);
    }

    buf
}

/// Encode a compact-u16 (variable-length encoding used in Solana wire format).
pub(crate) fn encode_compact_u16(value: usize, buf: &mut Vec<u8>) {
    let mut val = value as u16;
    loop {
        let mut byte = (val & 0x7f) as u8;
        val >>= 7;
        if val > 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if val == 0 {
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// Message account ordering helper
// ---------------------------------------------------------------------------

/// Build message account keys in Solana's canonical order and resolve instruction indices.
///
/// Solana wire format requires accounts ordered as:
///   1. Signer + writable
///   2. Signer + readonly
///   3. Non-signer + writable
///   4. Non-signer + readonly
///
/// `extra_message_accounts` are CPI-only accounts appended after the 4-pass ordering
/// (deduplicated against already-present accounts).
///
/// Returns (message_accounts, program_id_index, ix_account_indices).
pub(crate) fn build_message_accounts(
    instruction_accounts: &[AccountMeta],
    program_id: &PubkeyBytes,
    extra_message_accounts: &[AccountMeta],
) -> (Vec<AccountMeta>, u8, Vec<u8>) {
    let mut message_accounts: Vec<AccountMeta> = Vec::new();

    // 1. Signer + writable
    for m in instruction_accounts {
        if m.is_signer && m.is_writable {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: true,
                is_writable: true,
            });
        }
    }
    // 2. Signer + readonly
    for m in instruction_accounts {
        if m.is_signer && !m.is_writable {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: true,
                is_writable: false,
            });
        }
    }
    // 3. Non-signer + writable
    for m in instruction_accounts {
        if !m.is_signer && m.is_writable {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: false,
                is_writable: true,
            });
        }
    }
    // 4. Non-signer + readonly (includes program_id if not already present)
    let mut program_id_in_message = false;
    for m in instruction_accounts {
        if !m.is_signer && !m.is_writable {
            if m.pubkey == *program_id {
                program_id_in_message = true;
            }
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: false,
                is_writable: false,
            });
        }
    }
    if !program_id_in_message {
        message_accounts.push(AccountMeta {
            pubkey: *program_id,
            is_signer: false,
            is_writable: false,
        });
    }

    // Append CPI-only extra accounts (deduplicated)
    for m in extra_message_accounts {
        if !message_accounts.iter().any(|ma| ma.pubkey == m.pubkey) {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: false,
                is_writable: false,
            });
        }
    }

    // Build a lookup: pubkey → index in message_accounts
    let get_index = |pk: &PubkeyBytes| -> u8 {
        message_accounts
            .iter()
            .position(|m| &m.pubkey == pk)
            .expect("all accounts should be in message") as u8
    };

    let program_id_index = get_index(program_id);
    let ix_account_indices: Vec<u8> = instruction_accounts
        .iter()
        .map(|m| get_index(&m.pubkey))
        .collect();

    (message_accounts, program_id_index, ix_account_indices)
}
