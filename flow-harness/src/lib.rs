//! flow-harness — E2E regression harness for plan 005 §3.4.
//!
//! Drives the staging worker over HTTP and asserts the on-chain escrow
//! contract surface (deposit, refund, claim, auth) behaves per
//! `docs/escrow_contract_surface.md`. The harness is the safety mechanism for
//! plans 006 (SIWS) and 007 (Dioxus mobile): the §3.5 preflight gate refuses
//! production deploys unless a green run exists within the last hour.
//!
//! ## Crate layout
//!
//! - [`context`] — `StagingContext`: worker URL, keypairs, derived escrow PDAs.
//! - [`client`] — typed HTTP client over the worker endpoint surface (§6).
//! - [`assertions`] — the two-path refund-window predicate (the regression core).
//! - [`runner`] — orchestrator + `summary.json` / `.last-green` writer.
//! - [`flows`] — one module per scenario in plan 005 §3.4.
//! - [`error`] — `HarnessError` + `EscrowCode` (mirrors the program's error codes).
//!
//! ## Staging-independent vs staging-live
//!
//! Every assertion, PDA derivation, response-shape check, and the runner's
//! results writer are fully offline and `cargo test`-able. Only the flow
//! `run` bodies that issue HTTP calls require staging-live; those call-sites
//! are marked `// TODO(staging-live):` and fail fast with `HarnessError::Config`
//! until §3.1 is provisioned.
//!
//! ## Entry points
//!
//! - `cargo run -- --worker <url>` → CLI runner (`main.rs`).
//! - `cargo test` → runs the offline unit suites (assertions, PDA derivation,
//!   runner orchestration, response parsing).

pub mod assertions;
pub mod client;
pub mod context;
pub mod error;
pub mod flows;
pub mod runner;

// Convenience re-exports so external callers (and `cargo test` from the crate
// root) can write `flow_harness::StagingContext` instead of threading the full
// path. Kept to the load-bearing types; module paths above remain the SSOT.
pub use assertions::{predict_refund_outcome, refund_cta_enabled, RefundOutcome};
pub use client::WorkerClient;
pub use context::StagingContext;
pub use error::{EscrowCode, HarnessError, HarnessResult};
pub use flows::register_default;
pub use runner::{default_results_root, RunSummary, Runner};
