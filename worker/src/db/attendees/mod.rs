//! D1 attendee query helpers.
//!
//! Phase 2a: Every write to Google Sheets also writes to D1.
//! Phase 2b: Read paths try D1 first, fall back to Sheets on miss.
//!
//! This module was split from a single 1470-line file into focused submodules
//! (Issue #052). The split is a pure, behavior-preserving relocation: every
//! item stays `pub(crate)` and reachable as `attendees::NAME`.

mod deposit;
mod management;
mod reads;
mod walkin;
mod writes;

// The deposit-status helpers are all `#[allow(dead_code)]` (no live callers yet);
// keep them reachable as `attendees::NAME` without tripping unused-import lint.
#[allow(unused_imports)]
pub(crate) use deposit::*;
pub(crate) use management::*;
pub(crate) use reads::*;
pub(crate) use walkin::*;
pub(crate) use writes::*;
