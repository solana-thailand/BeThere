//! Error types for the flow-harness.
//!
//! The harness distinguishes three classes of failure because they mean very
//! different things when triaging a run:
//!
//! - [`HarnessError::AssertionFailed`] — the flow executed end-to-end but the
//!   observed state did not match the expected state. This is the harness
//!   *doing its job*: a regression has been caught. The run should exit
//!   non-zero and the result should be recorded as a flow failure.
//! - [`HarnessError::Worker`] — the staging worker returned a non-success
//!   response. Whether this is a failure depends on the flow (a negative test
//!   like `refund_pre_event_end` *expects* a worker/escrow error). Each flow
//!   decides by inspecting [`WorkerError::code`].
//! - [`HarnessError::Transport`] / [`HarnessError::Decode`] / the rest —
//!   infrastructure problems (network down, malformed JSON, bad config). These
//!   are *not* flow results; they prevent the flow from running at all and
//!   surface as harness-level errors rather than per-flow failures.
//!
//! Design note: we deliberately do NOT implement `From` for `reqwest::Error`
//! or `serde_json::Error` into `AssertionFailed`. Converting an infrastructure
//! error into an assertion failure would hide the difference between "the code
//! under test is wrong" and "the network is down", which defeats the gate.

use thiserror::Error;

/// On-chain escrow error code surfaced by the worker, mirroring the numeric
/// codes in `bethere-escrow/src/errors.rs` (see `docs/escrow_contract_surface.md`
/// §2). Used by flows that *expect* a specific revert (negative tests).
///
/// These are intentionally a subset — only the variants the harness asserts on
/// are named. Unknown codes round-trip via [`EscrowCode::Other`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscrowCode {
    /// #1 — `RefundNotYetAllowed` (`clock < event_end`).
    RefundNotYetAllowed,
    /// #4 — `AlreadyRefunded` (second refund on the same PDA).
    AlreadyRefunded,
    /// #19 — `RefundDeadlinePassed` (no-show past `refund_deadline`).
    RefundDeadlinePassed,
    /// #22 — `RefundRequiresClose` (refund ix issued without paired close).
    RefundRequiresClose,
    /// Any other numeric code. Carries the raw value for diagnostics.
    Other(u32),
}

impl EscrowCode {
    /// Parse a worker-surfaced escrow code from its numeric form.
    pub fn from_u32(raw: u32) -> Self {
        match raw {
            1 => Self::RefundNotYetAllowed,
            4 => Self::AlreadyRefunded,
            19 => Self::RefundDeadlinePassed,
            22 => Self::RefundRequiresClose,
            other => Self::Other(other),
        }
    }
}

impl std::fmt::Display for EscrowCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RefundNotYetAllowed => write!(f, "RefundNotYetAllowed(1)"),
            Self::AlreadyRefunded => write!(f, "AlreadyRefunded(4)"),
            Self::RefundDeadlinePassed => write!(f, "RefundDeadlinePassed(19)"),
            Self::RefundRequiresClose => write!(f, "RefundRequiresClose(22)"),
            Self::Other(n) => write!(f, "EscrowCode({n})"),
        }
    }
}

/// A non-success response body returned by the staging worker.
///
/// `code` is the escrow program error code when the failure originated
/// on-chain (parsed from the worker's error envelope); `message` is the
/// human-readable detail the worker surfaced. Negative-test flows match on
/// `code`; positive flows treat any `Worker` variant as a failure.
#[derive(Debug, Clone)]
pub struct WorkerError {
    pub http_status: u16,
    pub code: Option<EscrowCode>,
    pub message: String,
}

impl std::fmt::Display for WorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.code {
            Some(c) => write!(
                f,
                "worker error: HTTP {} {} — {}",
                self.http_status, c, self.message
            ),
            None => write!(
                f,
                "worker error: HTTP {} — {}",
                self.http_status, self.message
            ),
        }
    }
}

/// Top-level harness error.
#[derive(Debug, Error)]
pub enum HarnessError {
    /// A flow's assertion did not hold. `flow` is the flow name; `reason` is
    /// human-readable context for the result JSON.
    #[error("[{flow}] assertion failed: {reason}")]
    AssertionFailed { flow: &'static str, reason: String },

    /// The staging worker returned a non-success response. See [`WorkerError`]
    /// for how negative tests interpret this.
    #[error("{0}")]
    Worker(WorkerError),

    /// HTTP transport failure (connection refused, DNS, timeout). This is an
    /// infrastructure problem, not a flow result.
    #[error("transport error: {0}")]
    Transport(String),

    /// Response body could not be decoded into the expected shape. Usually a
    /// contract drift between the worker and `domain`.
    #[error("decode error: {0}")]
    Decode(String),

    /// Harness misconfiguration (missing env var, unparseable URL, missing
    /// keypair file, etc.).
    #[error("config error: {0}")]
    Config(String),

    /// A Solana-side failure (simulation revert without a parsed escrow code,
    /// RPC error, signature failure). The harness does not interpret these;
    /// it surfaces them for triage.
    #[error("solana error: {0}")]
    Solana(String),

    /// Catch-all for errors that do not fit the above. Kept so that `?` from
    /// ad-hoc callsites still compiles during scaffolding; prefer a more
    /// specific variant when adding new code paths.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<reqwest::Error> for HarnessError {
    fn from(err: reqwest::Error) -> Self {
        Self::Transport(err.to_string())
    }
}

impl From<serde_json::Error> for HarnessError {
    fn from(err: serde_json::Error) -> Self {
        Self::Decode(err.to_string())
    }
}

/// Convenient `Result` alias for harness code.
pub type HarnessResult<T> = Result<T, HarnessError>;
