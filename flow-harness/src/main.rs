//! flow-harness CLI — plan 005 §3.4 entry point.
//!
//! Drives the staging worker over the default flow set, writes a timestamped
//! `summary.json` + the `.last-green` sentinel, and exits non-zero on any
//! failure (the §3.5 preflight gate keys on the exit code + the sentinel).
//!
//! # Usage
//!
//! Run all flows against the default staging URL (read from
//! `FLOW_HARNESS_WORKER_URL` or the canonical staging hostname):
//!
//! ```text
//! cargo run --release --package flow-harness
//! ```
//!
//! Override the worker URL inline:
//!
//! ```text
//! cargo run --release --package flow-harness -- --worker https://bethere-staging.solana-thailand.workers.dev
//! ```
//!
//! # Required environment
//!
//! The context is built from environment (see `context::StagingContext::from_env`):
//!  - `FLOW_HARNESS_PAYER_KEYPAIR`     — path to a funded Solana keypair JSON (devnet)
//!  - `FLOW_HARNESS_ORGANIZER`          — base58 escrow organizer pubkey
//!  - `FLOW_HARNESS_ATTENDEE_WALLET`    — base58 test attendee wallet pubkey
//!  - `FLOW_HARNESS_DEPOSIT_MINT`       — base58 USDC mint (devnet)
//!  - `FLOW_HARNESS_ATTENDEE_SESSION`   — optional; enables the auth flow's logged-in sub-path
//!
//! Missing required env → the harness exits 2 with an actionable message before
//! any flow runs (fail-fast on misconfiguration, not mid-run).

use std::process::ExitCode;

use clap::Parser;

use flow_harness::client::WorkerClient;
use flow_harness::context::StagingContext;
use flow_harness::flows;
use flow_harness::runner::{default_results_root, Runner};

/// flow-harness CLI args.
///
/// Mirrors the §3.4 invocation contract: an optional `--worker` override that
/// wins over `FLOW_HARNESS_WORKER_URL`, which wins over the canonical staging
/// URL baked into `StagingContext::from_env`.
#[derive(Debug, Parser)]
#[command(
    name = "flow-harness",
    about = "E2E regression harness for plan 005 §3.4 (drives the staging worker).",
    version
)]
struct Cli {
    /// Staging worker base URL (no trailing slash). Overrides
    /// `FLOW_HARNESS_WORKER_URL` and the built-in default.
    #[arg(long, value_name = "URL")]
    worker: Option<String>,
}

/// Entry point. Returns [`ExitCode`] so the §3.5 gate can read it directly.
///
/// Failure modes are deliberately split by exit code so the gate script and CI
/// can distinguish them:
/// - `0`  — every registered flow passed; `.last-green` was touched.
/// - `1`  — one or more flows failed (or the run was empty). Production deploys
///   must block until a subsequent run exits 0.
/// - `2`  — misconfiguration (missing env, unparseable URL, missing keypair).
///   No flow executed; `.last-green` was NOT touched.
fn main() -> ExitCode {
    // Initialise tracing/CLI logging conventionally. The harness emits per-flow
    // lines to stderr so CI logs are readable without piping through `jq`.
    init_logging();

    let cli = Cli::parse();
    let worker_override = cli.worker.as_deref();

    // ── Build the staging context from env + CLI override ────────────────────
    let ctx = match StagingContext::from_env(worker_override) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("❌ flow-harness: configuration error: {e}");
            eprintln!("   See flow-harness/src/context.rs::StagingContext::from_env for the required env vars.");
            return ExitCode::from(2);
        }
    };

    eprintln!("▶  flow-harness targeting {}", ctx.worker_url);
    eprintln!(
        "   event_id={:?} (on_chain={})",
        ctx.event_id_str, ctx.event_id_on_chain
    );
    eprintln!("   organizer={}", ctx.organizer);
    eprintln!("   attendee_wallet={}", ctx.attendee_wallet);

    // ── Build the HTTP client ────────────────────────────────────────────────
    let client = match WorkerClient::new(ctx.worker_url.clone()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ flow-harness: failed to build HTTP client: {e}");
            return ExitCode::from(2);
        }
    };

    // ── Register the default flow set ────────────────────────────────────────
    //
    // `flows::register_default` wires the six canonical flows in dependency
    // order (deposit → refund-pre-event-end → refund-post-event-end-checked-in
    // → refund-no-show-deadline → claim → auth). The auth flow internally
    // calls `AuthFlow::from_env`, so it picks up `FLOW_HARNESS_ATTENDEE_SESSION`
    // at registration time — do NOT register AuthFlow a second time here.
    let results_root = default_results_root();
    let mut runner = Runner::new(ctx.clone(), client, results_root.clone());
    flows::register_default(&mut runner);

    // ── Drive every flow sequentially ────────────────────────────────────────
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("❌ flow-harness: failed to build tokio runtime: {e}");
            return ExitCode::from(2);
        }
    };

    let summary = match rt.block_on(runner.run_all()) {
        Ok(s) => s,
        Err(e) => {
            // A harness-level error (results dir write failure, sentinel write
            // failure) — distinct from a flow failure. Exit 2 so the gate does
            // not treat this as a regression.
            eprintln!("❌ flow-harness: harness-level error: {e}");
            return ExitCode::from(2);
        }
    };

    // ── Print a human-readable summary to stderr ─────────────────────────────
    print_summary(&summary);

    let code = Runner::exit_code(&summary);
    if code == 0 {
        eprintln!("✅ flow-harness: all {} flow(s) passed", summary.passed);
    } else {
        eprintln!(
            "❌ flow-harness: {}/{} flow(s) failed ({} passed, {} skipped)",
            summary.failed, summary.total, summary.passed, summary.skipped
        );
    }
    ExitCode::from(code as u8)
}

/// Print a per-flow table to stderr. Kept compact so CI logs stay readable
/// even with the full six-flow set.
fn print_summary(summary: &flow_harness::runner::RunSummary) {
    eprintln!();
    eprintln!(
        "── flow-harness summary ── started_at={} total_duration_ms={} ──",
        summary.started_at, summary.total_duration_ms
    );
    eprintln!("  worker_url   {}", summary.worker_url);
    eprintln!("  event_id     {}", summary.event_id);
    eprintln!(
        "  totals       {} passed / {} failed / {} skipped of {}",
        summary.passed, summary.failed, summary.skipped, summary.total
    );
    eprintln!();
    for rec in &summary.flows {
        let mark = match rec.outcome {
            flow_harness::runner::FlowOutcome::Passed => "✓",
            flow_harness::runner::FlowOutcome::Failed => "✗",
            flow_harness::runner::FlowOutcome::Skipped => "–",
        };
        eprintln!(
            "  {mark} {:<36} {:>6}ms  {}",
            rec.name,
            rec.duration_ms,
            rec.error.as_deref().unwrap_or("")
        );
        if let Some(kind) = &rec.error_kind {
            eprintln!("      error_kind: {kind}");
        }
    }
    eprintln!();
}

/// Initialise stderr logging. The harness currently uses plain `eprintln!`
/// (no `tracing` subscriber) to keep the dependency surface small; this
/// function exists so a future migration to `tracing` has a single call-site.
fn init_logging() {
    // Intentionally a no-op today. Reserved for a `tracing_subscriber::fmt`
    // initialiser once the flow bodies grow structured logging.
}
