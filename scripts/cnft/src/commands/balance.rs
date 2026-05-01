//! Check SOL balance of the authority keypair.

use anyhow::Result;

use crate::config::{self, Config};

pub async fn run(cfg: &Config) -> Result<()> {
    let pubkey = cfg.pubkey();
    let balance = cfg.rpc.get_balance(&pubkey)?;
    println!("💰 Wallet: {pubkey}");
    println!("   Balance: {} SOL", config::lamports_to_sol(balance));
    Ok(())
}
