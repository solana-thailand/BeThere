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

pub mod poller;
pub mod store;
pub mod webhook;

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
// Shared types
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
    RolloverDeposit,
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
            8 => Self::RolloverDeposit,
            _ => Self::Unknown,
        }
    }
}

impl EscrowInstruction {
    /// Extract structured data from instruction data bytes + accounts.
    ///
    /// Returns `(organizer, attendee, amount, target_escrow)` based on instruction type.
    /// `target_escrow` is only `Some` for `RolloverDeposit` (account index 2).
    pub fn extract_fields(
        &self,
        accounts: &[String],
        data_bytes: &[u8],
    ) -> (Option<String>, Option<String>, Option<u64>, Option<String>) {
        match self {
            // create_event(0): [organizer, event_escrow, usdc_mint, vault, ...]
            // data: disc(1) + event_id(8) + deposit_amount(8) + event_end(8) + refund_deadline(8)
            Self::CreateEvent => {
                let organizer = accounts.first().cloned();
                let amount = read_u64_le(data_bytes, 9);
                (organizer, None, amount, None)
            }
            // deposit(1): [attendee, event_escrow, usdc_mint, attendee_deposit, attendee_ta, vault, ...]
            // data: disc(1) + event_id(8)
            Self::Deposit => {
                let attendee = accounts.first().cloned();
                // deposit_amount is in escrow account, not instruction data.
                // We can't extract it from instruction data alone.
                (None, attendee, None, None)
            }
            // mark_checked_in(2): [organizer, event_escrow, attendee_deposit]
            // data: disc(1) + event_id(8)
            Self::MarkCheckedIn => {
                let organizer = accounts.first().cloned();
                let attendee_deposit = accounts.get(2).cloned();
                (organizer, attendee_deposit, None, None)
            }
            // refund(3): [attendee, event_escrow, usdc_mint, attendee_deposit, attendee_ta, vault, ...]
            // data: disc(1) + event_id(8)
            Self::Refund => {
                let attendee = accounts.first().cloned();
                (None, attendee, None, None)
            }
            // claim_forfeited(4): [organizer, event_escrow, organizer_ta, usdc_mint, vault, ...]
            // data: disc(1) + event_id(8)
            Self::ClaimForfeited => {
                let organizer = accounts.first().cloned();
                (organizer, None, None, None)
            }
            // close_event(5): [organizer, event_escrow, vault, token_program]
            // data: disc(1) + event_id(8)
            Self::CloseEvent => {
                let organizer = accounts.first().cloned();
                (organizer, None, None, None)
            }
            // deactivate_event(6): [organizer, event_escrow]
            // data: disc(1) + event_id(8)
            Self::DeactivateEvent => {
                let organizer = accounts.first().cloned();
                (organizer, None, None, None)
            }
            // close_deposit(7): [attendee/anyone, attendee_deposit, event_escrow]
            // data: disc(1) + event_id(8)
            Self::CloseDeposit => {
                let signer = accounts.first().cloned();
                (None, signer, None, None)
            }
            // rollover_deposit(8) account order (from tx_builders.rs):
            //   [0] attendee(S,W), [1] source_escrow(W), [2] source_deposit(W), [3] source_vault(W),
            //   [4] target_escrow(W), [5] target_deposit(W), [6] target_vault(W),
            //   [7] deposit_mint(R), [8] rent(R), [9] token_program(R), [10] system_program(R)
            // data: disc(1) + source_event_id(8) + target_event_id(8)
            Self::RolloverDeposit => {
                let attendee = accounts.first().cloned();
                let target_escrow = accounts.get(4).cloned();
                (None, attendee, None, target_escrow)
            }
            Self::Unknown => (None, None, None, None),
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
    /// For `RolloverDeposit`, this is the **source** escrow.
    pub escrow_address: String,
    /// Target escrow PDA address (base58) — only set for `RolloverDeposit`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_escrow_address: Option<String>,
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
// Event resolution (escrow address → event ID)
// ---------------------------------------------------------------------------

/// Resolve an escrow PDA address to an event ID.
///
/// Uses the reverse index (`escrow:{address} → event_id`) for O(1) lookup (H7).
/// Falls back to scanning all event configs if the index misses (migration).
pub async fn resolve_event_by_escrow(
    d1: Option<&worker::D1Database>,
    kv: Option<&KvStore>,
    escrow_address: &str,
) -> Option<String> {
    // Fast path: reverse index (D1 then KV)
    if let Some(event_id) = crate::event_store::get_event_id_by_escrow(d1, kv, escrow_address).await
    {
        tracing::debug!(
            escrow = %escrow_address,
            event_id = %event_id,
            "resolved escrow via reverse index"
        );
        return Some(event_id);
    }

    // Slow fallback: scan all event configs (for events created before the index existed)
    let kv_ref = kv?;
    tracing::debug!(escrow = %escrow_address, "reverse index miss — falling back to full scan");
    let index = crate::event_store::get_event_index(kv_ref).await.ok()?;

    for meta in &index.events {
        if let Ok(Some(config)) = crate::event_store::get_event_config(kv_ref, &meta.id).await
            && config.escrow_address == escrow_address
        {
            // Backfill the reverse index for future lookups
            let _ = crate::event_store::save_escrow_index(d1, kv, escrow_address, &config.id).await;
            return Some(config.id);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read a `u64` from byte slice at `offset` (little-endian).
pub(crate) fn read_u64_le(data: &[u8], offset: usize) -> Option<u64> {
    if data.len() < offset + 8 {
        return None;
    }
    let bytes: [u8; 8] = data[offset..offset + 8].try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

// ---------------------------------------------------------------------------
// Re-exports — existing `use crate::escrow_indexer::*` must keep working
// ---------------------------------------------------------------------------

pub use poller::poll_escrow_events;

pub use store::{get_onchain_events, read_onchain_events, save_cursor, save_onchain_event};

pub use webhook::{HeliusEnhancedTransaction, parse_helius_transaction};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::escrow_indexer::webhook::HeliusInstruction;

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
        assert_eq!(
            EscrowInstruction::from(8),
            EscrowInstruction::RolloverDeposit
        );
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
        assert_eq!(
            EscrowInstruction::RolloverDeposit.to_string(),
            "rollover_deposit"
        );
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

        let (organizer, attendee, amount, target_escrow) = instr.extract_fields(&accounts, &data);
        assert_eq!(organizer.as_deref(), Some("organizer_pubkey"));
        assert!(attendee.is_none());
        assert_eq!(amount, Some(15_000_000));
        assert!(target_escrow.is_none());
    }

    #[test]
    fn test_escrow_instruction_extract_fields_deposit() {
        let instr = EscrowInstruction::Deposit;
        let accounts = vec!["attendee_pubkey".to_string(), "escrow_pda".to_string()];
        let data = vec![1u8]; // just discriminator

        let (organizer, attendee, amount, target_escrow) = instr.extract_fields(&accounts, &data);
        assert!(organizer.is_none());
        assert_eq!(attendee.as_deref(), Some("attendee_pubkey"));
        assert!(amount.is_none());
        assert!(target_escrow.is_none());
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
    fn test_parse_helius_transaction_rollover_deposit() {
        // Build a base58-encoded instruction data: discriminator(8) + source_event_id(8) + target_event_id(8)
        let mut data_bytes = vec![8u8]; // rollover_deposit discriminator
        data_bytes.extend_from_slice(&1u64.to_le_bytes()); // source_event_id
        data_bytes.extend_from_slice(&2u64.to_le_bytes()); // target_event_id
        let data_b58 = crate::solana_escrow::base58_encode(&data_bytes);

        let tx = HeliusEnhancedTransaction {
            signature: "rollover_sig_456".to_string(),
            slot: 300,
            timestamp: 1700000000,
            fee_payer: Some("attendee_wallet".to_string()),
            description: None,
            tx_type: None,
            transaction_error: None,
            account_data: vec![],
            instruction_data: vec![HeliusInstruction {
                program_id: ESCROW_PROGRAM_ID.to_string(),
                data: data_b58,
                accounts: vec![
                    "attendee_pubkey".to_string(),    // [0] attendee
                    "source_escrow_pda".to_string(),  // [1] source_escrow
                    "source_deposit_pda".to_string(), // [2] source_deposit
                    "source_vault".to_string(),       // [3] source_vault
                    "target_escrow_pda".to_string(),  // [4] target_escrow
                    "target_deposit_pda".to_string(), // [5] target_deposit
                    "target_vault".to_string(),       // [6] target_vault
                    "usdc_mint".to_string(),          // [7] deposit_mint
                    "rent".to_string(),               // [8] rent
                    "token_program".to_string(),      // [9] token_program
                    "system_program".to_string(),     // [10] system_program
                ],
                inner_instructions: vec![],
            }],
            native_transfers: vec![],
            token_transfers: vec![],
            events: None,
        };

        let event = parse_helius_transaction(&tx).unwrap();
        assert_eq!(event.signature, "rollover_sig_456");
        assert_eq!(event.instruction, EscrowInstruction::RolloverDeposit);
        assert_eq!(event.escrow_address, "source_escrow_pda");
        assert_eq!(
            event.target_escrow_address.as_deref(),
            Some("target_escrow_pda")
        );
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
