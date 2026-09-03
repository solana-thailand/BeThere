//! Escrow init state machine and form fields.




// ===== State Machine =====

/// State machine for the escrow lifecycle (init → deactivate → close).
#[derive(Debug, Clone, PartialEq)]
pub enum EscrowInitState {
    /// No wallet connected yet.
    Idle,
    /// Wallet connected, ready to init escrow.
    WalletConnected {
        wallet_name: String,
        public_key: String,
    },
    /// Escrow init TX being signed.
    Initializing {
        wallet_name: String,
    },
    /// Escrow initialized on-chain — can deactivate.
    Done {
        escrow_address: String,
        vault_address: String,
        on_chain_event_id: u64,
        signature: String,
    },
    /// Deactivate TX being signed.
    Deactivating {
        wallet_name: String,
    },
    /// Escrow deactivated — vault still exists, can close.
    Deactivated {
        escrow_address: String,
        on_chain_event_id: u64,
    },
    /// Close event TX being signed.
    Closing {
        wallet_name: String,
    },
    /// Escrow closed — rent reclaimed, all on-chain accounts gone.
    Closed {
        signature: String,
    },
    /// Error during any step.
    Error {
        message: String,
    },
}

// ===== Form State (mirrors EventForm fields needed) =====

/// Form fields that the escrow init panel updates on success.
#[derive(Debug, Clone, Default)]
pub struct EscrowFormFields {
    pub escrow_address: String,
    pub on_chain_event_id: String,
}
