//! Flow orchestration + results persistence.
//!
//! The runner is the conductor of a harness run: it walks the registered flows
//! in order, records each outcome, and writes a timestamped `summary.json` the
//! preflight gate (plan 005 §3.5) consumes. It is deliberately **agnostic to
//! what each flow does** — flows implement [`Flow`]; the runner only knows how
//! to drive them and persist their verdict.
//!
//! ## Staging-independence
//!
//! Every piece of the runner is fully offline:
//!  - [`Flow`] trait + [`Runner::run_all`] orchestration is pure async glue.
//!  - [`Runner::write_summary`] writes JSON to disk — no network.
//!  - [`Runner::touch_last_green_if_all_passed`] writes a sentinel file whose
//!    mtime the preflight gate (§3.5) reads to enforce "green run within the
//!    last hour".
//!
//! What *does* require staging-live is the *contents* of a real run — the
//! flow bodies issue HTTP calls. The runner's own logic (timing, recording,
//! serialising) is exercised end-to-end by `tests/runner_offline`, which
//! registers no-op flows.
//!
//! ## Run ordering
//!
//! Flows run **sequentially**. Plan 005 §3.4 lists the flows in dependency
//! order: deposit → refund-pre-event-end (negative) → refund-post-event-end
//! (checked-in) → refund-no-show-deadline → claim → auth. Running them in
//! order means a failure in an early flow (e.g. deposit cannot verify)
//! produces a clean, ordered summary rather than interleaved logs. Parallel
//! execution would add nondeterminism to a tool whose entire purpose is
//! deterministic regression detection.
//!
//! ## Results layout
//!
//! ```text
//! flow-harness/results/
//! ├── .last-green                     # touched only when every flow passes
//! └── 2025-01-15T10-30-00Z/
//!     └── summary.json                # the run record (see RunSummary)
//! ```
//!
//! `.last-green`'s mtime is the §3.5 gate signal. The gate checks
//! `now - mtime < 1h`; `--force` bypasses with an audit-log entry.

use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::client::WorkerClient;
use crate::context::StagingContext;
use crate::error::{HarnessError, HarnessResult};

// ── Flow trait ───────────────────────────────────────────────────────────────

/// A single executable flow in the harness.
///
/// Each module under `flows/` implements this trait and is registered with
/// [`Runner::register`]. The runner supplies the shared [`StagingContext`] and
/// [`WorkerClient`]; the flow owns its own request sequence, assertions, and
/// any per-flow scratch state.
///
/// `async_trait` is used so flows can be held as `Box<dyn Flow>` (heterogeneous
/// collection). Native async traits are not yet object-safe on stable Rust.
#[async_trait::async_trait]
pub trait Flow: Send + Sync {
    /// Stable flow identifier used in `summary.json` and logs. Conventionally
    /// the module path (e.g. `"deposit"`, `"refund_no_show_deadline"`).
    fn name(&self) -> &'static str;

    /// Execute the flow. Returning `Ok(())` marks the flow as passed; any
    /// `Err` variant is recorded as a failure with the error's `Display`
    /// string. Negative-test flows return `Ok(())` once they've confirmed the
    /// expected revert was observed — a missing revert is the failure.
    async fn run(&self, ctx: &StagingContext, client: &WorkerClient) -> HarnessResult<()>;
}

// ── Result + summary models ──────────────────────────────────────────────────

/// The verdict for one flow within a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowOutcome {
    /// Flow executed and all assertions held.
    Passed,
    /// Flow executed but an assertion failed, or the worker returned an
    /// unexpected error. The run is failed regardless of other flows.
    Failed,
    /// Flow was not executed. Used when a precondition flow failed and the
    /// runner skips dependents (currently the runner runs everything; this
    /// variant exists for future short-circuit semantics).
    Skipped,
}

/// The recorded outcome of a single flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowRecord {
    pub name: String,
    pub outcome: FlowOutcome,
    /// Wall-clock duration in milliseconds. Useful for the §4 performance
    /// budget (E2E harness against staging < 5min).
    pub duration_ms: u128,
    /// Human-readable error message when `outcome == Failed`. Absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The error's discriminant name (e.g. `"AssertionFailed"`,
    /// `"Worker"`, `"Transport"`). Absent on success. Useful for
    /// triage queries over many runs without parsing free text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
}

/// The full record of one harness run, serialised to `summary.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    /// ISO-8601 start time (UTC). Matches the results directory name.
    pub started_at: String,
    /// Staging worker base URL the run targeted.
    pub worker_url: String,
    /// On-chain event id (for cross-referencing with the seeded event).
    pub event_id: String,
    /// Total wall-clock duration of the run, in milliseconds.
    pub total_duration_ms: u128,
    /// Total flows registered.
    pub total: usize,
    /// Count of flows per outcome. Equal to `total` when summed.
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    /// `true` iff `failed == 0 && skipped == 0`. The §3.5 gate keys on this.
    pub all_passed: bool,
    /// Per-flow records, in registration order.
    pub flows: Vec<FlowRecord>,
}

// ── Runner ───────────────────────────────────────────────────────────────────

/// Orchestrates a harness run: registers flows, drives them sequentially,
/// records outcomes, and persists results.
///
/// Lifetimes: the runner owns the [`StagingContext`] (cheaply cloneable, no
/// mutable state) and [`WorkerClient`] (also cloneable). Flows are boxed trait
/// objects so the runner can hold a heterogeneous collection.
pub struct Runner {
    ctx: StagingContext,
    client: WorkerClient,
    flows: Vec<Box<dyn Flow>>,
    /// Root directory for run artifacts. Defaults to `flow-harness/results`.
    results_root: PathBuf,
    /// Sentinel file touched on a green run; read by the §3.5 preflight gate.
    last_green_path: PathBuf,
}

impl Runner {
    /// Construct a runner bound to a staging context + client.
    ///
    /// `results_root` is created lazily on the first write (see
    /// [`Self::write_summary`]); this constructor does no filesystem work.
    pub fn new(ctx: StagingContext, client: WorkerClient, results_root: PathBuf) -> Self {
        let last_green_path = results_root.join(".last-green");
        Self {
            ctx,
            client,
            flows: Vec::new(),
            results_root,
            last_green_path,
        }
    }

    /// Register a flow. Flows execute in registration order.
    pub fn register<F: Flow + 'static>(&mut self, flow: F) -> &mut Self {
        self.flows.push(Box::new(flow));
        self
    }

    /// Register a pre-boxed flow (useful when a flow needs constructor args
    /// not available at the call site that builds it).
    pub fn register_boxed(&mut self, flow: Box<dyn Flow>) -> &mut Self {
        self.flows.push(flow);
        self
    }

    /// Run every registered flow sequentially, record outcomes, persist
    /// `summary.json`, and return the summary.
    ///
    /// A single flow's failure does not abort the run — every flow executes
    /// so the summary is a complete picture. The caller decides what to do
    /// with [`RunSummary::all_passed`] (the CLI exits non-zero on false;
    /// the preflight gate reads `.last-green`'s mtime).
    pub async fn run_all(&self) -> HarnessResult<RunSummary> {
        let started_at = Utc::now();
        let run_start = Instant::now();

        let mut records = Vec::with_capacity(self.flows.len());
        for flow in &self.flows {
            let name = flow.name();
            let flow_start = Instant::now();
            let outcome = match flow.run(&self.ctx, &self.client).await {
                Ok(()) => FlowOutcome::Passed,
                Err(e) => {
                    let (kind, msg) = error_parts(&e);
                    records.push(FlowRecord {
                        name: name.to_string(),
                        outcome: FlowOutcome::Failed,
                        duration_ms: flow_start.elapsed().as_millis(),
                        error: Some(msg),
                        error_kind: Some(kind),
                    });
                    continue;
                }
            };
            records.push(FlowRecord {
                name: name.to_string(),
                outcome,
                duration_ms: flow_start.elapsed().as_millis(),
                error: None,
                error_kind: None,
            });
        }

        let passed = records
            .iter()
            .filter(|r| r.outcome == FlowOutcome::Passed)
            .count();
        let failed = records
            .iter()
            .filter(|r| r.outcome == FlowOutcome::Failed)
            .count();
        let skipped = records
            .iter()
            .filter(|r| r.outcome == FlowOutcome::Skipped)
            .count();

        let summary = RunSummary {
            started_at: started_at.to_rfc3339(),
            worker_url: self.ctx.worker_url.to_string(),
            event_id: self.ctx.event_id_str.clone(),
            total_duration_ms: run_start.elapsed().as_millis(),
            total: records.len(),
            passed,
            failed,
            skipped,
            all_passed: failed == 0 && skipped == 0 && !records.is_empty(),
            flows: records,
        };

        // Persist before touching the sentinel so a crash mid-write never
        // produces a green marker without a corresponding summary.
        let _ = self.write_summary(&summary, &started_at)?;
        if summary.all_passed {
            self.touch_last_green_if_all_passed(&started_at)?;
        }

        Ok(summary)
    }

    /// Write `summary.json` to `results_root/<ISO-timestamp>/summary.json`.
    ///
    /// Returns the directory path so the caller can surface it. The timestamp
    /// uses hyphens in place of colons so the directory name is portable
    /// across filesystems (Windows forbids `:`).
    pub fn write_summary(&self, summary: &RunSummary, started_at: &chrono::DateTime<Utc>) -> HarnessResult<PathBuf> {
        let dir_name = started_at
            .format("%Y-%m-%dT%H-%M-%SZ")
            .to_string();
        let run_dir = self.results_root.join(&dir_name);

        std::fs::create_dir_all(&run_dir).map_err(|e| {
            HarnessError::Config(format!(
                "create results dir {}: {e}",
                run_dir.display()
            ))
        })?;

        let json = serde_json::to_string_pretty(summary)?;
        let summary_path = run_dir.join("summary.json");
        std::fs::write(&summary_path, json).map_err(|e| {
            HarnessError::Config(format!("write {}: {e}", summary_path.display()))
        })?;

        Ok(run_dir)
    }

    /// Touch the `.last-green` sentinel file iff all flows passed.
    ///
    /// The file's **mtime** is what the §3.5 gate reads (`now - mtime < 1h`).
    /// The file's *contents* are the ISO timestamp + run directory for audit;
    /// they are informational, not load-bearing for the gate.
    pub fn touch_last_green_if_all_passed(
        &self,
        started_at: &chrono::DateTime<Utc>,
    ) -> HarnessResult<()> {
        let dir_name = started_at
            .format("%Y-%m-%dT%H-%M-%SZ")
            .to_string();
        let body = format!(
            "{}\nrun_dir={}\n",
            started_at.to_rfc3339(),
            dir_name
        );

        // Ensure the results root exists (the sentinel lives at its root).
        std::fs::create_dir_all(&self.results_root).map_err(|e| {
            HarnessError::Config(format!(
                "create results root {}: {e}",
                self.results_root.display()
            ))
        })?;

        std::fs::write(&self.last_green_path, body).map_err(|e| {
            HarnessError::Config(format!(
                "write {}: {e}",
                self.last_green_path.display()
            ))
        })
    }

    /// Read the last-green mtime for the preflight gate. Returns `None` if the
    /// sentinel does not exist (gate must treat this as "never green").
    pub fn last_green_mtime(&self) -> HarnessResult<Option<std::time::SystemTime>> {
        match std::fs::metadata(&self.last_green_path) {
            Ok(m) => Ok(Some(
                m.modified()
                    .map_err(|e| HarnessError::Config(format!("stat mtime: {e}")))?,
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(HarnessError::Config(format!(
                "stat {}: {e}",
                self.last_green_path.display()
            ))),
        }
    }

    /// Exit code convention: `0` when all passed, `1` otherwise. Mirrors the
    /// `cargo test` convention so wrapping the harness in CI `cargo run --`
    /// Just Works.
    pub fn exit_code(summary: &RunSummary) -> i32 {
        if summary.all_passed {
            0
        } else {
            1
        }
    }
}

impl std::fmt::Debug for Runner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runner")
            .field("ctx", &self.ctx)
            .field("client", &self.client)
            .field("flow_count", &self.flows.len())
            .field("results_root", &self.results_root)
            .finish_non_exhaustive()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Extract `(discriminant_name, display_message)` from a [`HarnessError`].
/// The discriminant is the variant name (e.g. `"AssertionFailed"`) so summary
/// consumers can group failures without parsing free text.
fn error_parts(err: &HarnessError) -> (String, String) {
    let kind = match err {
        HarnessError::AssertionFailed { .. } => "AssertionFailed",
        HarnessError::Worker(_) => "Worker",
        HarnessError::Transport(_) => "Transport",
        HarnessError::Decode(_) => "Decode",
        HarnessError::Config(_) => "Config",
        HarnessError::Solana(_) => "Solana",
        HarnessError::Other(_) => "Other",
    }
    .to_string();
    (kind, err.to_string())
}

/// Default results root: `<crate root>/results`. Resolved from `CARGO_MANIFEST_DIR`
/// at compile time so the runner works regardless of the caller's CWD.
pub fn default_results_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("results")
}

/// Validate that a path is non-empty and not a directory — small guard used
/// when the gate reads `.last-green`. Kept here so the gate script can share
/// the same definition if it ever moves into Rust.
pub fn is_valid_sentinel(path: &Path) -> bool {
    path.file_name().is_some() && path.is_file()
}

/// Best-effort parse of a `summary.json` back into [`RunSummary`]. Used by
/// ad-hoc triage tooling; not load-bearing for the gate.
pub fn parse_summary_file(path: &Path) -> HarnessResult<RunSummary> {
    let bytes = std::fs::read(path)
        .map_err(|e| HarnessError::Config(format!("read {}: {e}", path.display())))?;
    let summary: RunSummary = serde_json::from_slice(&bytes)?;
    Ok(summary)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::StagingContext;
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;
    use tempfile::TempDir;

    // ── Test flows ───────────────────────────────────────────────────────────

    struct AlwaysOk;
    struct AlwaysFail;
    struct AlwaysFailWorker;

    #[async_trait::async_trait]
    impl Flow for AlwaysOk {
        fn name(&self) -> &'static str {
            "always_ok"
        }
        async fn run(
            &self,
            _ctx: &StagingContext,
            _client: &WorkerClient,
        ) -> HarnessResult<()> {
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl Flow for AlwaysFail {
        fn name(&self) -> &'static str {
            "always_fail"
        }
        async fn run(
            &self,
            _ctx: &StagingContext,
            _client: &WorkerClient,
        ) -> HarnessResult<()> {
            Err(HarnessError::AssertionFailed {
                flow: "always_fail",
                reason: "intentional test failure".to_string(),
            })
        }
    }

    #[async_trait::async_trait]
    impl Flow for AlwaysFailWorker {
        fn name(&self) -> &'static str {
            "always_fail_worker"
        }
        async fn run(
            &self,
            _ctx: &StagingContext,
            _client: &WorkerClient,
        ) -> HarnessResult<()> {
            Err(HarnessError::Worker(crate::error::WorkerError {
                http_status: 400,
                code: Some(crate::error::EscrowCode::RefundDeadlinePassed),
                message: "test worker error".to_string(),
            }))
        }
    }

    fn test_ctx() -> StagingContext {
        StagingContext::for_testing(
            "https://staging.example.workers.dev",
            1,
            Pubkey::from_str("11111111111111111111111111111112").unwrap(),
            Pubkey::from_str("11111111111111111111111111111111").unwrap(),
        )
        .expect("for_testing succeeds with valid inputs")
    }

    fn test_client() -> WorkerClient {
        WorkerClient::new(
            url::Url::parse("https://staging.example.workers.dev").unwrap(),
        )
        .unwrap()
    }

    fn runner_in_tmp() -> (Runner, TempDir) {
        let tmp = TempDir::new().expect("tempdir");
        let runner = Runner::new(test_ctx(), test_client(), tmp.path().to_path_buf());
        (runner, tmp)
    }

    #[tokio::test]
    async fn all_passing_run_touches_last_green_and_writes_summary() {
        let (mut runner, tmp) = runner_in_tmp();
        runner.register(AlwaysOk);
        runner.register(AlwaysOk);

        let summary = runner.run_all().await.expect("run_all");

        assert!(summary.all_passed);
        assert_eq!(summary.passed, 2);
        assert_eq!(summary.failed, 0);
        assert_eq!(Runner::exit_code(&summary), 0);

        // Sentinel exists and parses.
        let last_green = tmp.path().join(".last-green");
        assert!(last_green.is_file(), "last-green sentinel must exist");
        let mtime = runner.last_green_mtime().unwrap();
        assert!(mtime.is_some());

        // summary.json exists in a timestamped subdir.
        let entries: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().collect();
        let summary_dirs: Vec<_> = entries
            .into_iter()
            .filter(|e| {
                let p = e.as_ref().unwrap().path();
                p.is_dir() && p.file_name().is_some()
            })
            .collect();
        assert_eq!(
            summary_dirs.len(),
            1,
            "exactly one timestamped run dir expected"
        );
    }

    #[tokio::test]
    async fn any_failure_does_not_touch_last_green() {
        let (mut runner, tmp) = runner_in_tmp();
        runner.register(AlwaysOk);
        runner.register(AlwaysFail);

        let summary = runner.run_all().await.expect("run_all");

        assert!(!summary.all_passed);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(Runner::exit_code(&summary), 1);

        let last_green = tmp.path().join(".last-green");
        assert!(
            !last_green.exists(),
            "last-green must NOT be written when any flow fails"
        );
        assert_eq!(runner.last_green_mtime().unwrap(), None);
    }

    #[tokio::test]
    async fn worker_error_is_recorded_with_correct_kind() {
        let (mut runner, _tmp) = runner_in_tmp();
        runner.register(AlwaysFailWorker);

        let summary = runner.run_all().await.expect("run_all");

        assert!(!summary.all_passed);
        let rec = &summary.flows[0];
        assert_eq!(rec.outcome, FlowOutcome::Failed);
        assert_eq!(rec.error_kind.as_deref(), Some("Worker"));
        assert!(rec.error.as_deref().unwrap().contains("RefundDeadlinePassed"));
    }

    #[tokio::test]
    async fn empty_run_is_not_all_passed() {
        // Zero flows → all_passed must be false (an empty run is not "green";
        // it is misconfiguration). This prevents the gate from blessing a
        // broken registration that registered nothing.
        let (runner, _tmp) = runner_in_tmp();
        let summary = runner.run_all().await.expect("run_all");
        assert!(!summary.all_passed);
        assert_eq!(summary.total, 0);
        assert_eq!(Runner::exit_code(&summary), 1);
    }

    #[tokio::test]
    async fn flows_execute_in_registration_order() {
        // Names appear in summary.flows in the order they were registered.
        let (mut runner, _tmp) = runner_in_tmp();
        runner.register(AlwaysOk);
        runner.register(AlwaysFail);
        runner.register(AlwaysOk);

        let summary = runner.run_all().await.expect("run_all");
        let names: Vec<_> = summary.flows.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["always_ok", "always_fail", "always_ok"]);
    }

    #[tokio::test]
    async fn failed_flow_does_not_abort_subsequent_flows() {
        let (mut runner, _tmp) = runner_in_tmp();
        runner.register(AlwaysFail);
        runner.register(AlwaysOk);

        let summary = runner.run_all().await.expect("run_all");
        assert_eq!(summary.flows.len(), 2);
        assert_eq!(summary.flows[0].outcome, FlowOutcome::Failed);
        assert_eq!(summary.flows[1].outcome, FlowOutcome::Passed);
    }

    #[test]
    fn summary_round_trips_through_json() {
        let summary = RunSummary {
            started_at: "2025-01-01T00:00:00+00:00".to_string(),
            worker_url: "https://example".to_string(),
            event_id: "flow-test-event".to_string(),
            total_duration_ms: 1234,
            total: 2,
            passed: 1,
            failed: 1,
            skipped: 0,
            all_passed: false,
            flows: vec![
                FlowRecord {
                    name: "ok".to_string(),
                    outcome: FlowOutcome::Passed,
                    duration_ms: 10,
                    error: None,
                    error_kind: None,
                },
                FlowRecord {
                    name: "bad".to_string(),
                    outcome: FlowOutcome::Failed,
                    duration_ms: 20,
                    error: Some("nope".to_string()),
                    error_kind: Some("AssertionFailed".to_string()),
                },
            ],
        };
        let json = serde_json::to_string(&summary).unwrap();
        let back: RunSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(back.passed, 1);
        assert_eq!(back.failed, 1);
        assert!(!back.all_passed);
        assert_eq!(back.flows[1].error_kind.as_deref(), Some("AssertionFailed"));
    }

    #[test]
    fn flow_record_skips_none_fields_in_json() {
        let rec = FlowRecord {
            name: "x".to_string(),
            outcome: FlowOutcome::Passed,
            duration_ms: 1,
            error: None,
            error_kind: None,
        };
        let json = serde_json::to_string(&rec).unwrap();
        assert!(!json.contains("error"), "None error must be skipped: {json}");
        assert!(
            !json.contains("error_kind"),
            "None error_kind must be skipped: {json}"
        );
    }

    #[test]
    fn default_results_root_points_at_crate_results() {
        let root = default_results_root();
        assert!(root.ends_with("results"));
        assert!(root.to_string_lossy().contains("flow-harness"));
    }

    #[test]
    fn exit_code_reflects_all_passed() {
        let mk = |all_passed| RunSummary {
            started_at: "x".to_string(),
            worker_url: "x".to_string(),
            event_id: "x".to_string(),
            total_duration_ms: 0,
            total: 1,
            passed: if all_passed { 1 } else { 0 },
            failed: if all_passed { 0 } else { 1 },
            skipped: 0,
            all_passed,
            flows: vec![],
        };
        assert_eq!(Runner::exit_code(&mk(true)), 0);
        assert_eq!(Runner::exit_code(&mk(false)), 1);
    }

    // Sentinel contents sanity (informational, not gate-load-bearing).
    #[tokio::test]
    async fn last_green_contents_include_iso_timestamp() {
        let (mut runner, tmp) = runner_in_tmp();
        runner.register(AlwaysOk);
        let _ = runner.run_all().await.unwrap();
        let body =
            std::fs::read_to_string(tmp.path().join(".last-green")).unwrap();
        assert!(
            body.contains("T") && body.contains("Z"),
            "expected an ISO-ish timestamp in last-green body, got: {body}"
        );
        assert!(body.contains("run_dir="));
    }

}
