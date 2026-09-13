//! Rust Adventures — interactive tile-based puzzle game.
//!
//! Teaches Rust programming through a Vim Adventures-style game:
//! grid movement, key collection, code puzzles, NPC dialogs.

mod advanced;
mod basics;
mod config;
mod registry;

pub use advanced::*;
pub use basics::*;
pub use config::{AdventureConfig, AdventureProgress};
pub use registry::default_levels;
