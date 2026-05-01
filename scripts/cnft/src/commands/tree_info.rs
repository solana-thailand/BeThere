//! Show tree configuration and mint count.
//!
//! Fetches the TreeConfig account from the Bubblegum program and displays
//! the current state (creator, delegate, capacity, minted count).

use anyhow::{Context, Result};
use solana_sdk::pubkey::Pubkey;

use crate::config::{self, Config};

pub async fn run(
    cfg: &Config,
    tree_address: Option<String>,
    tree_label: Option<String>,
) -> Result<()> {
    // Resolve tree address
    let tree_addr = resolve_tree_address(tree_address, tree_label)?;
    let merkle_tree: Pubkey = tree_addr
        .parse()
        .with_context(|| format!("invalid tree address: {tree_addr}"))?;

    let (tree_authority, _) = config::derive_tree_authority(&merkle_tree);

    println!("🌳 Tree Info");
    println!("   Merkle tree: {merkle_tree}");
    println!("   Tree authority: {tree_authority}");
    println!();

    // Fetch the tree config account
    let account_data = cfg.rpc.get_account_data(&tree_authority)?;

    if account_data.len() < 88 {
        anyhow::bail!("tree config account data too short or doesn't exist");
    }

    // Parse TreeConfig (Borsh serialized):
    // Discriminator: 8 bytes (Anchor)
    // tree_creator: 32 bytes (Pubkey)
    // tree_delegate: 32 bytes (Pubkey)
    // total_capacity: 8 bytes (u64)
    // num_minted: 8 bytes (u64)
    // is_public: 1 byte (bool) at offset 88
    let discriminator = &account_data[0..8];
    let tree_creator: Pubkey = Pubkey::try_from(&account_data[8..40])?;
    let tree_delegate: Pubkey = Pubkey::try_from(&account_data[40..72])?;
    let total_capacity = u64::from_le_bytes(account_data[72..80].try_into()?);
    let num_minted = u64::from_le_bytes(account_data[80..88].try_into()?);
    let is_public = account_data.len() > 88 && account_data[88] != 0;

    println!("📋 Tree Config:");
    println!("   Tree Creator:   {tree_creator}");
    println!("   Tree Delegate:  {tree_delegate}");
    println!("   Total Capacity: {total_capacity}");
    println!("   Minted:         {num_minted}");
    println!("   Remaining:      {}", total_capacity - num_minted);
    println!("   Is Public:      {is_public}");
    println!(
        "   Discriminator:  {}",
        bs58::encode(discriminator).into_string()
    );

    let usage_pct = if total_capacity > 0 {
        (num_minted as f64 / total_capacity as f64) * 100.0
    } else {
        0.0
    };
    println!("   Usage:          {usage_pct:.2}%");

    // Progress bar
    let bar_width = 40;
    let filled = if total_capacity > 0 {
        ((num_minted as f64 / total_capacity as f64) * bar_width as f64) as usize
    } else {
        0
    };
    let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);
    println!("   [{bar}]");

    // Show local registry info
    if let Some(record) = config::find_tree_record(&tree_addr) {
        println!();
        println!("📂 Local Registry:");
        println!("   Version:    {}", record.version);
        println!("   maxDepth:   {}", record.max_depth);
        println!("   Buffer:     {}", record.max_buffer_size);
        println!("   Created:    {}", record.created_at);
        println!("   Cluster:    {}", record.cluster);
        if let Some(label) = &record.label {
            println!("   Label:      {label}");
        }
    }

    // List all registered trees
    let all_records = config::load_tree_registry();
    if all_records.len() > 1 {
        println!();
        println!("📁 All Registered Trees ({}):", all_records.len());
        for r in &all_records {
            let label = r
                .label
                .as_ref()
                .map(|l| format!(" ({l})"))
                .unwrap_or_default();
            let current = if r.address == tree_addr { " ← current" } else { "" };
            println!("   {}{label}{current}", r.address);
        }
    }

    Ok(())
}

fn resolve_tree_address(
    tree_address: Option<String>,
    tree_label: Option<String>,
) -> Result<String> {
    if let Some(addr) = tree_address {
        return Ok(addr);
    }
    if let Some(label) = tree_label {
        if let Some(record) = config::find_tree_record(&label) {
            return Ok(record.address);
        }
        anyhow::bail!("no tree found with label '{label}'");
    }
    if let Some(record) = config::latest_tree_record() {
        println!("📌 Using latest tree from registry");
        return Ok(record.address);
    }
    anyhow::bail!("no tree specified. Use --tree <address> or --label <name>")
}
