//! Solana JSON-RPC client for minting compressed NFTs via Helius API.
//!
//! Uses `worker::Fetch` to call the Helius `mintCompressedNft` RPC method.
//! Auth is via query-param (`?api-key=KEY`) — no header-based auth.

use serde::Deserialize;
use worker::{Fetch, Headers, Method, Request, RequestInit};

/// Priority fee in microLamports per compute unit for faster transaction inclusion.
/// Set to 100_000 (0.1 SOL per 1M CU) to land in the leader's next block.
/// Adjust based on network congestion — higher = faster confirmation.
const PRIORITY_FEE_MICROLAMPORTS: u64 = 100_000;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Result of a successful compressed NFT mint via Helius.
#[derive(Debug, Clone, Deserialize)]
pub struct MintResult {
    pub signature: String,
    pub asset_id: String,
}

/// Helius JSON-RPC response envelope.
#[derive(Debug, Deserialize)]
struct HeliusRpcResponse {
    result: Option<HeliusMintResult>,
    error: Option<HeliusRpcError>,
}

/// Inner `result` object from Helius `mintCompressedNft`.
#[derive(Debug, Deserialize)]
struct HeliusMintResult {
    #[serde(rename = "assetId")]
    asset_id: String,
    signature: String,
    minted: bool,
}

/// Helius JSON-RPC error object.
#[derive(Debug, Deserialize)]
struct HeliusRpcError {
    message: String,
    code: Option<i64>,
}

// ---------------------------------------------------------------------------
// Mint compressed NFT
// ---------------------------------------------------------------------------

/// Mint a compressed NFT via the Helius `mintCompressedNft` JSON-RPC method.
///
/// The Helius API uses query-param auth (`?api-key=KEY`), so the URL is built
/// by appending the api key to the rpc url.
///
/// Returns [`MintResult`] with the transaction signature and asset id on success.
pub async fn mint_compressed_nft(
    wallet_address: &str,
    rpc_url: &str,
    api_key: &str,
    collection_mint: &str,
    metadata_uri: &str,
    image_url: &str,
) -> Result<MintResult, String> {
    let url = format!("{rpc_url}/?api-key={api_key}");

    // Build params — include collection/uri/imageUrl only when non-empty
    // Priority fee ensures the transaction lands in the leader's next block
    // for fastest confirmation (~400ms on a healthy network).
    let mut params = serde_json::json!({
        "name": "BeThere - Road to Mainnet",
        "symbol": "BETH",
        "description": "Proof of attendance at Road to Mainnet 1 — Bangkok",
        "owner": wallet_address,
        "externalUrl": "https://solana-thailand.workers.dev",
        "sellerFeeBasisPoints": 0,
        "confirmTransaction": true,
        "priorityFee": PRIORITY_FEE_MICROLAMPORTS
    });

    if !collection_mint.is_empty() {
        params["collection"] = serde_json::Value::String(collection_mint.to_string());
    }
    if !metadata_uri.is_empty() {
        params["uri"] = serde_json::Value::String(metadata_uri.to_string());
    }
    if !image_url.is_empty() {
        params["imageUrl"] = serde_json::Value::String(image_url.to_string());
    }

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-claim",
        "method": "mintCompressedNft",
        "params": params
    });

    let json_body = serde_json::to_string(&body)
        .map_err(|e| format!("failed to serialize mint request: {e}"))?;

    let headers = Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("failed to set content-type: {e:?}"))?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = Request::new_with_init(&url, &init)
        .map_err(|e| format!("failed to create mint request: {e:?}"))?;

    let mut response = Fetch::Request(request)
        .send()
        .await
        .map_err(|e| format!("helius mint request failed: {e:?}"))?;

    let status = response.status_code();
    if !(200..300).contains(&status) {
        let body_text = response
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read body>".to_string());
        return Err(format!("helius rpc returned HTTP {status}: {body_text}"));
    }

    let rpc_response: HeliusRpcResponse = response
        .json()
        .await
        .map_err(|e| format!("failed to parse helius rpc response: {e:?}"))?;

    if let Some(err) = rpc_response.error {
        let code = err.code.map(|c| format!(" (code {c})")).unwrap_or_default();
        return Err(format!(
            "helius rpc error: {message}{code}",
            message = err.message
        ));
    }

    let result = rpc_response
        .result
        .ok_or_else(|| "helius rpc returned no result and no error".to_string())?;

    if !result.minted {
        return Err("helius rpc returned minted=false".to_string());
    }

    tracing::info!(
        "minted compressed nft: asset_id={} signature={} priority_fee={}",
        result.asset_id,
        result.signature,
        PRIORITY_FEE_MICROLAMPORTS
    );

    Ok(MintResult {
        signature: result.signature,
        asset_id: result.asset_id,
    })
}

// ---------------------------------------------------------------------------
// Wallet validation
// ---------------------------------------------------------------------------

/// Validate a Solana wallet address (base58, 32-44 characters).
/// Returns `Ok(())` if valid, `Err` with a description otherwise.
pub fn validate_wallet_address(address: &str) -> Result<(), String> {
    let len = address.len();
    if !(32..=44).contains(&len) {
        return Err(format!(
            "invalid wallet address length: expected 32-44 chars, got {len}"
        ));
    }

    // Base58 alphabet: 123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
    let base58_chars: std::collections::HashSet<char> =
        "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
            .chars()
            .collect();

    let invalid: Vec<char> = address
        .chars()
        .filter(|c| !base58_chars.contains(c))
        .collect();
    if !invalid.is_empty() {
        return Err(format!(
            "wallet address contains invalid base58 characters: {:?}",
            invalid
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_wallet_address_valid() {
        // Typical Solana address (44 chars, base58)
        let addr = "11111111111111111111111111111111";
        assert!(validate_wallet_address(addr).is_ok());
    }

    #[test]
    fn test_validate_wallet_address_too_short() {
        let addr = "abc";
        let result = validate_wallet_address(addr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("length"));
    }

    #[test]
    fn test_validate_wallet_address_too_long() {
        let addr = "a".repeat(50);
        let result = validate_wallet_address(&addr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("length"));
    }

    #[test]
    fn test_validate_wallet_address_invalid_chars() {
        // Contains '0' which is not in base58 alphabet
        let addr = "0".repeat(40);
        let result = validate_wallet_address(&addr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid base58"));
    }

    #[test]
    fn test_validate_wallet_address_min_length() {
        // 32 chars of valid base58
        let addr: String = "1".repeat(32);
        assert!(validate_wallet_address(&addr).is_ok());
    }

    #[test]
    fn test_validate_wallet_address_max_length() {
        // 44 chars of valid base58
        let addr: String = "1".repeat(44);
        assert!(validate_wallet_address(&addr).is_ok());
    }

    #[test]
    fn test_validate_wallet_address_real_solana_address() {
        // A well-known Solana address (System Program)
        let addr = "11111111111111111111111111111111";
        assert!(validate_wallet_address(addr).is_ok());
    }
}
