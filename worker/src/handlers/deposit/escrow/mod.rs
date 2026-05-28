mod handlers;
mod status;
pub mod types;
mod workflows;

// Re-export all public handlers so `handlers/deposit/mod.rs` can reference them
// as `escrow::<handler>` without knowing the submodule layout.
pub use handlers::{
    backfill_wallets_handler, claim_forfeited_tx_handler, close_deposit_tx_handler,
    close_event_tx_handler, deactivate_event_tx_handler, init_escrow_tx_handler,
    mark_checked_in_tx_handler, refund_and_close_tx_handler,
};
pub use status::{
    cancel_status_handler, confirm_escrow_init_handler, escrow_health_handler,
    rollover_deposit_tx_handler, usdc_refund_queue_handler,
};

// Re-export the derive_on_chain_event_id helper (used from parent mod.rs via `super::`)
pub(crate) fn derive_on_chain_event_id(event_id: &str) -> u64 {
    super::derive_on_chain_event_id(event_id)
}
