//! NFT minting orchestration: claim lookup, execution, and walk-in claims.
//!
//! Contains the core business logic for looking up claim status and executing
//! the full claim flow (validate → gates → lock → mint → record).

mod execute;
mod helpers;
mod lookup;
mod quest;
mod types;
mod walkin;

#[cfg(test)]
mod tests;

pub use execute::execute_claim;
pub use lookup::lookup_claim;

pub(crate) use helpers::coalesce_event_id;
