//! D1 event query helpers.
//!
//! Phase 2d: Dual-write events to D1 alongside KV.
//! Write path: upsert on every create/update/seed.
//! Read path: D1-first fallback when KV is empty (e.g. after data loss).

//!
//! Split from one file into a module directory (.issues/052). This file is an
//! index only.

mod config;
mod public;
mod read;
mod row;
mod write;

pub use config::*;
pub use public::*;
pub use read::*;
pub use row::*;
pub use write::*;
