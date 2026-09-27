//! Environment configuration. Devnet + staging only: the guard is the
//! harness's own `validate_live_target`, so the two tools refuse the same
//! targets.

use std::path::PathBuf;

use url::Url;

use crate::error::ToolError;

pub const DEFAULT_API_URL: &str = "https://bethere-staging.solana-thailand.workers.dev";
pub const DEFAULT_RPC_URL: &str = flow_harness::context::DEFAULT_RPC_URL;

pub const ENV_API_URL: &str = "BETHERE_API_URL";
pub const ENV_RPC_URL: &str = "BETHERE_RPC_URL";
pub const ENV_AGENT_KEYPAIR: &str = "BETHERE_AGENT_KEYPAIR";
pub const ENV_MAX_DEPOSIT_USDC: &str = "BETHERE_MAX_DEPOSIT_USDC";

/// Default spend cap per deposit: 10 USDC in smallest units (6 decimals).
pub const DEFAULT_MAX_DEPOSIT_USDC: u64 = 10_000_000;

#[derive(Debug, Clone)]
pub struct Config {
    pub api_url: Url,
    pub rpc_url: Url,
    /// Path to the agent's own Solana keypair JSON. `None` leaves the
    /// read-only tools working and makes the wallet tools say what to set.
    pub agent_keypair: Option<PathBuf>,
    /// Largest deposit `pay_deposit` will sign, in USDC smallest units. The
    /// operator's limit, not the agent's: the agent cannot raise it.
    pub max_deposit_usdc: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, ToolError> {
        Self::from_values(
            std::env::var(ENV_API_URL).ok().as_deref(),
            std::env::var(ENV_RPC_URL).ok().as_deref(),
            std::env::var_os(ENV_AGENT_KEYPAIR).map(PathBuf::from),
            std::env::var(ENV_MAX_DEPOSIT_USDC).ok().as_deref(),
        )
    }

    pub fn from_values(
        api_url: Option<&str>,
        rpc_url: Option<&str>,
        agent_keypair: Option<PathBuf>,
        max_deposit_usdc: Option<&str>,
    ) -> Result<Self, ToolError> {
        let api_url = parse_url(ENV_API_URL, api_url.unwrap_or(DEFAULT_API_URL))?;
        let rpc_url = parse_url(ENV_RPC_URL, rpc_url.unwrap_or(DEFAULT_RPC_URL))?;
        flow_harness::context::validate_live_target(&api_url, &rpc_url)
            .map_err(|e| ToolError::Config(e.to_string()))?;
        let max_deposit_usdc = match max_deposit_usdc {
            None => DEFAULT_MAX_DEPOSIT_USDC,
            Some(raw) => raw
                .trim()
                .parse::<u64>()
                .map_err(|e| ToolError::Config(format!("{ENV_MAX_DEPOSIT_USDC}: {e}")))?,
        };
        Ok(Self {
            api_url,
            rpc_url,
            agent_keypair,
            max_deposit_usdc,
        })
    }
}

fn parse_url(label: &str, raw: &str) -> Result<Url, ToolError> {
    Url::parse(raw.trim_end_matches('/')).map_err(|e| ToolError::Config(format!("{label}: {e}")))
}
