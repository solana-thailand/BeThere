//! Google Sheets API operations for the Cloudflare Worker.
//!
//! Mirrors `src/sheets/client.rs` from the Axum build but uses
//! `worker::Fetch` (via `crate::http`) and SubtleCrypto (via `crate::crypto`)
//! instead of `reqwest` and the `rsa` crate.

pub mod a1;
mod attendees;
pub mod bg_sync;
mod columns;
pub mod contacts;
pub mod events_tab;
mod gid;
pub mod locate;
mod staff;
mod token;
pub mod write;

// Re-export all public write functions for backward compatibility.
pub use write::*;

pub use attendees::*;
pub(crate) use columns::invalidate_column_map_cache;
pub use columns::{column_mapping_or_hardcoded, get_column_mapping};
pub use gid::resolve_sheet_gid;
pub use staff::get_staff_members;
pub use token::{get_access_token, get_cached_access_token};
