//! On-chain escrow event indexing via Helius webhooks and RPC polling.
//!
//! Bridges the gap between on-chain CPI events emitted by the bethere-escrow
//! program and the off-chain audit trail stored in KV.
//!
//! The escrow program (`C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`) emits
//! 8 event types via `emit!()`:
//!
//! | Discriminator | Event             | Instruction      |
//! |---------------|-------------------|------------------|
//! | 0             | EventCreated      | create_event     |
//! | 1             | Deposited         | deposit          |
//! | 2             | CheckedIn         | mark_checked_in  |
//! | 3             | Refunded          | refund           |
//! | 4             | ForfeitedClaimed  | claim_forfeited  |
//! | 5             | EventClosed       | close_event      |
//! | 6             | EventDeactivated  | deactivate_event |
//! | 7             | DepositClosed     | close_deposit    |
//!
//! KV key schema (EVENTS namespace):
//!
//! | Key pattern                        | Value                           |
//! |------------------------------------|---------------------------------|
//! | `event:{id}:onchain`               | `Vec<OnChainEvent>` (max 200)  |
//! | `onchain:sig:{signature}`          | `"1"` (dedup, TTL 90 days)     |
//! | `onchain:cursor:{escrow_addr}`     | Last processed signature        |
//!
//! Indexing modes:
//!   1. **Webhook**: Helius enhanced webhook → `POST /api/escrow/onchain-webhook`
//!   2. **Manual**:  Admin triggers → `POST /api/escrow/sync`
//!   3. **Cron**:    Daily sync in cleanup cron (catches missed events)

use chrono::Utc;
use serde::{Deserialize, Serialize};
use worker::KvStore;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Bethere-escrow program ID (devnet and mainnet).
pub const ESCROW_PROGRAM_ID: &str = "C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T";

/// Max on-chain events per event (FIFO).
const MAX_ONCHAIN_EVENTS: usize = 200;

/// Max signatures to fetch per polling cycle.
const POLL_BATCH_SIZE: usize = 25;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Instruction discriminators from the bethere-escrow program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscrowInstruction {
    CreateEvent,
    Deposit,
    MarkCheckedIn,
    Refund,
    ClaimForfeited,
    CloseEvent,
    DeactivateEvent,
    CloseDeposit,
    Unknown,
}

impl From<u8> for EscrowInstruction {
    fn from(disc: u8) -> Self {
        match disc {
            0 => Self::CreateEvent,
            1 => Self::Deposit,
            2 => Self::MarkCheckedIn,
            3 => Self::Refund,
            4 => Self::ClaimForfeited,
            5 => Self::CloseEvent,
            6 => Self::DeactivateEvent,
            7 => Self::CloseDeposit,
            _ => Self::Unknown,
        }
    }
}

impl EscrowInstruction {
    /// Extract structured data from instruction data bytes + accounts.
    ///
    /// Returns `(organizer, attendee, amount)` based on instruction type.
    fn extract_fields(
        &self,
        accounts: &[String],
        data_bytes: &[u8],
    ) -> (Option<String>, Option<String>, Option<u64>) {
        match self {
            // create_event(0): [organizer, event_escrow, usdc_mint, vault, ...]
            // data: disc(1) + event_id(8) + deposit_amount(8) + event_end(8) + refund_deadline(8)
            Self::CreateEvent => {
                let organizer = accounts.first().cloned();
                let amount = read_u64_le(data_bytes, 9);
                (organizer, None, amount)
            }
            // deposit(1): [attendee, event_escrow, usdc_mint, attendee_deposit, attendee_ta, vault, ...]
            // data: disc(1) + event_id(8)
            Self::Deposit => {
                let attendee = accounts.first().cloned();
                // deposit_amount is in escrow account, not instruction data.
                // We can't extract it from instruction data alone.
                (None, attendee, None)
            }
            // mark_checked_in(2): [organizer, event_escrow, attendee_deposit]
            // data: disc(1) + event_id(8)
            Self::MarkCheckedIn => {
                let organizer = accounts.first().cloned();
                let attendee_deposit = accounts.get(2).cloned();
                (organizer, attendee_deposit, None)
            }
            // refund(3): [attendee, event_escrow, usdc_mint, attendee_deposit, attendee_ta, vault, ...]
            // data: disc(1) + event_id(8)
            Self::Refund => {
                let attendee = accounts.first().cloned();
                (None, attendee, None)
            }
            // claim_forfeited(4): [organizer, event_escrow, organizer_ta, usdc_mint, vault, ...]
            // data: disc(1) + event_id(8)
            Self::ClaimForfeited => {
                let organizer = accounts.first().cloned();
                (organizer, None, None)
            }
            // close_event(5): [organizer, event_escrow, vault, token_program]
            // data: disc(1) + event_id(8)
            Self::CloseEvent => {
                let organizer = accounts.first().cloned();
                (organizer, None, None)
            }
            // deactivate_event(6): [organizer, event_escrow]
            // data: disc(1) + event_id(8)
            Self::DeactivateEvent => {
                let organizer = accounts.first().cloned();
                (organizer, None, None)
            }
            // close_deposit(7): [attendee/anyone, attendee_deposit, event_escrow]
            // data: disc(1) + event_id(8)
            Self::CloseDeposit => {
                let signer = accounts.first().cloned();
                (None, signer, None)
            }
            Self::Unknown => (None, None, None),
        }
    }
}

impl std::fmt::Display for EscrowInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string(self).unwrap_or_else(|_| "\"unknown\"".to_string());
        // Remove quotes
        write!(f, "{}", s.trim_matches('"'))
    }
}

/// On-chain event record stored in KV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnChainEvent {
    /// Transaction signature (unique identifier).
    pub signature: String,
    /// Slot number.
    pub slot: u64,
    /// Block time (unix timestamp).
    pub block_time: i64,
    /// Which instruction was called.
    pub instruction: EscrowInstruction,
    /// Escrow PDA address (base58).
    pub escrow_address: String,
    /// Organizer wallet address (base58, if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organizer: Option<String>,
    /// Attendee wallet address (base58, if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attendee: Option<String>,
    /// Amount involved (USDC lamports, if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<u64>,
    /// ISO 8601 timestamp when this was indexed.
    pub indexed_at: String,
}

// ---------------------------------------------------------------------------
// Helius enhanced webhook types
// ---------------------------------------------------------------------------

/// Helius enhanced transaction webhook payload.
///
/// See: https://docs.helius.dev/webhooks/webhook-payload
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HeliusEnhancedTransaction {
    /// Transaction signature.
    pub signature: String,
    /// Slot number.
    #[serde(default)]
    pub slot: u64,
    /// Unix timestamp of the transaction.
    #[serde(default)]
    pub timestamp: i64,
    /// Fee payer address.
    #[serde(default, rename = "feePayer")]
    pub fee_payer: Option<String>,
    /// Transaction description (human-readable).
    #[serde(default)]
    pub description: Option<String>,
    /// Transaction type classification.
    #[serde(default, rename = "type")]
    pub tx_type: Option<String>,
    /// Transaction error, if any.
    #[serde(default, rename = "transactionError")]
    pub transaction_error: Option<serde_json::Value>,
    /// Account data with balance changes.
    #[serde(default, rename = "accountData")]
    pub account_data: Vec<HeliusAccountData>,
    /// Parsed instructions.
    #[serde(default, rename = "instructionData")]
    pub instruction_data: Vec<HeliusInstruction>,
    /// Native SOL transfers.
    #[serde(default, rename = "nativeTransfers")]
    pub native_transfers: Vec<serde_json::Value>,
    /// SPL token transfers.
    #[serde(default, rename = "tokenTransfers")]
    pub token_transfers: Vec<HeliusTokenTransfer>,
    /// Classified events (NFT sale, swap, etc).
    #[serde(default)]
    pub events: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HeliusAccountData {
    pub account: String,
    #[serde(default, rename = "nativeBalanceChange")]
    pub native_balance_change: i64,
    #[serde(default, rename = "tokenBalanceChanges")]
    pub token_balance_changes: Vec<HeliusTokenBalanceChange>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HeliusTokenBalanceChange {
    #[serde(default, rename = "userAccount")]
    pub user_account: String,
    #[serde(default, rename = "tokenAccount")]
    pub token_account: String,
    #[serde(default, rename = "rawTokenAmount")]
    pub raw_token_amount: Option<HeliusRawTokenAmount>,
    #[serde(default, rename = "mint")]
    pub mint: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HeliusRawTokenAmount {
    #[serde(default, rename = "tokenAmount")]
    pub token_amount: String,
    #[serde(default, rename = "decimals")]
    pub decimals: u8,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HeliusInstruction {
    /// Program ID that owns this instruction.
    #[serde(default, rename = "programId")]
    pub program_id: String,
    /// Base58-encoded instruction data.
    #[serde(default)]
    pub data: String,
    /// Account addresses involved in this instruction.
    #[serde(default)]
    pub accounts: Vec<String>,
    /// Inner instructions (CPI calls).
    #[serde(default, rename = "innerInstructions")]
    pub inner_instructions: Vec<HeliusInnerInstruction>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HeliusInnerInstruction {
    #[serde(default, rename = "programId")]
    pub program_id: String,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub accounts: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HeliusTokenTransfer {
    #[serde(default, rename = "fromUserAccount")]
    pub from_user_account: Option<String>,
    #[serde(default, rename = "toUserAccount")]
    pub to_user_account: Option<String>,
    #[serde(default, rename = "fromTokenAccount")]
    pub from_token_account: Option<String>,
    #[serde(default, rename = "toTokenAccount")]
    pub to_token_account: Option<String>,
    #[serde(default, rename = "tokenAmount")]
    pub token_amount: f64,
    #[serde(default)]
    pub mint: Option<String>,
}

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
    #[serde(default)]
    pub account_keys: Vec<String>,
    pub instructions: Vec<RpcInstruction>,
}

#[derive(Debug, Deserialize)]
pub struct RpcInstruction {
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
// KV read/write
// ---------------------------------------------------------------------------

/// Read on-chain events for an event.
pub async fn read_onchain_events(kv: &KvStore, event_id: &str) -> Vec<OnChainEvent> {
    let key = format!("event:{event_id}:onchain");
    let raw: Option<String> = match kv.get(&key).text().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(key, "onchain events KV read failed: {e:?}");
            return Vec::new();
        }
    };

    match raw {
        None => Vec::new(),
        Some(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
            tracing::warn!(key, "onchain events parse failed: {e:?}");
            Vec::new()
        }),
    }
}

/// Read on-chain events for an event, newest first (up to `limit`).
pub async fn get_onchain_events(kv: &KvStore, event_id: &str, limit: usize) -> Vec<OnChainEvent> {
    let mut events = read_onchain_events(kv, event_id).await;
    events.reverse();
    events.into_iter().take(limit).collect()
}

/// Save an on-chain event, deduplicating by signature.
pub async fn save_onchain_event(
    kv: &KvStore,
    event_id: &str,
    event: OnChainEvent,
) -> Result<bool, String> {
    // Check dedup
    let dedup_key = format!("onchain:sig:{}", event.signature);
    if let Ok(Some(_)) = kv.get(&dedup_key).text().await {
        tracing::debug!(sig = %event.signature, "on-chain event already indexed, skipping");
        return Ok(false);
    }

    // Append to per-event list
    let key = format!("event:{event_id}:onchain");
    let mut events = read_onchain_events(kv, event_id).await;
    events.push(event);

    // FIFO trim
    let start = events.len().saturating_sub(MAX_ONCHAIN_EVENTS);
    let trimmed = &events[start..];
    let trimmed_vec: Vec<_> = trimmed.to_vec();

    let json = serde_json::to_string(&trimmed_vec)
        .map_err(|e| format!("onchain events serialize failed: {e:?}"))?;

    // Write events
    kv.put(&key, &json)
        .map_err(|e| format!("onchain events put failed: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("onchain events write failed: {e:?}"))?;

    // Mark dedup (TTL handled by cleanup cron)
    let _ = kv
        .put(&dedup_key, "1")
        .map_err(|e| format!("dedup put failed: {e:?}"))?
        .execute()
        .await;

    Ok(true)
}

/// Save the polling cursor (last processed signature) for an escrow address.
pub async fn save_cursor(
    kv: &KvStore,
    escrow_address: &str,
    signature: &str,
) -> Result<(), String> {
    let key = format!("onchain:cursor:{escrow_address}");
    kv.put(&key, signature)
        .map_err(|e| format!("cursor put failed: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("cursor write failed: {e:?}"))
}

/// Read the polling cursor for an escrow address.
pub async fn read_cursor(kv: &KvStore, escrow_address: &str) -> Option<String> {
    let key = format!("onchain:cursor:{escrow_address}");
    kv.get(&key).text().await.ok().flatten()
}

// ---------------------------------------------------------------------------
// Helius webhook parsing
// ---------------------------------------------------------------------------

/// Parse a Helius enhanced transaction into an `OnChainEvent`.
///
/// Looks for instructions targeting the escrow program, decodes the
/// discriminator from the instruction data, and extracts accounts.
pub fn parse_helius_transaction(tx: &HeliusEnhancedTransaction) -> Option<OnChainEvent> {
    // Skip failed transactions
    if tx.transaction_error.is_some() {
        tracing::debug!(sig = %tx.signature, "skipping failed transaction");
        return None;
    }

    for instr in &tx.instruction_data {
        if instr.program_id != ESCROW_PROGRAM_ID {
            continue;
        }

        // Decode instruction discriminator
        let data_bytes = match crate::solana_escrow::base58_decode(&instr.data) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(
                    sig = %tx.signature,
                    data = %instr.data,
                    "failed to decode instruction data: {e:?}"
                );
                continue;
            }
        };

        if data_bytes.is_empty() {
            continue;
        }

        let discriminator = data_bytes[0];
        let instruction = EscrowInstruction::from(discriminator);
        if instruction == EscrowInstruction::Unknown {
            tracing::debug!(
                sig = %tx.signature,
                disc = discriminator,
                "unknown instruction discriminator, skipping"
            );
            continue;
        }

        // Extract structured fields
        let (organizer, attendee, amount) =
            instruction.extract_fields(&instr.accounts, &data_bytes);

        // Escrow PDA is account index 1 for most instructions
        let escrow_address = instr
            .accounts
            .get(1)
            .cloned()
            .unwrap_or_else(|| instr.accounts.first().cloned().unwrap_or_default());

        // Try to extract amount from token transfers for deposit/refund
        let resolved_amount = amount.or_else(|| extract_amount_from_transfers(tx, &instruction));

        return Some(OnChainEvent {
            signature: tx.signature.clone(),
            slot: tx.slot,
            block_time: tx.timestamp,
            instruction,
            escrow_address,
            organizer,
            attendee,
            amount: resolved_amount,
            indexed_at: Utc::now().to_rfc3339(),
        });
    }

    None
}

/// Try to extract USDC amount from Helius token transfers for deposit/refund.
fn extract_amount_from_transfers(
    tx: &HeliusEnhancedTransaction,
    instruction: &EscrowInstruction,
) -> Option<u64> {
    match instruction {
        EscrowInstruction::Deposit
        | EscrowInstruction::Refund
        | EscrowInstruction::ClaimForfeited => {}
        _ => return None,
    }

    // Look for token transfers involving USDC
    for transfer in &tx.token_transfers {
        if transfer.token_amount > 0.0 {
            // Convert from UI amount to raw (6 decimals for USDC)
            return Some((transfer.token_amount * 1_000_000.0) as u64);
        }
    }

    // Fallback: check account token balance changes
    for account in &tx.account_data {
        for change in &account.token_balance_changes {
            if let Some(raw) = &change.raw_token_amount
                && let Ok(amount) = raw.token_amount.parse::<u64>()
                    && amount > 0 {
                        return Some(amount);
                    }
        }
    }

    None
}

/// Index a batch of Helius enhanced transactions.
///
/// For each transaction, parses the event, resolves the event_id from the
/// escrow address, and saves to KV.
#[allow(dead_code)]
pub async fn index_helius_transactions(
    kv: &KvStore,
    transactions: &[HeliusEnhancedTransaction],
    event_resolver: &dyn Fn(&str) -> Option<String>,
) -> IndexSummary {
    let mut summary = IndexSummary::default();

    for tx in transactions {
        // Skip failed transactions
        if tx.transaction_error.is_some() {
            summary.skipped_failed += 1;
            continue;
        }

        let Some(event) = parse_helius_transaction(tx) else {
            summary.skipped_no_event += 1;
            continue;
        };

        // Resolve event ID from escrow address
        let Some(event_id) = event_resolver(&event.escrow_address) else {
            tracing::warn!(
                escrow = %event.escrow_address,
                sig = %event.signature,
                "no off-chain event found for escrow address, skipping"
            );
            summary.skipped_no_event += 1;
            continue;
        };

        match save_onchain_event(kv, &event_id, event.clone()).await {
            Ok(true) => {
                tracing::info!(
                    sig = %event.signature,
                    instruction = %event.instruction,
                    event_id = %event_id,
                    "indexed on-chain event"
                );

                // Also append to audit trail
                let _ = crate::audit_store::append_event_audit(
                    kv,
                    &event_id,
                    crate::audit_store::create_entry_with_meta(
                        "on-chain",
                        crate::audit_store::AuditAction::OnChainEventIndexed,
                        &event.signature,
                        &format!("on-chain: {}", event.instruction),
                        serde_json::json!({
                            "instruction": event.instruction.to_string(),
                            "escrow_address": event.escrow_address,
                            "slot": event.slot,
                            "block_time": event.block_time,
                            "organizer": event.organizer,
                            "attendee": event.attendee,
                            "amount": event.amount,
                        }),
                    ),
                )
                .await;

                summary.indexed += 1;
            }
            Ok(false) => {
                summary.duplicates += 1;
            }
            Err(e) => {
                tracing::error!(sig = %event.signature, error = %e, "failed to save on-chain event");
                summary.errors += 1;
            }
        }
    }

    summary
}

/// Summary of an indexing pass.
#[derive(Debug, Default, Serialize)]
pub struct IndexSummary {
    /// Number of events successfully indexed.
    pub indexed: usize,
    /// Number of duplicates (already indexed).
    pub duplicates: usize,
    /// Number of transactions that failed on-chain.
    pub skipped_failed: usize,
    /// Number of transactions with no escrow event.
    pub skipped_no_event: usize,
    /// Number of storage errors.
    pub errors: usize,
}

// ---------------------------------------------------------------------------
// RPC polling (for manual sync / cron fallback)
// ---------------------------------------------------------------------------

/// Poll for new signatures for an escrow address and index them.
///
/// Uses `getSignaturesForAddress` RPC method to fetch recent signatures,
/// then fetches each transaction to extract events.
pub async fn poll_escrow_events(
    kv: &KvStore,
    rpc_url: &str,
    escrow_address: &str,
    event_id: &str,
) -> Result<IndexSummary, String> {
    let mut summary = IndexSummary::default();

    // Get last cursor
    let cursor = read_cursor(kv, escrow_address).await;

    // Fetch recent signatures
    let signatures =
        fetch_signatures_for_address(rpc_url, escrow_address, cursor.as_deref()).await?;

    if signatures.is_empty() {
        tracing::debug!(escrow = %escrow_address, "no new signatures found");
        return Ok(summary);
    }

    tracing::info!(
        escrow = %escrow_address,
        count = signatures.len(),
        "fetched signatures for polling"
    );

    // Process each signature (oldest first — API returns newest first)
    for sig_info in signatures.into_iter().rev() {
        // Skip failed transactions
        if sig_info.err.is_some() {
            summary.skipped_failed += 1;
            continue;
        }

        // Fetch full transaction
        let tx_result = fetch_transaction(rpc_url, &sig_info.signature).await?;

        let Some(tx) = tx_result else {
            summary.skipped_no_event += 1;
            continue;
        };

        // Parse instructions to find escrow program events
        let event = parse_rpc_transaction(&tx, &sig_info);

        let Some(event) = event else {
            summary.skipped_no_event += 1;
            continue;
        };

        // Save
        match save_onchain_event(kv, event_id, event).await {
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

        // Update cursor
        let _ = save_cursor(kv, escrow_address, &sig_info.signature).await;
    }

    Ok(summary)
}

/// Fetch signatures for an address via RPC `getSignaturesForAddress`.
async fn fetch_signatures_for_address(
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
async fn fetch_transaction(
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
                "encoding": "jsonParsed",
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
fn parse_rpc_transaction(
    tx: &RpcTransactionResult,
    sig_info: &RpcSignatureInfo,
) -> Option<OnChainEvent> {
    // Skip if meta has error
    if let Some(meta) = &tx.meta
        && meta.err.is_some() {
            return None;
        }

    // Find instructions targeting the escrow program
    for instr in &tx.transaction.message.instructions {
        let program_id_index = instr.program_id_index as usize;
        let program_id = tx.transaction.message.account_keys.get(program_id_index)?;

        if program_id != ESCROW_PROGRAM_ID {
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

        let (organizer, attendee, amount) = instruction.extract_fields(&accounts, &data_bytes);

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

// ---------------------------------------------------------------------------
// Event resolution (escrow address → event ID)
// ---------------------------------------------------------------------------

/// Resolve an escrow PDA address to an event ID by scanning all events.
///
/// This iterates the event index and checks `escrow_address` field.
/// For a small number of events (< 100), this is fast enough.
pub async fn resolve_event_by_escrow(kv: &KvStore, escrow_address: &str) -> Option<String> {
    let index = crate::event_store::get_event_index(kv).await.ok()?;

    for meta in &index.events {
        if let Ok(Some(config)) = crate::event_store::get_event_config(kv, &meta.id).await
            && config.escrow_address == escrow_address {
                return Some(config.id);
            }
    }

    None
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read a `u64` from byte slice at `offset` (little-endian).
fn read_u64_le(data: &[u8], offset: usize) -> Option<u64> {
    if data.len() < offset + 8 {
        return None;
    }
    let bytes: [u8; 8] = data[offset..offset + 8].try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escrow_instruction_from_discriminator() {
        assert_eq!(EscrowInstruction::from(0), EscrowInstruction::CreateEvent);
        assert_eq!(EscrowInstruction::from(1), EscrowInstruction::Deposit);
        assert_eq!(EscrowInstruction::from(2), EscrowInstruction::MarkCheckedIn);
        assert_eq!(EscrowInstruction::from(3), EscrowInstruction::Refund);
        assert_eq!(
            EscrowInstruction::from(4),
            EscrowInstruction::ClaimForfeited
        );
        assert_eq!(EscrowInstruction::from(5), EscrowInstruction::CloseEvent);
        assert_eq!(
            EscrowInstruction::from(6),
            EscrowInstruction::DeactivateEvent
        );
        assert_eq!(EscrowInstruction::from(7), EscrowInstruction::CloseDeposit);
        assert_eq!(EscrowInstruction::from(99), EscrowInstruction::Unknown);
    }

    #[test]
    fn test_escrow_instruction_display() {
        assert_eq!(EscrowInstruction::CreateEvent.to_string(), "create_event");
        assert_eq!(EscrowInstruction::Deposit.to_string(), "deposit");
        assert_eq!(
            EscrowInstruction::MarkCheckedIn.to_string(),
            "mark_checked_in"
        );
        assert_eq!(EscrowInstruction::Refund.to_string(), "refund");
        assert_eq!(
            EscrowInstruction::ClaimForfeited.to_string(),
            "claim_forfeited"
        );
        assert_eq!(EscrowInstruction::CloseEvent.to_string(), "close_event");
        assert_eq!(
            EscrowInstruction::DeactivateEvent.to_string(),
            "deactivate_event"
        );
        assert_eq!(EscrowInstruction::CloseDeposit.to_string(), "close_deposit");
    }

    #[test]
    fn test_escrow_instruction_extract_fields_create_event() {
        let instr = EscrowInstruction::CreateEvent;
        let accounts = vec![
            "organizer_pubkey".to_string(),
            "escrow_pda".to_string(),
            "usdc_mint".to_string(),
        ];
        // discriminator(1) + event_id(8) + deposit_amount(8)
        let mut data = vec![0u8];
        data.extend_from_slice(&1u64.to_le_bytes()); // event_id
        data.extend_from_slice(&15_000_000u64.to_le_bytes()); // deposit_amount

        let (organizer, attendee, amount) = instr.extract_fields(&accounts, &data);
        assert_eq!(organizer.as_deref(), Some("organizer_pubkey"));
        assert!(attendee.is_none());
        assert_eq!(amount, Some(15_000_000));
    }

    #[test]
    fn test_escrow_instruction_extract_fields_deposit() {
        let instr = EscrowInstruction::Deposit;
        let accounts = vec!["attendee_pubkey".to_string(), "escrow_pda".to_string()];
        let data = vec![1u8]; // just discriminator

        let (organizer, attendee, amount) = instr.extract_fields(&accounts, &data);
        assert!(organizer.is_none());
        assert_eq!(attendee.as_deref(), Some("attendee_pubkey"));
        assert!(amount.is_none());
    }

    #[test]
    fn test_read_u64_le() {
        let expected: u64 = 5_500_000;
        let data: Vec<u8> = expected.to_le_bytes().to_vec();
        // Pad with leading zeros to test offset
        let mut padded = vec![0u8; 8];
        padded.extend_from_slice(&data);
        assert_eq!(read_u64_le(&padded, 0), Some(0));
        assert_eq!(read_u64_le(&padded, 8), Some(5_500_000u64));
        assert_eq!(read_u64_le(&padded, 20), None);
    }

    #[test]
    fn test_parse_helius_transaction_skip_failed() {
        let tx = HeliusEnhancedTransaction {
            signature: "sig123".to_string(),
            slot: 100,
            timestamp: 1700000000,
            fee_payer: None,
            description: None,
            tx_type: None,
            transaction_error: Some(serde_json::json!({"err": "insufficient funds"})),
            account_data: vec![],
            instruction_data: vec![],
            native_transfers: vec![],
            token_transfers: vec![],
            events: None,
        };

        assert!(parse_helius_transaction(&tx).is_none());
    }

    #[test]
    fn test_parse_helius_transaction_escrow_deposit() {
        // Build a base58-encoded instruction data: discriminator(1) + event_id(8)
        let mut data_bytes = vec![1u8]; // deposit discriminator
        data_bytes.extend_from_slice(&42u64.to_le_bytes()); // event_id
        let data_b58 = crate::solana_escrow::base58_encode(&data_bytes);

        let tx = HeliusEnhancedTransaction {
            signature: "deposit_sig_123".to_string(),
            slot: 200,
            timestamp: 1700000000,
            fee_payer: Some("attendee_wallet".to_string()),
            description: None,
            tx_type: None,
            transaction_error: None,
            account_data: vec![],
            instruction_data: vec![HeliusInstruction {
                program_id: ESCROW_PROGRAM_ID.to_string(),
                data: data_b58,
                accounts: vec!["attendee_pubkey".to_string(), "escrow_pda".to_string()],
                inner_instructions: vec![],
            }],
            native_transfers: vec![],
            token_transfers: vec![],
            events: None,
        };

        let event = parse_helius_transaction(&tx).unwrap();
        assert_eq!(event.signature, "deposit_sig_123");
        assert_eq!(event.instruction, EscrowInstruction::Deposit);
        assert_eq!(event.escrow_address, "escrow_pda");
        assert_eq!(event.attendee.as_deref(), Some("attendee_pubkey"));
        assert!(event.organizer.is_none());
    }

    #[test]
    fn test_index_summary_default() {
        let summary = IndexSummary::default();
        assert_eq!(summary.indexed, 0);
        assert_eq!(summary.duplicates, 0);
        assert_eq!(summary.skipped_failed, 0);
        assert_eq!(summary.skipped_no_event, 0);
        assert_eq!(summary.errors, 0);
    }
}
