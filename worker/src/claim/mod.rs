//! Claim service module — core business logic for NFT claim lookup and minting.
//!
//! Extracted from the HTTP handler layer so the claim flow can be tested
//! and reused independently of Axum/Workers request types.

mod lock;
mod mint;
mod ttl;

// Lock management (pub(crate) for internal reuse)
pub(crate) use lock::{claim_lock_key, finalized_expires_at};

// Mint/claim orchestration (public API)
pub use mint::{execute_claim, lookup_claim};

// Capability-token replay window (Issue 071). Every site that exchanges a
// token for an attendee must state its policy, so the compiler guarantees no
// resolution path silently skips the window.
pub(crate) use ttl::{ClaimTokenPolicy, DEFAULT_CLAIM_TOKEN_TTL_SECS};

// Shared event-id resolution — token-bearing quiz endpoints reuse this so their
// stored progress matches what the claim gate reads.
pub(crate) use mint::coalesce_event_id;
