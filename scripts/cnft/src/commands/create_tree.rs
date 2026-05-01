//! Create a Bubblegum Merkle tree on Solana devnet.
//!
//! Uses `CreateTreeConfigBuilder` from mpl-bubblegum to build the instruction,
//! then sends via RPC. The tree config PDA is derived from the merkle tree address.

use anyhow::{Context, Result};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::config::{self, Config, TreeRecord};

pub async fn run(
    cfg: &Config,
    preset_name: String,
    depth: Option<u32>,
    buffer: Option<u32>,
    label: Option<String>,
    public: bool,
) -> Result<()> {
    // Resolve tree parameters
    let preset = config::get_preset(&preset_name);
    if preset.is_none() && depth.is_none() {
        anyhow::bail!(
            "unknown preset '{}'. available: small, medium, large",
            preset_name
        );
    }

    let max_depth = depth.unwrap_or_else(|| preset.as_ref().unwrap().max_depth);
    let max_buffer_size = buffer.unwrap_or_else(|| preset.as_ref().unwrap().max_buffer_size);
    let capacity = 2u64.pow(max_depth);

    // Generate new merkle tree keypair
    let merkle_tree = Keypair::new();
    let merkle_tree_pubkey = merkle_tree.pubkey();

    // Derive tree authority PDA
    let (tree_authority, _bump) = config::derive_tree_authority(&merkle_tree_pubkey);

    let payer = cfg.pubkey();
    let compression_id: Pubkey = config::MPL_COMPRESSION_PROGRAM_ID
        .parse()
        .context("invalid mpl-compression program id")?;
    let noop_id: Pubkey = config::MPL_NOOP_PROGRAM_ID
        .parse()
        .context("invalid mpl-noop program id")?;

    println!("🌳 Creating Bubblegum Merkle Tree...");
    println!("   Network: devnet");
    println!("   Authority: {payer}");
    println!("   Tree address: {merkle_tree_pubkey}");
    println!("   Tree authority PDA: {tree_authority}");
    println!("   maxDepth: {max_depth}");
    println!("   maxBufferSize: {max_buffer_size}");
    println!("   Capacity: {capacity}");
    println!("   Public: {public}");
    println!();

    // Build the createTreeV2 instruction manually.
    // The on-chain Bubblegum V2 program expects the merkle tree account
    // to be pre-allocated with the right space.
    // Discriminator: SHA256("global:create_tree_v2")[..8] = 37635fd78ecbe3cd
    let discriminator: [u8; 8] = [0x37, 0x63, 0x5f, 0xd7, 0x8e, 0xcb, 0xe3, 0xcd];

    // Args: maxDepth (u32) + maxBufferSize (u32) + public (Option<bool>)
    let mut ix_data = Vec::with_capacity(18);
    ix_data.extend_from_slice(&discriminator);
    ix_data.extend_from_slice(&max_depth.to_le_bytes());
    ix_data.extend_from_slice(&max_buffer_size.to_le_bytes());
    // Borsh Option<bool>: Some(true) = [1, 1], Some(false) = [1, 0], None = [0]
    ix_data.push(if public { 1 } else { 0 }); // 0 = None for public

    // Build accounts manually based on IDL createTreeV2
    let bubblegum_id: Pubkey = config::BUBBLEGUM_PROGRAM_ID
        .parse()
        .context("invalid bubblegum program id")?;

    let create_tree_ix = solana_sdk::instruction::Instruction {
        program_id: bubblegum_id,
        accounts: vec![
            solana_sdk::instruction::AccountMeta::new(tree_authority, false),      // treeAuthority
            solana_sdk::instruction::AccountMeta::new(merkle_tree_pubkey, false),   // merkleTree (pre-allocated)
            solana_sdk::instruction::AccountMeta::new(payer, true),               // payer (writable+signer)
            // treeCreator is optional — defaults to payer, omit or include as signer
            solana_sdk::instruction::AccountMeta::new_readonly(payer, true),         // treeCreator (optional, defaults to payer)
            solana_sdk::instruction::AccountMeta::new_readonly(noop_id, false),     // logWrapper
            solana_sdk::instruction::AccountMeta::new_readonly(compression_id, false), // compressionProgram
            solana_sdk::instruction::AccountMeta::new_readonly(
                solana_sdk::system_program::id(), false
            ), // systemProgram
        ],
        data: ix_data,
    };

    // Pre-allocate the merkle tree account with the right space.
    // The account must exist with all-zero data before calling createTreeV2.
    let merkle_tree_space = calculate_merkle_tree_space(max_depth, max_buffer_size);
    let rent_lamports = cfg.rpc.get_minimum_balance_for_rent_exemption(merkle_tree_space as usize)?;
    let alloc_ix = solana_sdk::system_instruction::create_account(
        &payer,
        &merkle_tree_pubkey,
        rent_lamports,
        merkle_tree_space,
        &compression_id,
    );
    println!("   Merkle tree space: {merkle_tree_space} bytes, rent: {rent_lamports} lamports");



    // Add compute budget for tree creation (requires significant compute)
    let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_000_000);
    let compute_price_ix = ComputeBudgetInstruction::set_compute_unit_price(100_000);

    // Build and send transaction
    let recent_blockhash = cfg.rpc.get_latest_blockhash()?;
    let mut tx = Transaction::new_with_payer(
        &[compute_budget_ix, compute_price_ix, alloc_ix, create_tree_ix],
        Some(&payer),
    );
    tx.sign(&[&cfg.payer, &merkle_tree], recent_blockhash);

    println!("   ⏳ Sending transaction...");
    let sig = cfg.rpc.send_and_confirm_transaction(&tx)?;

    println!();
    println!("   ✅ Tree created successfully!");
    println!("   Signature: {sig}");
    println!("   Tree address: {merkle_tree_pubkey}");
    println!("   Tree authority: {tree_authority}");

    // Save to local registry
    config::save_tree_record(TreeRecord {
        address: merkle_tree_pubkey.to_string(),
        max_depth,
        max_buffer_size,
        version: "V2".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        cluster: "devnet".to_string(),
        label,
    })?;

    // Show remaining balance
    let balance = cfg.rpc.get_balance(&payer)?;
    println!("   💰 Remaining balance: {} SOL", config::lamports_to_sol(balance));

    println!();
    println!("   Next steps:");
    println!("   1. Mint a cNFT: cargo run -- mint --tree {merkle_tree_pubkey}");
    println!("   2. Tree info:   cargo run -- tree-info --tree {merkle_tree_pubkey}");

    Ok(())
}

/// Calculate the space needed for a concurrent merkle tree account.
/// Uses spl-account-compression's internal function for exact sizing.
fn calculate_merkle_tree_space(max_depth: u32, max_buffer_size: u32) -> u64 {
    use borsh::BorshDeserialize;
    use spl_account_compression::state::{
        merkle_tree_get_size, ConcurrentMerkleTreeHeader,
        CONCURRENT_MERKLE_TREE_HEADER_SIZE_V1,
    };

    // Create a dummy header to calculate the merkle tree size
    let header_bytes = vec![0u8; CONCURRENT_MERKLE_TREE_HEADER_SIZE_V1];
    let mut header = ConcurrentMerkleTreeHeader::try_from_slice(&header_bytes)
        .expect("header deserialize from zeros");
    let dummy_pubkey = Pubkey::new_unique();
    header.initialize(max_depth, max_buffer_size, &dummy_pubkey, 0);

    let merkle_tree_size = merkle_tree_get_size(&header)
        .expect("merkle tree size calculation");

    let total = CONCURRENT_MERKLE_TREE_HEADER_SIZE_V1 as u64 + merkle_tree_size as u64;
    tracing::info!(
        "header_size={}, merkle_tree_size={}, total={}",
        CONCURRENT_MERKLE_TREE_HEADER_SIZE_V1, merkle_tree_size, total
    );
    total
}
