//! Solana JSON-RPC client for minting compressed NFTs via Helius API.
//!
//! Uses `worker::Fetch` to call the Helius `mintCompressedNft` RPC method.
//! Auth is via query-param (`?api-key=KEY`) — no header-based auth.

use serde::Deserialize;
use worker::{Fetch, Headers, Method, Request, RequestInit};

// ---------------------------------------------------------------------------
// Request struct
// ---------------------------------------------------------------------------

/// Parameters for minting a compressed NFT.
pub struct MintRequest<'a> {
    /// Owner's Solana wallet address (base58).
    pub wallet_address: &'a str,
    /// Helius RPC base URL (without query params).
    pub rpc_url: &'a str,
    /// Helius API key, appended as `?api-key=KEY`.
    pub api_key: &'a str,
    /// Collection mint address; ignored if empty.
    pub collection_mint: &'a str,
    /// Off-chain metadata URI; ignored if empty.
    pub metadata_uri: &'a str,
    /// NFT image URL; ignored if empty.
    pub image_url: &'a str,
    /// NFT name (e.g. event-specific title).
    pub nft_name: &'a str,
    /// NFT symbol (e.g. event ticker).
    pub nft_symbol: &'a str,
    /// NFT description (e.g. proof of attendance text).
    pub nft_description: &'a str,
    /// External URL associated with the NFT.
    pub nft_external_url: &'a str,
    /// Merkle tree address. Reserved for future use.
    pub merkle_tree: &'a str,
}

/// Priority fee in microLamports per compute unit for faster transaction inclusion.
/// Reserved for future direct Bubblegum `mint_v2` calls (CLI already uses this).
/// NOT sent to Helius `mintCompressedNft` — that API rejects unknown params.
#[allow(dead_code)]
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
    #[serde(rename = "assetId", default)]
    asset_id: Option<String>,
    /// Transaction signature. May be null if confirmation is pending.
    #[serde(default)]
    signature: Option<String>,
    #[serde(default)]
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
/// See [`MintRequest`] for field documentation.
///
/// Returns [`MintResult`] with the transaction signature and asset id on success.
pub async fn mint_compressed_nft(req: &MintRequest<'_>) -> Result<MintResult, String> {
    let url = format!("{}/?api-key={}", req.rpc_url, req.api_key);

    // Build params — include collection/uri/imageUrl only when non-empty.
    // NOTE: Helius mintCompressedNft only accepts documented params.
    // "priorityFee" and "tree" cause Invalid request params errors.
    // See: https://www.helius.dev/docs/api-reference/mint/mintcompressednft
    let mut params = serde_json::json!({
        "name": req.nft_name,
        "symbol": req.nft_symbol,
        "description": req.nft_description,
        "owner": req.wallet_address,
        "externalUrl": req.nft_external_url,
        "sellerFeeBasisPoints": 0,
        "confirmTransaction": true
    });

    if !req.collection_mint.is_empty() {
        params["collection"] = serde_json::Value::String(req.collection_mint.to_string());
    }
    if !req.metadata_uri.is_empty() {
        params["uri"] = serde_json::Value::String(req.metadata_uri.to_string());
    }
    if !req.image_url.is_empty() {
        params["imageUrl"] = serde_json::Value::String(req.image_url.to_string());
    }

    // NOTE: Helius mintCompressedNft does NOT support a custom "tree" parameter.
    // It always mints to Helius' own managed Merkle tree. The merkle_tree field
    // is kept in MintRequest for future use (direct Bubblegum calls or when
    // Helius adds custom tree support).
    // See: https://www.helius.dev/docs/api-reference/mint/mintcompressednft
    let _ = req.merkle_tree; // suppress unused warning

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

    // Read raw body text, then parse
    let body_text = response
        .text()
        .await
        .map_err(|e| format!("failed to read helius response body: {e:?}"))?;

    let rpc_response: HeliusRpcResponse = serde_json::from_str(&body_text)
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
        return Err(format!(
            "helius rpc returned minted=false (asset_id={:?}, signature={:?})",
            result.asset_id, result.signature
        ));
    }

    let asset_id = result
        .asset_id
        .ok_or_else(|| "helius rpc returned no assetId".to_string())?;

    tracing::info!(
        asset_id = %asset_id,
        signature = %result.signature.as_deref().unwrap_or("pending"),
        "minted compressed nft"
    );

    Ok(MintResult {
        signature: result.signature.unwrap_or_default(),
        asset_id,
    })
}

// ---------------------------------------------------------------------------
// DAS API — getAssetsByOwner (for on-chain verification)
// ---------------------------------------------------------------------------

/// Response from Helius DAS `getAssetsByOwner` — paginated list of assets.
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
pub struct DasAssetsResponse {
    pub items: Vec<DasAsset>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

/// A single compressed NFT asset from the DAS API.
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
pub struct DasAsset {
    pub id: String,
    #[serde(default)]
    pub content: Option<DasContent>,
    #[serde(default)]
    pub grouping: Option<Vec<DasGrouping>>,
    #[serde(default, rename = "type")]
    pub asset_type: Option<String>,
}

/// Content metadata for a DAS asset (name, image, etc.).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DasContent {
    #[serde(default)]
    pub metadata: Option<DasMetadata>,
    #[serde(default)]
    pub json_uri: Option<String>,
    #[serde(default)]
    pub files: Option<Vec<DasFile>>,
}

/// Metadata within DAS content.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DasMetadata {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// File reference within DAS content.
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
pub struct DasFile {
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub mime: Option<String>,
}

/// Grouping info (which collection an NFT belongs to).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DasGrouping {
    #[serde(default)]
    pub group_key: Option<String>,
    #[serde(default)]
    pub group_value: Option<String>,
}

/// Helius DAS JSON-RPC response envelope.
#[derive(Debug, serde::Deserialize)]
struct HeliusDasResponse {
    result: Option<DasAssetsResponse>,
    error: Option<HeliusRpcError>,
}

/// Fetch all compressed NFTs owned by a wallet address using Helius DAS API.
/// Returns paginated results — caller can specify page and limit.
pub async fn get_assets_by_owner(
    rpc_url: &str,
    api_key: &str,
    wallet_address: &str,
    page: i64,
    limit: i64,
) -> Result<DasAssetsResponse, String> {
    let url = format!("{}/?api-key={}", rpc_url, api_key);

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-das",
        "method": "getAssetsByOwner",
        "params": {
            "ownerAddress": wallet_address,
            "page": page,
            "limit": limit,
            "displayOptions": {
                "showFungible": false,
                "showNativeBalance": false,
                "showInscription": false
            }
        }
    });

    let json_body = serde_json::to_string(&body)
        .map_err(|e| format!("failed to serialize DAS request: {e}"))?;

    let headers = Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("failed to set content-type: {e:?}"))?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = Request::new_with_init(&url, &init)
        .map_err(|e| format!("failed to create DAS request: {e:?}"))?;

    let mut response = Fetch::Request(request)
        .send()
        .await
        .map_err(|e| format!("helius DAS request failed: {e:?}"))?;

    let status = response.status_code();
    if !(200..300).contains(&status) {
        let body_text = response
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read body>".to_string());
        return Err(format!("helius DAS returned HTTP {status}: {body_text}"));
    }

    let body_text = response
        .text()
        .await
        .map_err(|e| format!("failed to read DAS response body: {e:?}"))?;

    let rpc_response: HeliusDasResponse = serde_json::from_str(&body_text)
        .map_err(|e| format!("failed to parse helius DAS response: {e:?}"))?;

    if let Some(err) = rpc_response.error {
        let code = err.code.map(|c| format!(" (code {c})")).unwrap_or_default();
        return Err(format!(
            "helius DAS error: {message}{code}",
            message = err.message
        ));
    }

    rpc_response
        .result
        .ok_or_else(|| "helius DAS returned no result and no error".to_string())
}

// ---------------------------------------------------------------------------
// Wallet validation
// ---------------------------------------------------------------------------

/// Validate a Solana wallet address (base58, 32-44 characters).
/// Const lookup table for base58 alphabet — zero heap allocation.
const BASE58_TABLE: &[u8; 128] = &{
    let mut table = [0u8; 128];
    let alphabet = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut i = 0;
    while i < alphabet.len() {
        table[alphabet[i] as usize] = 1;
        i += 1;
    }
    table
};

/// Validate a Solana wallet address (base58, 32-44 chars).
///
/// Returns `Ok(())` if valid, `Err` with a description otherwise.
pub fn validate_wallet_address(address: &str) -> Result<(), String> {
    let len = address.len();
    if !(32..=44).contains(&len) {
        return Err(format!(
            "invalid wallet address length: expected 32-44 chars, got {len}"
        ));
    }

    let invalid: Vec<char> = address
        .chars()
        .filter(|c| (*c as usize) >= 128 || BASE58_TABLE[*c as usize] == 0)
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
