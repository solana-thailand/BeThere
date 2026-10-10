//! Who signs the visitor's transactions: the test wallet made in this
//! browser, or the visitor's own wallet (Phantom, Solflare, …) set to devnet,
//! so the sandbox doubles as practice for a real event's USDC deposit
//! (.plans/045 SG3: "connect a devnet wallet or use a burner").

use super::burner::{burner_sign_and_send, confirm_signature};
use crate::pages::deposit::js_interop::{connect_wallet, sign_and_send_tx};
use crate::pages::escrow_init::check_wallet_cluster;
use crate::wallet_error::WalletResult;

/// The sandbox runs on devnet only.
const CLUSTER: &str = "devnet";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signer {
    /// The burner in this browser's storage.
    Burner,
    /// A detected Wallet Standard / injected wallet, by name.
    Wallet(String),
}

fn wallet_text(result: WalletResult) -> Result<String, String> {
    match result {
        WalletResult::Success(value) => Ok(value),
        WalletResult::Error(e) if e.is_user_rejected() => {
            Err("You cancelled in the wallet.".to_string())
        }
        WalletResult::Error(e) => Err(e.raw_message),
        WalletResult::UnknownFailure => Err("The wallet did not answer.".to_string()),
    }
}

/// Connect `name` and check it is on devnet. Returns its address.
pub async fn connect(name: &str) -> Result<String, String> {
    let address = wallet_text(connect_wallet(name).await)?;
    check_wallet_cluster(name, CLUSTER)
        .await
        .map_err(|_| "Your wallet is not on devnet. Switch it to Solana Devnet (in Phantom: Settings → Developer settings → Testnet mode → Solana Devnet), then choose it again.".to_string())?;
    Ok(address)
}

/// Sign the Worker-built transaction with `signer`, send it, and wait for
/// `confirmed`. Returns the signature.
pub async fn sign_and_send(signer: &Signer, rpc_url: &str, tx_b64: &str) -> Result<String, String> {
    match signer {
        Signer::Burner => burner_sign_and_send(rpc_url, tx_b64).await,
        Signer::Wallet(name) => {
            let signature = wallet_text(sign_and_send_tx(name, tx_b64).await)?;
            confirm_signature(rpc_url, &signature).await
        }
    }
}
