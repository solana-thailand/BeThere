//! Write operations: saves, creates, updates, deletes, and migration.
//!
//! Split into focused submodules (Issue #052). Every previously-public item
//! remains resolvable as `event_store::write::ITEM` via the glob re-exports below.

mod create;
mod deposit;
mod escrow;
mod form;
mod index;
mod lifecycle;
mod migration;
mod seed;
mod update;

pub use create::*;
pub use deposit::*;
pub use escrow::*;
pub use form::*;
pub use index::*;
pub use lifecycle::*;
pub use migration::*;
pub use seed::*;
pub use update::*;
