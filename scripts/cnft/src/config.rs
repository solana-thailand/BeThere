//! Shared configuration: keypair loading, RPC client setup, tree registry, constants.

use anyhow::{Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use std::{fs, path::PathBuf};

// ---------------------------------------------------------------------------
// Tree Presets
// ---------------------------------------------------------------------------

/// Recommended tree configurations for different event sizes.
#[allow(dead_code)]
pub struct TreePreset {
    pub max_depth: u32,
    pub max_buffer_size: u32,
    pub capacity: u64,
    pub approx_cost_sol: &'static str,
}

pub fn get_preset(name: &str) -> Option<TreePreset> {
    match name {
        "small" => Some(TreePreset {
            max_depth: 14,
            max_buffer_size: 64,
            capacity: 16_384,
            approx_cost_sol: "~0.34",
        }),
        "medium" => Some(TreePreset {
            max_depth: 17,
            max_buffer_size: 64,
            capacity: 131_072,
            approx_cost_sol: "~1.4",
        }),
        "large" => Some(TreePreset {
            max_depth: 20,
            max_buffer_size: 1024,
            capacity: 1_048_576,
            approx_cost_sol: "~8.5",
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Program IDs
// ---------------------------------------------------------------------------

/// MPL Account Compression program ID (V2 — required by Bubblegum V2)
pub const MPL_COMPRESSION_PROGRAM_ID: &str = "mcmt6YrQEMKw8Mw43FmpRLmf7BqRnFMKmAcbxE3xkAW";

/// MPL Noop program ID (V2 — required by Bubblegum V2 on-chain program)
pub const MPL_NOOP_PROGRAM_ID: &str = "mnoopTCrg4p8ry25e4bcWA9XZjbNjMTfgYVGGEdRsf3";

/// Bubblegum program ID
pub const BUBBLEGUM_PROGRAM_ID: &str = "BGUMAp9Gq7iTEuizy4pqaxsTyUCBK68MDfK752saRPUY";

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Application configuration loaded from CLI args and keypair file.
pub struct Config {
    pub rpc: RpcClient,
    pub payer: Keypair,
    #[allow(dead_code)]
    pub keys_dir: PathBuf,
}

impl Config {
    /// Load config: create RPC client and load keypair from `keys/` directory.
    pub fn load(rpc_url: &str, keypair_filename: &str) -> Result<Self> {
        let rpc = RpcClient::new(rpc_url.to_string());

        let keys_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("keys");

        let keypair_path = keys_dir.join(keypair_filename);
        let payer = load_keypair(&keypair_path)?;

        tracing::info!("authority: {}", payer.pubkey());
        tracing::info!("rpc: {rpc_url}");

        Ok(Self {
            rpc,
            payer,
            keys_dir,
        })
    }

    /// Get the payer's public key.
    pub fn pubkey(&self) -> Pubkey {
        self.payer.pubkey()
    }
}

// ---------------------------------------------------------------------------
// Keypair Loading
// ---------------------------------------------------------------------------

/// Load a keypair from a Solana CLI JSON file `[1,2,3,...]`.
fn load_keypair(path: &PathBuf) -> Result<Keypair> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("keypair file not found: {}", path.display()))?;

    // Solana CLI format: [1,2,3,...] byte array
    let bytes: Vec<u8> = serde_json::from_str(&content)
        .with_context(|| format!("invalid keypair format in {}", path.display()))?;

    Keypair::from_bytes(&bytes).with_context(|| "invalid keypair bytes")
}

// ---------------------------------------------------------------------------
// Tree Authority PDA
// ---------------------------------------------------------------------------

/// Derive the tree authority PDA from a merkle tree address.
/// Seeds: [merkle_tree_bytes]
pub fn derive_tree_authority(merkle_tree: &Pubkey) -> (Pubkey, u8) {
    let bubblegum_id: Pubkey = BUBBLEGUM_PROGRAM_ID
        .parse()
        .expect("invalid bubblegum program id");
    Pubkey::find_program_address(&[merkle_tree.as_ref()], &bubblegum_id)
}

// ---------------------------------------------------------------------------
// Tree Registry (local JSON file)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TreeRecord {
    pub address: String,
    pub max_depth: u32,
    pub max_buffer_size: u32,
    pub version: String,
    pub created_at: String,
    pub cluster: String,
    pub label: Option<String>,
}

/// Path to the local tree registry file.
fn trees_file() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("keys")
        .join("trees.json")
}

/// Load all tree records from the registry.
pub fn load_tree_registry() -> Vec<TreeRecord> {
    let path = trees_file();
    if !path.exists() {
        return vec![];
    }
    let content = fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&content).unwrap_or_default()
}

/// Save a new tree record to the registry.
pub fn save_tree_record(record: TreeRecord) -> Result<()> {
    let mut records = load_tree_registry();
    records.push(record);
    let path = trees_file();
    let json = serde_json::to_string_pretty(&records)?;
    fs::write(&path, json)?;
    tracing::info!("tree record saved to {}", path.display());
    Ok(())
}

/// Find a tree record by address or label.
pub fn find_tree_record(query: &str) -> Option<TreeRecord> {
    let records = load_tree_registry();
    records
        .iter()
        .find(|r| r.address == query || r.label.as_deref() == Some(query))
        .cloned()
}

/// Get the latest tree from the registry.
pub fn latest_tree_record() -> Option<TreeRecord> {
    let records = load_tree_registry();
    records.last().cloned()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert lamports to SOL string.
pub fn lamports_to_sol(lamports: u64) -> String {
    format!("{:.4}", lamports as f64 / 1_000_000_000.0)
}
