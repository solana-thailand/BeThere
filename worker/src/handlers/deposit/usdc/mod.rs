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

// Re-export the helpers consumed by `handlers/` and by
// `handlers::attendee::read`.
//
// `verify_tx_with_signer`, `VerifyWithSignerOutcome` and
// `discover_deposit_tx_on_chain` are deliberately *not* re-exported: they are
// the primitives of the deposit-verification transition, and every caller
// outside this module now goes through `recover_and_verify_deposit`, which is
// the only implementation carrying the F1 and double-registration guards.
// Widening them again is how a second, unguarded verification path gets built.
pub(crate) use confirm::verify_and_confirm_deposit;
pub(crate) use gating::{check_and_switch_deadline, check_in_person_capacity};
pub(crate) use recover::recover_and_verify_deposit;
pub use types::{
    ConfirmDepositQuery, ConfirmDepositResponse, DepositTxQuery, DepositTxResponse,
    UpdateDepositSignatureRequest,
};
