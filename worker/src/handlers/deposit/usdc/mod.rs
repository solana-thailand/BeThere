//! USDC deposit handlers — Solana Pay flow, on-chain verification, Helius webhook.

mod confirm;
mod discovery;
mod gating;
mod handlers;
mod recover;
mod rpc;
mod types;

#[cfg(test)]
mod tests;

// Re-export all public handlers so `deposit/mod.rs` can reference them as `usdc::<handler>`.
pub use handlers::{
    confirm_deposit_handler, deposit_usdc_handler, deposit_usdc_tx_handler,
    deposit_webhook_handler, get_deposit_status_handler,
};

// Re-export the helpers and types consumed by `handlers.rs` and by
// `handlers::attendee::read`, so every existing `usdc::<item>` path keeps
// resolving unchanged.
pub(crate) use confirm::verify_and_confirm_deposit;
pub(crate) use discovery::discover_deposit_tx_on_chain;
pub(crate) use gating::{check_and_switch_deadline, check_in_person_capacity};
pub(crate) use recover::recover_and_verify_deposit;
pub(crate) use rpc::verify_tx_with_signer;
pub(crate) use types::VerifyWithSignerOutcome;
pub use types::{
    ConfirmDepositQuery, ConfirmDepositResponse, DepositTxQuery, DepositTxResponse,
    UpdateDepositSignatureRequest,
};
