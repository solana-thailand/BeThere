//! Solana NFT minting via the Crossmint REST API + DAS reads via Helius.
//!
//! Helius retired its one-call `mintCompressedNft` RPC method (HTTP 410
//! `-32410`), so minting now goes through Crossmint's hosted API: it custodies
//! the merkle tree, signs, and pays fees, so the Worker needs no on-chain
//! signer. Auth is header-based (`X-API-KEY`). The cluster is selected by host
//! (`staging.crossmint.com` = devnet, `www.crossmint.com` = mainnet).
//!
//! Crossmint minting is asynchronous: we POST to fire the mint, then poll the
//! NFT resource until it confirms on-chain and yields a signature + asset id.
//! DAS reads (`getAssetsByOwner`, below) still use Helius and are unaffected.

use serde::Deserialize;
use worker::{Fetch, Headers, Method, Request, RequestInit};

/// Crossmint API version segment (path-pinned by Crossmint).
const CROSSMINT_API_VERSION: &str = "2022-06-09";
/// Max confirmation polls before giving up (see [`mint_compressed_nft`]).
const CROSSMINT_MAX_POLLS: u32 = 12;
/// Delay between confirmation polls, milliseconds.
const CROSSMINT_POLL_DELAY_MS: u64 = 1200;

// ---------------------------------------------------------------------------
// Request struct
// ---------------------------------------------------------------------------

/// Parameters for minting a (compressed) NFT via Crossmint.
pub struct MintRequest<'a> {
    /// Recipient's Solana wallet address (base58). Sent as `solana:<addr>`.
    pub wallet_address: &'a str,
    /// Crossmint host: `staging.crossmint.com` (devnet) or `www.crossmint.com`.
    pub host: &'a str,
    /// Crossmint server-side API key (`X-API-KEY`).
    pub api_key: &'a str,
    /// Crossmint collection id to mint into (created once per cluster).
    pub collection_id: &'a str,
    /// NFT name (e.g. event-specific title).
    pub nft_name: &'a str,
    /// NFT image URL; omitted from metadata if empty.
    pub image_url: &'a str,
    /// NFT description (e.g. proof of attendance text).
    pub nft_description: &'a str,
    /// External URL associated with the NFT; omitted if empty.
    pub nft_external_url: &'a str,
    /// Whether to mint compressed (cNFT). Solana only; Crossmint defaults true.
    pub compressed: bool,
    /// Idempotency key (the claim token). When set alongside a KV store, a mint
    /// that fired but hasn't confirmed is resumed instead of re-fired on retry,
    /// preventing a duplicate mint after a poll timeout. Empty = no guard.
    pub idempotency_key: &'a str,
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Result of a successful NFT mint.
#[derive(Debug, Clone, Deserialize)]
pub struct MintResult {
    pub signature: String,
    pub asset_id: String,
}

// ---------------------------------------------------------------------------
// Mint compressed NFT (Crossmint)
// ---------------------------------------------------------------------------

/// Mint a (compressed) NFT via Crossmint and wait for on-chain confirmation.
///
/// Fires `POST /collections/{id}/nfts`, then polls the created NFT resource
/// until Crossmint reports `onChain.status == "success"`, returning the
/// transaction signature and asset id. Returns `Err` (which releases the claim
/// lock upstream so the attendee can retry) on misconfiguration, an API error,
/// a failed mint, or if confirmation is still pending after the poll budget.
///
/// See [`MintRequest`] for field documentation. `kv`, when present with a
/// non-empty `idempotency_key`, guards against double-mint: the created NFT id
/// is persisted before polling, so a retry after a poll timeout resumes the
/// same mint instead of firing a new one.
pub async fn mint_compressed_nft(
    req: &MintRequest<'_>,
    kv: Option<&worker::KvStore>,
) -> Result<MintResult, String> {
    if req.api_key.is_empty() {
        return Err("crossmint not configured: missing CROSSMINT_API_KEY".to_string());
    }
    if req.collection_id.is_empty() {
        return Err("crossmint not configured: missing CROSSMINT_COLLECTION_ID".to_string());
    }
    let host = if req.host.is_empty() {
        "staging.crossmint.com"
    } else {
        req.host
    };
    let base = format!(
        "https://{host}/api/{CROSSMINT_API_VERSION}/collections/{}/nfts",
        req.collection_id
    );

    // Idempotency: a pending-mint marker keyed by the claim token. Present only
    // when both a KV store and a non-empty key are supplied.
    let pending_key = (!req.idempotency_key.is_empty())
        .then(|| format!("crossmint:pending:{}", req.idempotency_key));
    let pending_kv = kv.zip(pending_key.as_ref());

    // If a mint for this key already fired but never confirmed, resume polling it
    // instead of firing a duplicate.
    let resumed_nft_id = match pending_kv {
        Some((kv, key)) => kv.get(key).text().await.ok().flatten(),
        None => None,
    };

    let nft_id = if let Some(id) = resumed_nft_id {
        tracing::info!(nft_id = %id, "crossmint: resuming in-flight mint (idempotent retry)");
        id
    } else {
        // Metadata — only well-supported fields; add image/external_url when present.
        let mut metadata = serde_json::json!({
            "name": req.nft_name,
            "description": req.nft_description,
        });
        if !req.image_url.is_empty() {
            metadata["image"] = serde_json::Value::String(req.image_url.to_string());
        }
        if !req.nft_external_url.is_empty() {
            metadata["external_url"] = serde_json::Value::String(req.nft_external_url.to_string());
        }

        let body = serde_json::json!({
            "recipient": format!("solana:{}", req.wallet_address),
            "metadata": metadata,
            "compressed": req.compressed,
        });
        let json_body = serde_json::to_string(&body)
            .map_err(|e| format!("failed to serialize mint request: {e}"))?;

        // Fire the mint.
        let post_json =
            crossmint_request(&base, Method::Post, req.api_key, Some(&json_body)).await?;

        // If Crossmint already confirmed synchronously (unlikely), short-circuit.
        if let Some(result) = parse_crossmint_success(&post_json) {
            tracing::info!(asset_id = %result.asset_id, "crossmint mint confirmed on submit");
            return Ok(result);
        }

        let id = post_json
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("crossmint mint response missing id: {post_json}"))?
            .to_string();
        // Persist the pending marker BEFORE polling so a timeout/crash can resume.
        // 24h TTL (not 1h): a slow on-chain confirmation that lands after our poll
        // budget must still be resumable by a later retry, or a fresh mint would
        // fire and the wallet would receive a duplicate badge. See
        // docs/SECURITY-FINDINGS-2026-08-13.md #5.
        if let Some((kv, key)) = pending_kv
            && let Ok(builder) = kv.put(key, &id)
        {
            let _ = builder.expiration_ttl(86_400).execute().await;
        }
        tracing::info!(nft_id = %id, "crossmint mint submitted; polling for confirmation");
        id
    };

    // Poll the NFT resource until it confirms on-chain.
    let clear_pending = || async {
        if let Some((kv, key)) = pending_kv {
            let _ = kv.delete(key).await;
        }
    };
    let poll_url = format!("{base}/{nft_id}");
    for attempt in 1..=CROSSMINT_MAX_POLLS {
        worker::Delay::from(std::time::Duration::from_millis(CROSSMINT_POLL_DELAY_MS)).await;

        let poll_json = match crossmint_request(&poll_url, Method::Get, req.api_key, None).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(attempt, error = %e, "crossmint poll failed; retrying");
                continue;
            }
        };
        // Log the raw shape on the first poll so field mapping can be verified
        // against a real response (the exact asset-id field name is unconfirmed).
        if attempt == 1 {
            tracing::info!(nft_id = %nft_id, raw = %poll_json, "crossmint poll #1 raw response");
        }

        match crossmint_status(&poll_json).as_deref() {
            Some("success") => {
                let result = parse_crossmint_success(&poll_json).ok_or_else(|| {
                    format!("crossmint reported success but no asset id/signature: {poll_json}")
                })?;
                clear_pending().await;
                return Ok(result);
            }
            Some("failed") | Some("rejected") | Some("error") => {
                // Definitive failure — clear the marker so a retry mints fresh.
                clear_pending().await;
                return Err(format!("crossmint mint failed: {poll_json}"));
            }
            _ => { /* pending — keep polling */ }
        }
    }

    // Timeout: leave the pending marker in place so the next attempt resumes.
    Err(format!(
        "crossmint mint still pending after {CROSSMINT_MAX_POLLS} polls (nft_id={nft_id})"
    ))
}

/// Send a Crossmint REST request and parse the JSON body. Non-2xx is an error.
async fn crossmint_request(
    url: &str,
    method: Method,
    api_key: &str,
    body: Option<&str>,
) -> Result<serde_json::Value, String> {
    let headers = Headers::new();
    headers
        .set("X-API-KEY", api_key)
        .map_err(|e| format!("failed to set api key header: {e:?}"))?;
    if body.is_some() {
        headers
            .set("Content-Type", "application/json")
            .map_err(|e| format!("failed to set content-type: {e:?}"))?;
    }

    let mut init = RequestInit::new();
    init.with_method(method).with_headers(headers);
    if let Some(b) = body {
        init.with_body(Some(wasm_bindgen::JsValue::from_str(b)));
    }

    let request = Request::new_with_init(url, &init)
        .map_err(|e| format!("failed to create crossmint request: {e:?}"))?;
    let mut response = Fetch::Request(request)
        .send()
        .await
        .map_err(|e| format!("crossmint request failed: {e:?}"))?;

    let status = response.status_code();
    let text = response
        .text()
        .await
        .map_err(|e| format!("failed to read crossmint response body: {e:?}"))?;
    if !(200..300).contains(&status) {
        return Err(format!("crossmint returned HTTP {status}: {text}"));
    }
    serde_json::from_str(&text).map_err(|e| format!("failed to parse crossmint response: {e}: {text}"))
}

/// Extract the on-chain status (lowercased) from a Crossmint NFT/action body.
fn crossmint_status(v: &serde_json::Value) -> Option<String> {
    v.get("onChain")
        .and_then(|o| o.get("status"))
        .and_then(|s| s.as_str())
        .or_else(|| v.get("status").and_then(|s| s.as_str()))
        .map(|s| s.to_ascii_lowercase())
}

/// Parse a confirmed Crossmint response into a [`MintResult`]. Returns `None`
/// unless the status is `success` AND an asset id is present. Field names are
/// tried defensively because Crossmint's exact keys are unconfirmed without a
/// live account (poll #1 logs the raw body to pin these down).
fn parse_crossmint_success(v: &serde_json::Value) -> Option<MintResult> {
    if crossmint_status(v).as_deref() != Some("success") {
        return None;
    }
    let onchain = v.get("onChain");
    let pick = |obj: Option<&serde_json::Value>, keys: &[&str]| -> Option<String> {
        let obj = obj?;
        keys.iter()
            .find_map(|k| obj.get(*k).and_then(|s| s.as_str()))
            .map(|s| s.to_string())
    };

    let asset_id = pick(onchain, &["assetId", "mintHash", "address"])
        .or_else(|| pick(Some(v), &["assetId"]))?;
    let signature = pick(onchain, &["txId", "signature", "transaction"])
        .or_else(|| pick(Some(v), &["txId"]))
        .unwrap_or_default();

    Some(MintResult {
        signature,
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

/// Helius JSON-RPC error object (DAS reads).
#[derive(Debug, serde::Deserialize)]
struct HeliusRpcError {
    message: String,
    code: Option<i64>,
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

// ---------------------------------------------------------------------------
// Sign-In With Solana (SIWS) signature verification
// ---------------------------------------------------------------------------

/// Verify that `signature_b58` is a valid ed25519 signature over `message`
/// produced by the secret key for `wallet_address` (the base58 public key).
///
/// `signature_b58` is base58-encoded 64 signature bytes (as returned by the
/// wallet's `signMessage`). Returns `Ok(())` on a valid signature.
pub fn verify_siws_signature(
    wallet_address: &str,
    message: &str,
    signature_b58: &str,
) -> Result<(), String> {
    use ed25519_dalek::{Signature, VerifyingKey};

    let pubkey_bytes = crate::solana_escrow::crypto::base58_decode(wallet_address)
        .map_err(|e| format!("invalid wallet public key: {e:?}"))?;
    let pubkey_arr: [u8; 32] = pubkey_bytes
        .as_slice()
        .try_into()
        .map_err(|_| format!("wallet public key must be 32 bytes, got {}", pubkey_bytes.len()))?;
    let verifying_key = VerifyingKey::from_bytes(&pubkey_arr)
        .map_err(|e| format!("wallet public key is not a valid ed25519 point: {e}"))?;

    let sig_bytes = crate::solana_escrow::crypto::base58_decode(signature_b58)
        .map_err(|e| format!("invalid signature encoding: {e:?}"))?;
    let sig_arr: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| format!("signature must be 64 bytes, got {}", sig_bytes.len()))?;
    let signature = Signature::from_bytes(&sig_arr);

    verifying_key
        .verify_strict(message.as_bytes(), &signature)
        .map_err(|_| "signature does not match wallet and message".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ed25519 vector generated offline (Node crypto) over the exact SIWS
    // challenge message below, signed by the key whose public key is PUB_B58.
    const SIWS_PUB_B58: &str = "FAe4sisG95oZ42w7buUn5qEE4TAnfTTFPiguZUHmhiF";
    const SIWS_MSG: &str =
        "BeThere Protocol Sign-In With Solana\nWallet: TEST\nNonce: deadbeef\nExpires: 1786400000";
    const SIWS_SIG_B58: &str =
        "5tYugeHRvrBvCMhC2NMdzxJxd43gkzXjcpkieC2FPrda8g2Tfb76G6cDxaiRNdMGxGiQfRCtiRon2uguwa8D73Lc";
    const SIWS_BADSIG_B58: &str =
        "5tYugeHRvrBvCMhC2NMdzxJxd43gkzXjcpkieC2FPrda8g2Tfb76G6cDxaiRNdMGxGiQfRCtiRon2uguwa8D73Lb";

    #[test]
    fn test_siws_valid_signature_accepts() {
        assert!(verify_siws_signature(SIWS_PUB_B58, SIWS_MSG, SIWS_SIG_B58).is_ok());
    }

    #[test]
    fn test_siws_tampered_signature_rejects() {
        assert!(verify_siws_signature(SIWS_PUB_B58, SIWS_MSG, SIWS_BADSIG_B58).is_err());
    }

    #[test]
    fn test_siws_wrong_message_rejects() {
        assert!(verify_siws_signature(SIWS_PUB_B58, "different message", SIWS_SIG_B58).is_err());
    }

    #[test]
    fn test_siws_placeholder_signature_rejects() {
        // The old bypass sent this literal string as the "signature".
        assert!(verify_siws_signature(SIWS_PUB_B58, SIWS_MSG, "siws_verified").is_err());
    }

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

    // ── Crossmint response parsing ──────────────────────────────────────
    // These lock the mint response mapping, which could not be verified against
    // a live Crossmint account when written (fields parsed defensively).
    use serde_json::json;

    #[test]
    fn test_crossmint_status_reads_onchain_first() {
        let v = json!({ "onChain": { "status": "Success" }, "status": "pending" });
        assert_eq!(crossmint_status(&v).as_deref(), Some("success")); // lowercased
    }

    #[test]
    fn test_crossmint_status_falls_back_to_top_level() {
        let v = json!({ "status": "PENDING" });
        assert_eq!(crossmint_status(&v).as_deref(), Some("pending"));
    }

    #[test]
    fn test_crossmint_status_absent_is_none() {
        assert_eq!(crossmint_status(&json!({ "id": "x" })), None);
    }

    #[test]
    fn test_parse_success_none_when_pending() {
        // Even with an assetId present, a non-success status must not resolve.
        let v = json!({ "onChain": { "status": "pending", "assetId": "AID", "txId": "SIG" } });
        assert!(parse_crossmint_success(&v).is_none());
    }

    #[test]
    fn test_parse_success_extracts_asset_and_signature() {
        let v = json!({ "onChain": { "status": "success", "assetId": "AID123", "txId": "SIG456" } });
        let r = parse_crossmint_success(&v).expect("should parse");
        assert_eq!(r.asset_id, "AID123");
        assert_eq!(r.signature, "SIG456");
    }

    #[test]
    fn test_parse_success_asset_id_fallbacks() {
        // mintHash is a valid asset-id source when assetId is absent.
        let v = json!({ "onChain": { "status": "success", "mintHash": "MINT789" } });
        let r = parse_crossmint_success(&v).expect("mintHash fallback");
        assert_eq!(r.asset_id, "MINT789");
        assert_eq!(r.signature, ""); // signature optional
    }

    #[test]
    fn test_parse_success_none_when_success_but_no_asset_id() {
        // Success with no recognizable asset-id field must NOT be treated as done
        // (the caller surfaces the raw body so the mapping can be fixed).
        let v = json!({ "onChain": { "status": "success", "txId": "SIG" } });
        assert!(parse_crossmint_success(&v).is_none());
    }

    #[test]
    fn test_parse_success_failed_status_is_none() {
        let v = json!({ "onChain": { "status": "failed", "assetId": "AID" } });
        assert!(parse_crossmint_success(&v).is_none());
    }
}
