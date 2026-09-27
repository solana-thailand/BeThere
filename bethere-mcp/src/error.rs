//! Tool-level errors. Every variant becomes an MCP tool result with
//! `isError: true`, so the agent reads the message and can recover; none of
//! them is a JSON-RPC protocol error.

use std::fmt;

#[derive(Debug)]
pub enum ToolError {
    /// Missing or invalid configuration (env vars, keypair file).
    Config(String),
    /// Bad tool arguments.
    InvalidArgs(String),
    /// The BeThere API answered non-2xx; `body` is its error envelope.
    Api { status: u16, body: String },
    /// Transport, RPC or signing failure.
    Upstream(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(m) => write!(f, "configuration: {m}"),
            Self::InvalidArgs(m) => write!(f, "invalid arguments: {m}"),
            Self::Api { status, body } => write!(f, "BeThere API {status}: {body}"),
            Self::Upstream(m) => write!(f, "upstream: {m}"),
        }
    }
}

impl std::error::Error for ToolError {}

impl From<reqwest::Error> for ToolError {
    fn from(e: reqwest::Error) -> Self {
        Self::Upstream(e.to_string())
    }
}

impl From<flow_harness::HarnessError> for ToolError {
    fn from(e: flow_harness::HarnessError) -> Self {
        Self::Upstream(e.to_string())
    }
}
