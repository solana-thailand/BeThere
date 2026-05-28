//! Claim service module — core business logic for NFT claim lookup and minting.
//!
//! Extracted from the HTTP handler layer so the claim flow can be tested
//! and reused independently of Axum/Workers request types.

mod lock;
mod mint;

// Lock management (pub(crate) for internal reuse)
pub(crate) use lock::claim_lock_key;

// Mint/claim orchestration (public API)
pub use mint::{execute_claim, lookup_claim};
