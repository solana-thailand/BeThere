//! BeThere cNFT CLI — Bubblegum V2 tree management & minting on Solana devnet.
//!
//! Usage:
//!   cargo run -- airdrop
//!   cargo run -- balance
//!   cargo run -- create-tree --preset small
//!   cargo run -- create-tree --depth 14 --buffer 64
//!   cargo run -- mint --tree <ADDRESS> --recipient <WALLET>
//!   cargo run -- tree-info --tree <ADDRESS>

mod commands;
mod config;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bethere-cnft")]
#[command(about = "BeThere cNFT tooling — Bubblegum V2 on Solana devnet")]
#[command(version)]
struct Cli {
    /// RPC URL (default: devnet)
    #[arg(long, global = true, default_value = "https://api.devnet.solana.com")]
    rpc_url: String,

    /// Authority keypair file in keys/ directory
    #[arg(long, global = true, default_value = "payer.json")]
    keypair: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Request SOL airdrop on devnet (2 SOL)
    Airdrop,

    /// Check SOL balance of the authority keypair
    Balance,

    /// Create a Bubblegum V2 Merkle tree
    CreateTree {
        /// Tree size preset: small (16K), medium (131K), large (1M)
        #[arg(long, default_value = "small")]
        preset: String,

        /// Custom max depth (overrides preset)
        #[arg(long)]
        depth: Option<u32>,

        /// Custom max buffer size (overrides preset)
        #[arg(long)]
        buffer: Option<u32>,

        /// Label for local tree registry
        #[arg(long)]
        label: Option<String>,

        /// Allow anyone to mint to this tree
        #[arg(long, default_value = "false")]
        public: bool,
    },

    /// Mint a compressed NFT using Bubblegum V2
    Mint {
        /// Merkle tree address (base58)
        #[arg(long)]
        tree: Option<String>,

        /// Tree label from local registry
        #[arg(long)]
        label: Option<String>,

        /// Recipient wallet address (default: authority keypair)
        #[arg(long)]
        recipient: Option<String>,

        /// NFT name
        #[arg(long, default_value = "BeThere Attendance Badge")]
        name: String,

        /// NFT symbol
        #[arg(long, default_value = "BETHERE")]
        symbol: String,

        /// Metadata URI
        #[arg(long, default_value = "https://bethere.solana-thailand.workers.dev/api/metadata/default")]
        uri: String,

        /// Simulate transaction before sending (dry-run)
        #[arg(long, default_value = "false")]
        simulate: bool,
    },

    /// Show tree configuration and mint count
    TreeInfo {
        /// Merkle tree address
        #[arg(long)]
        tree: Option<String>,

        /// Tree label from local registry
        #[arg(long)]
        label: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("bethere_cnft=info")
        .init();

    let cli = Cli::parse();
    let cfg = config::Config::load(&cli.rpc_url, &cli.keypair)?;

    match cli.command {
        Commands::Airdrop => commands::airdrop::run(&cfg).await,
        Commands::Balance => commands::balance::run(&cfg).await,
        Commands::CreateTree {
            preset,
            depth,
            buffer,
            label,
            public,
        } => {
            commands::create_tree::run(&cfg, preset, depth, buffer, label, public).await
        }
        Commands::Mint {
            tree,
            label,
            recipient,
            name,
            symbol,
            uri,
            simulate,
        } => commands::mint::run(&cfg, tree, label, recipient, name, symbol, uri, simulate).await,
        Commands::TreeInfo { tree, label } => {
            commands::tree_info::run(&cfg, tree, label).await
        }
    }
}
