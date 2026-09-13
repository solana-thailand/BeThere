//! Escrow initialization panel — single-TX flow.
//!
//! Extracted from `events_page.rs` to keep file sizes manageable.
//! Handles wallet detection, connection, and single-transaction escrow
//! initialization (vault ATA + event escrow in one TX).

mod panel;
mod state;
mod wallet;

pub use panel::EscrowInitPanel;
pub use state::{EscrowFormFields, EscrowInitState};
pub use wallet::{
    SimulateResult, check_wallet_cluster, connect_wallet_js, get_detected_wallets_js,
    get_wallet_cluster_js, sign_and_send_tx_js, simulate_transaction_js,
};
