//! The deposit promises, written once.
//!
//! They follow the owner's decisions of 2026-09-28
//! (`docs/deposit-commitment-model.md` §5): D1, nothing is forfeited; D3, THB
//! refunds within 7 days. When a decision changes, change it here; the event
//! page and the landing FAQ read these. `tests/deposit_promise_has_one_home.rs`
//! fails if a page writes the promise itself.
//!
//! PromptPay (THB) only: the USDC escrow program can still forfeit after its
//! refund deadline, so no USDC surface may use `NEVER_FORFEITED`.

/// D3: when a THB deposit comes back.
pub const THB_REFUND_WINDOW: &str = "within 7 days after the event";

/// D1: what a no-show loses (nothing).
pub const NEVER_FORFEITED: &str = "Your deposit is never forfeited.";
