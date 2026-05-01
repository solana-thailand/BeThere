//! Request SOL airdrop on devnet.

use anyhow::Result;

use crate::config::{self, Config};

pub async fn run(cfg: &Config) -> Result<()> {
    let pubkey = cfg.pubkey();
    println!("💰 Requesting 2 SOL airdrop on devnet...");
    println!("   Wallet: {pubkey}");

    let sig = cfg.rpc.request_airdrop(&pubkey, 2_000_000_000)?;
    println!("   ✅ Signature: {sig}");

    cfg.rpc.confirm_transaction(&sig)?;
    println!("   ✅ Confirmed");

    let balance = cfg.rpc.get_balance(&pubkey)?;
    println!("   💰 Balance: {} SOL", config::lamports_to_sol(balance));

    Ok(())
}
