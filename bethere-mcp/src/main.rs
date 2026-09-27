use bethere_mcp::{config::Config, config::ENV_AGENT_KEYPAIR, server, tools::Tools};

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("bethere-mcp: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    // A configured but unreadable keypair is a startup error, not a tool error.
    let wallet = match &config.agent_keypair {
        Some(path) => Some(flow_harness::context::load_keypair_file(
            ENV_AGENT_KEYPAIR,
            path,
        )?),
        None => None,
    };
    eprintln!(
        "bethere-mcp: api={} rpc={} wallet={}",
        config.api_url,
        config.rpc_url,
        wallet
            .as_ref()
            .map(|k| solana_sdk::signer::Signer::pubkey(k).to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    let tools = Tools::new(config, wallet)?;
    server::serve(&tools, tokio::io::stdout()).await?;
    Ok(())
}
