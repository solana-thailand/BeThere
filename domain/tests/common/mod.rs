//! Shared helpers for integration test binaries. Each binary that declares
//! `mod common;` compiles its own copy.

#[cfg(feature = "alloc_count")]
pub mod counting_alloc;
