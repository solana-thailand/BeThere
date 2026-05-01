//! Mint a compressed NFT using Bubblegum V2.
//!
//! Manually builds the `mint_v2` instruction (discriminator + Borsh-serialized
//! `MetadataArgsV2`) so it works with V2 trees on the new Bubblegum on-chain
//! program. The tree must already exist (created via create-tree command).

use anyhow::{Context, Result};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
    transaction::Transaction,
};

use crate::config::{self, Config};

// ---------------------------------------------------------------------------
// MetadataArgsV2 — manual Borsh-serializable struct
// ---------------------------------------------------------------------------

/// Mirrors the Bubblegum V2 `MetadataArgsV2` IDL type for Borsh serialization.
/// Fields must be in exact IDL order.
struct MetadataArgsV2 {
    name: String,
    symbol: String,
    uri: String,
    seller_fee_basis_points: u16,
    primary_sale_happened: bool,
    is_mutable: bool,
    /// Option<TokenStandard> where TokenStandard is an Anchor/Borsh enum (u8).
    /// NonFungible=0, FungibleAsset=1, Fungible=2, NonFungibleEdition=3
    /// Serialized as: [1, 0] for Some(NonFungible)
    token_standard: Option<u8>,
    creators: Vec<CreatorArgs>,
    /// Option<Pubkey> — just a pubkey, not the V1 Collection struct
    collection: Option<Pubkey>,
}

/// Single creator entry inside `MetadataArgsV2`.
struct CreatorArgs {
    address: Pubkey,
    verified: bool,
    share: u8,
}

/// Manual Borsh serialization for MetadataArgsV2 to match Anchor IDL format.
fn serialize_metadata_v2(meta: &MetadataArgsV2) -> Vec<u8> {
    let mut buf = Vec::with_capacity(256);

    // name: String (4-byte len + utf8)
    borsh_len_and_str(&mut buf, &meta.name);
    // symbol: String
    borsh_len_and_str(&mut buf, &meta.symbol);
    // uri: String
    borsh_len_and_str(&mut buf, &meta.uri);
    // seller_fee_basis_points: u16
    buf.extend_from_slice(&meta.seller_fee_basis_points.to_le_bytes());
    // primary_sale_happened: bool
    buf.push(meta.primary_sale_happened as u8);
    // is_mutable: bool
    buf.push(meta.is_mutable as u8);
    // token_standard: Option<u8> (Anchor Borsh enum is u8, NOT u32)
    match meta.token_standard {
        Some(v) => {
            buf.push(1); // Some
            buf.push(v); // u8 enum variant
        }
        None => buf.push(0), // None
    }
    // creators: Vec<CreatorArgs>
    let creators_len = meta.creators.len() as u32;
    buf.extend_from_slice(&creators_len.to_le_bytes());
    for creator in &meta.creators {
        buf.extend_from_slice(creator.address.as_ref()); // 32 bytes
        buf.push(creator.verified as u8);
        buf.push(creator.share);
    }
    // collection: Option<Pubkey>
    match &meta.collection {
        Some(pk) => {
            buf.push(1); // Some
            buf.extend_from_slice(pk.as_ref()); // 32 bytes
        }
        None => buf.push(0), // None
    }

    buf
}

fn borsh_len_and_str(buf: &mut Vec<u8>, s: &str) {
    let len = s.len() as u32;
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
}

/// `mint_v2` discriminator: SHA-256("global:mint_v2")[..8].
const MINT_V2_DISCRIMINATOR: [u8; 8] = [0x78, 0x79, 0x17, 0x92, 0xad, 0x6e, 0xc7, 0xcd];

/// TokenStandard::NonFungible variant value (Borsh u8).
const TOKEN_STANDARD_NON_FUNGIBLE: u8 = 0;

/// MPL Core program ID (required by mintV2 even without collections).
const MPL_CORE_PROGRAM_ID: &str = "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d";

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub async fn run(
    cfg: &Config,
    tree_address: Option<String>,
    tree_label: Option<String>,
    recipient: Option<String>,
    name: String,
    symbol: String,
    uri: String,
    simulate: bool,
) -> Result<()> {
    // Resolve tree address
    let tree_addr = resolve_tree_address(tree_address, tree_label)?;
    let merkle_tree: Pubkey = tree_addr
        .parse()
        .with_context(|| format!("invalid tree address: {tree_addr}"))?;

    // Derive tree authority PDA
    let (tree_authority, _) = config::derive_tree_authority(&merkle_tree);

    // Resolve recipient
    let payer = cfg.pubkey();
    let leaf_owner: Pubkey = match recipient {
        Some(ref addr) => addr
            .parse()
            .with_context(|| format!("invalid recipient address: {addr}"))?,
        None => payer,
    };

    // V2 program IDs (MPL compression & noop — not the legacy SPL ones)
    let compression_id: Pubkey = config::MPL_COMPRESSION_PROGRAM_ID
        .parse()
        .context("invalid MPL compression program id")?;
    let noop_id: Pubkey = config::MPL_NOOP_PROGRAM_ID
        .parse()
        .context("invalid MPL noop program id")?;
    let bubblegum_id: Pubkey = config::BUBBLEGUM_PROGRAM_ID
        .parse()
        .context("invalid bubblegum program id")?;
    let mpl_core_id: Pubkey = MPL_CORE_PROGRAM_ID
        .parse()
        .context("invalid MPL core program id")?;

    println!("🎫 Minting compressed NFT (V2)...");
    println!("   Tree: {merkle_tree}");
    println!("   Tree authority: {tree_authority}");
    println!("   Recipient: {leaf_owner}");
    println!("   Name: {name}");
    println!("   Symbol: {symbol}");
    println!("   URI: {uri}");
    println!();

    // -----------------------------------------------------------------------
    // Build MetadataArgsV2 and serialize
    // -----------------------------------------------------------------------
    let metadata = MetadataArgsV2 {
        name: name.clone(),
        symbol: symbol.clone(),
        uri: uri.clone(),
        seller_fee_basis_points: 0,
        primary_sale_happened: false,
        is_mutable: true,
        token_standard: Some(TOKEN_STANDARD_NON_FUNGIBLE),
        creators: vec![CreatorArgs {
            address: payer,
            verified: true,
            share: 100,
        }],
        collection: None,
    };

    let mut data = Vec::with_capacity(256);
    data.extend_from_slice(&MINT_V2_DISCRIMINATOR);
    let meta_bytes = serialize_metadata_v2(&metadata);
    data.extend_from_slice(&meta_bytes);
    // assetData: Option<Vec<u8>> = None
    data.push(0);
    // assetDataSchema: Option<AssetDataSchema> = None
    // AssetDataSchema is an enum (u8): Binary=0, Json=1, MsgPack=2
    data.push(0);


    // -----------------------------------------------------------------------
    // Build mint_v2 instruction
    // -----------------------------------------------------------------------
    //
    // Accounts (from IDL mintV2, in exact IDL order):
    //   0. treeAuthority      — writable, not signer
    //   1. payer              — writable, signer
    //   2. treeDelegate       — readonly, signer (optional, defaults to payer)
    //   3. collectionAuthority — readonly, signer (optional, no collection = omit)
    //   4. leafOwner          — readonly, not signer
    //   5. leafDelegate       — readonly, not signer (optional, defaults to leaf_owner)
    //   6. merkleTree         — writable, not signer
    //   7. coreCollection     — writable, not signer (optional, no collection = omit)
    //   8. mplCoreCpiSigner   — readonly, not signer (optional, no collection = omit)
    //   9. logWrapper         — MPL noop, readonly
    //  10. compressionProgram — MPL compression, readonly
    //  11. mplCoreProgram     — readonly (required even without collections)
    //  12. systemProgram      — readonly
    //
    // For Anchor optional accounts, we can either:
    //  a) Pass the program ID as a placeholder (common pattern)
    //  b) Omit trailing optional accounts
    //
    // We include all accounts up to mplCoreProgram to be safe.
    let mint_ix = Instruction {
        program_id: bubblegum_id,
        accounts: vec![
            AccountMeta::new(tree_authority, false),                    // 0: treeAuthority
            AccountMeta::new(payer, true),                              // 1: payer
            AccountMeta::new_readonly(payer, true),                     // 2: treeDelegate (defaults to payer)
            AccountMeta::new_readonly(bubblegum_id, false),             // 3: collectionAuthority (optional, program ID placeholder)
            AccountMeta::new_readonly(leaf_owner, false),               // 4: leafOwner
            AccountMeta::new_readonly(leaf_owner, false),               // 5: leafDelegate (optional, defaults to leaf_owner)
            AccountMeta::new(merkle_tree, false),                       // 6: merkleTree
            AccountMeta::new_readonly(bubblegum_id, false),             // 7: coreCollection (optional, no collection)
            AccountMeta::new_readonly(bubblegum_id, false),             // 8: mplCoreCpiSigner (optional, no collection)
            AccountMeta::new_readonly(noop_id, false),                  // 9: logWrapper
            AccountMeta::new_readonly(compression_id, false),           // 10: compressionProgram
            AccountMeta::new_readonly(mpl_core_id, false),              // 11: mplCoreProgram
            AccountMeta::new_readonly(system_program::id(), false),     // 12: systemProgram
        ],
        data,
    };

    // Add compute budget
    let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(400_000);

    // Build and send transaction
    let recent_blockhash = cfg.rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[compute_budget_ix, mint_ix],
        Some(&payer),
        &[&cfg.payer],
        recent_blockhash,
    );

    if simulate {
        println!("   🔍 Simulating transaction (dry-run)...");
        let sim_result = cfg.rpc.simulate_transaction(&tx)?;
        let sim_value = sim_result.value;

        if let Some(err) = &sim_value.err {
            println!("   ❌ Simulation failed: {err:?}");
        } else {
            println!("   ✅ Simulation succeeded");
        }

        if let Some(units_consumed) = sim_value.units_consumed {
            println!("   Compute units consumed: {units_consumed}");
        }

        if let Some(logs) = &sim_value.logs {
            println!("   Logs:");
            for log in logs {
                println!("     {log}");
            }
        }

        println!();
        println!("   ⚠️  SIMULATION ONLY — no transaction sent");
        return Ok(());
    }

    println!("   ⏳ Sending transaction...");

    let sig = cfg
        .rpc
        .send_and_confirm_transaction(&tx)
        .map_err(|err| {
            let sim_logs = simulate_for_debug_logs(&cfg.rpc, &tx);
            let mut msg = format!("transaction failed: {err}");
            if let Some(logs) = sim_logs {
                msg.push_str("\n\nSimulation debug logs:");
                for log in logs {
                    msg.push_str(&format!("\n  {log}"));
                }
            }
            anyhow::anyhow!(msg)
        })?;

    println!();
    println!("   ✅ cNFT minted successfully!");
    println!("   Signature: {sig}");
    println!("   Recipient: {leaf_owner}");

    // Compute asset ID
    // The asset ID is derived from the tree address and nonce (leaf index)
    // We fetch the tree config to get the current nonce
    match get_tree_nonce(&cfg.rpc, &merkle_tree) {
        Ok(nonce) => {
            let asset_id = mpl_bubblegum::utils::get_asset_id(&merkle_tree, nonce);
            println!("   Asset ID: {asset_id}");
            println!("   Nonce: {nonce}");
        }
        Err(e) => {
            println!("   ⚠️  Could not compute asset ID: {e}");
        }
    }

    println!();
    println!(
        "   Explorer: https://explorer.solana.com/address/{leaf_owner}?cluster=devnet"
    );

    Ok(())
}

/// Resolve tree address from CLI args or local registry.
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

    // Fallback: use latest from registry
    if let Some(record) = config::latest_tree_record() {
        println!("📌 Using latest tree from registry: {}", record.address);
        return Ok(record.address);
    }

    anyhow::bail!("no tree specified. Use --tree <address> or --label <name>")
}

/// Fetch the current nonce (numMinted) from the tree config account.
///
/// TreeConfig layout (Borsh, 8-byte discriminator prefix):
///   discriminator      8 bytes   (offset 0)
///   tree_creator      32 bytes   (offset 8)
///   tree_delegate     32 bytes   (offset 40)
///   total_capacity     8 bytes   (offset 72)
///   num_minted         8 bytes   (offset 80)
///
/// The V2 tree config appends `is_decompressible` and `version` after these
/// fields, but the nonce offset remains the same.
fn get_tree_nonce(rpc: &solana_client::rpc_client::RpcClient, merkle_tree: &Pubkey) -> Result<u64> {
    let (tree_authority, _) = config::derive_tree_authority(merkle_tree);
    let account_data = rpc.get_account_data(&tree_authority)?;

    if account_data.len() < 88 {
        anyhow::bail!("tree config account data too short");
    }

    let nonce_bytes: [u8; 8] = account_data[80..88].try_into()?;
    Ok(u64::from_le_bytes(nonce_bytes))
}

/// Attempt to simulate a transaction to extract debug logs on failure.
///
/// Returns `Some(logs)` on successful simulation, `None` if simulation itself
/// fails (e.g. blockhash expired).
fn simulate_for_debug_logs(
    rpc: &solana_client::rpc_client::RpcClient,
    tx: &Transaction,
) -> Option<Vec<String>> {
    match rpc.simulate_transaction(tx) {
        Ok(sim) => sim.value.logs,
        Err(e) => {
            tracing::warn!("failed to simulate transaction for debug logs: {e}");
            None
        }
    }
}
