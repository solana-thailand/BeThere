#![forbid(unsafe_code)]

// Native-only timing helper for benches; `Instant` panics on wasm32.
#[cfg(not(target_arch = "wasm32"))]
pub mod ab_timing;

pub mod config;
pub mod image_kind;
pub mod models;
pub mod money;
pub mod onchain;
pub mod pr_pack;
pub mod slip_verify;

#[cfg(feature = "qr")]
pub mod qr;

pub mod validation;

#[cfg(feature = "wire")]
pub mod wire;
