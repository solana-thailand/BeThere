//! Rust Adventures — interactive tile-based puzzle game.
//!
//! Teaches Rust programming through a Vim Adventures-style game:
//! grid movement, key collection, code puzzles, NPC dialogs.

pub mod engine;
pub mod levels;
pub mod page;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export level data and game types for convenient access.
pub use levels::{default_levels, test_level, AdventureConfig, AdventureProgress};
