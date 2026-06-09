//! Helius webhook parsing: types and transaction processing.

use chrono::Utc;
use serde::Deserialize;
use worker::D1Database;

use super::{ESCROW_PROGRAM_ID, EscrowInstruction, IndexSummary, OnChainEvent};

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
        let (organizer, attendee, amount, target_escrow) =
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
            target_escrow_address: target_escrow,
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
        | EscrowInstruction::ClaimForfeited
        | EscrowInstruction::RolloverDeposit => {}
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
                && amount > 0
            {
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
    db: &D1Database,
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

        match super::store::save_onchain_event(db, &event_id, event.clone()).await {
            Ok(true) => {
                tracing::info!(
                    sig = %event.signature,
                    instruction = %event.instruction,
                    event_id = %event_id,
                    "indexed on-chain event"
                );

                // Note: audit trail not appended here (dead code path)
                // Active webhook handler (onchain_webhook_handler) appends audit.

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
