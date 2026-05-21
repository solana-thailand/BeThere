//! Solana escrow transaction building for USDC deposit.
//!
//! Builds a serialized Solana transaction containing the `deposit` instruction
//! for the bethere-escrow program. The transaction is returned as base64
//! for Solana Pay Transaction Request flow.
//!
//! PDA derivation uses SHA-256 via Web Crypto (SubtleCrypto).
//! Transaction serialization follows the Solana wire format (bincode-like).

pub(crate) mod crypto;
pub(crate) mod tx_builders;
pub(crate) mod wire;

// Re-export all public types and functions from submodules
#[allow(unused_imports)]
pub use crypto::{
    base58_decode, base58_encode, get_associated_token_address, pubkey_from_base58,
    pubkey_to_base58,
};
#[allow(unused_imports)]
pub use tx_builders::{
    build_batch_claim_forfeited_transaction, build_claim_forfeited_transaction,
    build_close_deposit_transaction, build_close_event_transaction,
    build_deactivate_event_transaction, build_deposit_transaction, build_init_escrow_transaction,
    build_mark_checked_in_transaction, build_refund_and_close_transaction,
    build_refund_transaction,
};
#[allow(unused_imports)]
pub use wire::{check_escrow_pda_available, derive_escrow_address, verify_escrow_account_exists};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Bethere-escrow program ID on devnet.
pub(crate) const ESCROW_PROGRAM_ID: &str = "C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T";

/// Devnet USDC mint.
/// Mainnet: EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m
pub(crate) const USDC_MINT_DEVNET: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

/// Mainnet USDC mint.
pub(crate) const USDC_MINT_MAINNET: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m";

/// Returns the USDC mint address based on the SOLANA_CLUSTER env var.
/// Defaults to devnet if not set.
pub(crate) fn usdc_mint() -> &'static str {
    match std::env::var("SOLANA_CLUSTER").unwrap_or_default().as_str() {
        "mainnet-beta" => USDC_MINT_MAINNET,
        _ => USDC_MINT_DEVNET,
    }
}

/// SPL Token program ID.
pub(crate) const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// Associated Token Program ID.
/// Source: https://github.com/solana-program/associated-token-account
pub(crate) const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

/// System program ID.
pub(crate) const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

/// Rent sysvar ID.
pub(crate) const RENT_SYSVAR_ID: &str = "SysvarRent111111111111111111111111111111111";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A 32-byte Solana pubkey.
pub(crate) type PubkeyBytes = [u8; 32];

/// Result of building a deposit transaction.
pub struct DepositTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for Solana Pay.
    pub message: String,
}

/// Result of building a refund transaction.
#[allow(dead_code)]
pub struct RefundTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for Solana Pay.
    pub message: String,
}

/// Result of building a mark_checked_in transaction.
pub struct MarkCheckedInTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for Solana Pay.
    pub message: String,
}

/// Result of building a deactivate_event transaction.
pub struct DeactivateEventTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Result of building a close_event transaction.
pub struct CloseEventTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Result of building a claim_forfeited transaction.
pub struct ClaimForfeitedTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Result of building a close_deposit transaction.
pub struct CloseDepositTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Response for a combined refund + close_deposit transaction.
pub struct RefundAndCloseTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Result of building a combined init escrow transaction.
pub struct InitEscrowTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
    /// Derived EventEscrow PDA address (base58).
    pub escrow_address: String,
    /// Derived vault ATA address (base58).
    pub vault_address: String,
}

/// Error type for escrow operations.
#[derive(Debug)]
#[allow(dead_code)]
pub enum EscrowError {
    /// Invalid base58 pubkey string.
    InvalidPubkey(String),
    /// PDA derivation failed (no valid bump found — should not happen).
    PdaDerivationFailed,
    /// SHA-256 computation failed.
    HashFailed(String),
    /// RPC call failed.
    RpcFailed(String),
    /// On-chain escrow account not found or has wrong state.
    AccountNotFound(String),
}

impl std::fmt::Display for EscrowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPubkey(s) => write!(f, "invalid pubkey: {s}"),
            Self::PdaDerivationFailed => write!(f, "PDA derivation failed — no valid bump found"),
            Self::HashFailed(s) => write!(f, "SHA-256 hash failed: {s}"),
            Self::RpcFailed(s) => write!(f, "RPC call failed: {s}"),
            Self::AccountNotFound(s) => write!(f, "escrow account check failed: {s}"),
        }
    }
}

impl std::error::Error for EscrowError {}
