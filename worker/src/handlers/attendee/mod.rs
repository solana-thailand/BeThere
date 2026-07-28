//! Attendee handlers for the Cloudflare Worker.
//!
//! Mirrors `src/handlers/attendee.rs` from the Axum build but uses
//! `crate::sheets` (worker::Fetch) and `crate::auth` (SubtleCrypto JWT)
//! instead of `reqwest` + `jsonwebtoken`.
//!
//! Split into focused submodules (Issue #052) — each owns a route handler and
//! its private helpers:
//! - [`list`] — `list_attendees`
//! - [`read`] — `get_attendee`, `get_public_ticket`
//! - [`delete`] — `delete_attendee`
//! - [`participation`] — `update_participation_type`
//! - [`admin`] — `flush_cache`, `repair_claim_tokens`

mod admin;
mod delete;
mod list;
mod participation;
mod read;

pub use admin::*;
pub use delete::*;
pub use list::*;
pub use participation::*;
pub use read::*;

#[cfg(test)]
mod tests;
