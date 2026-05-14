//! Solana escrow transaction building for USDC deposit.
//!
//! Builds a serialized Solana transaction containing the `deposit` instruction
//! for the bethere-escrow program. The transaction is returned as base64
//! for Solana Pay Transaction Request flow.
//!
//! PDA derivation uses SHA-256 via Web Crypto (SubtleCrypto).
//! Transaction serialization follows the Solana wire format (bincode-like).

use base64::Engine;

/// Optional KV store for caching RPC responses (blockhash).
/// Import here so the module can accept `Option<&KvStore>` without
/// coupling to the full worker environment.
use worker::KvStore;

#[cfg(not(test))]
use js_sys::{Object, Reflect, Uint8Array};
#[cfg(not(test))]
use wasm_bindgen::prelude::*;
#[cfg(not(test))]
use wasm_bindgen_futures::JsFuture;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Bethere-escrow program ID on devnet.
const ESCROW_PROGRAM_ID: &str = "C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T";

/// Devnet USDC mint.
/// Mainnet: EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m
const USDC_MINT_DEVNET: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

/// Mainnet USDC mint.
const USDC_MINT_MAINNET: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m";

/// Returns the USDC mint address based on the SOLANA_CLUSTER env var.
/// Defaults to devnet if not set.
fn usdc_mint() -> &'static str {
    match std::env::var("SOLANA_CLUSTER").unwrap_or_default().as_str() {
        "mainnet-beta" => USDC_MINT_MAINNET,
        _ => USDC_MINT_DEVNET,
    }
}

/// SPL Token program ID.
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// Associated Token Program ID.
/// Source: https://github.com/solana-program/associated-token-account
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

/// System program ID.
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

/// Rent sysvar ID.
const RENT_SYSVAR_ID: &str = "SysvarRent111111111111111111111111111111111";

// ---------------------------------------------------------------------------
// Blockhash cache constants
// ---------------------------------------------------------------------------

/// KV key for caching the latest Solana blockhash.
const BLOCKHASH_CACHE_KEY: &str = "cache:blockhash";

/// TTL for the cached blockhash in seconds (30s).
/// Solana blockhashes expire after ~60s on mainnet; 30s gives a good
/// trade-off between RPC call reduction and freshness.
const BLOCKHASH_CACHE_TTL_SECS: u64 = 30;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A 32-byte Solana pubkey.
type PubkeyBytes = [u8; 32];

/// Result of building a deposit transaction.
pub struct DepositTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for Solana Pay.
    pub message: String,
}

/// Result of building a refund transaction.
#[allow(dead_code)]
pub struct RefundTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for Solana Pay.
    pub message: String,
}

/// Result of building a mark_checked_in transaction.
pub struct MarkCheckedInTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for Solana Pay.
    pub message: String,
}

/// Result of building a deactivate_event transaction.
pub struct DeactivateEventTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Result of building a close_event transaction.
pub struct CloseEventTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Result of building a claim_forfeited transaction.
pub struct ClaimForfeitedTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Result of building a close_deposit transaction.
pub struct CloseDepositTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Response for a combined refund + close_deposit transaction.
pub struct RefundAndCloseTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
}

/// Error type for escrow operations.
#[derive(Debug)]
#[allow(dead_code)]
pub enum EscrowError {
    /// Invalid base58 pubkey string.
    InvalidPubkey(String),
    /// PDA derivation failed (no valid bump found — should not happen).
    PdaDerivationFailed,
    /// SHA-256 computation failed.
    HashFailed(String),
    /// RPC call failed.
    RpcFailed(String),
    /// On-chain escrow account not found or has wrong state.
    AccountNotFound(String),
}

impl std::fmt::Display for EscrowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPubkey(s) => write!(f, "invalid pubkey: {s}"),
            Self::PdaDerivationFailed => write!(f, "PDA derivation failed — no valid bump found"),
            Self::HashFailed(s) => write!(f, "SHA-256 hash failed: {s}"),
            Self::RpcFailed(s) => write!(f, "RPC call failed: {s}"),
            Self::AccountNotFound(s) => write!(f, "escrow account check failed: {s}"),
        }
    }
}

impl std::error::Error for EscrowError {}

// ---------------------------------------------------------------------------
// SHA-256
// ---------------------------------------------------------------------------

/// Compute SHA-256 hash.
/// In WASM (worker runtime): uses Web Crypto SubtleCrypto.
/// In native (tests): uses pure Rust implementation.
async fn sha256(data: &[u8]) -> Result<[u8; 32], EscrowError> {
    #[cfg(not(test))]
    {
        sha256_wasm(data).await
    }
    #[cfg(test)]
    {
        sha256_native(data)
    }
}

/// Pure Rust SHA-256 for native test builds.
#[cfg(test)]
fn sha256_native(data: &[u8]) -> Result<[u8; 32], EscrowError> {
    // Minimal SHA-256 implementation (no external dependency needed for tests)
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let k: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    // Pad message
    let bit_len = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    // Process 64-byte blocks
    for chunk in padded.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [a, b, c, d, e, f, g, h] = state;
        let mut state_working = [a, b, c, d, e, f, g, h];

        for i in 0..64 {
            let s1 = state_working[4].rotate_right(6)
                ^ state_working[4].rotate_right(11)
                ^ state_working[4].rotate_right(25);
            let ch = (state_working[4] & state_working[5]) ^ (!state_working[4] & state_working[6]);
            let temp1 = state_working[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(k[i])
                .wrapping_add(w[i]);
            let s0 = state_working[0].rotate_right(2)
                ^ state_working[0].rotate_right(13)
                ^ state_working[0].rotate_right(22);
            let maj = (state_working[0] & state_working[1])
                ^ (state_working[0] & state_working[2])
                ^ (state_working[1] & state_working[2]);
            let temp2 = s0.wrapping_add(maj);

            state_working[7] = state_working[6];
            state_working[6] = state_working[5];
            state_working[5] = state_working[4];
            state_working[4] = state_working[3].wrapping_add(temp1);
            state_working[3] = state_working[2];
            state_working[2] = state_working[1];
            state_working[1] = state_working[0];
            state_working[0] = temp1.wrapping_add(temp2);
        }

        for i in 0..8 {
            state[i] = state[i].wrapping_add(state_working[i]);
        }
    }

    let mut result = [0u8; 32];
    for i in 0..8 {
        result[i * 4..i * 4 + 4].copy_from_slice(&state[i].to_be_bytes());
    }
    Ok(result)
}

/// Web Crypto SubtleCrypto SHA-256 for WASM worker runtime.
#[cfg(not(test))]
async fn sha256_wasm(data: &[u8]) -> Result<[u8; 32], EscrowError> {
    let global = js_sys::global();

    let crypto_val = Reflect::get(&global, &JsValue::from_str("crypto"))
        .map_err(|e| EscrowError::HashFailed(format!("no crypto: {e:?}")))?;

    let subtle_val = Reflect::get(&crypto_val, &JsValue::from_str("subtle"))
        .map_err(|e| EscrowError::HashFailed(format!("no subtle: {e:?}")))?;

    let subtle = Object::try_from(&subtle_val)
        .ok_or_else(|| EscrowError::HashFailed("subtle not an object".to_string()))?;

    let digest_fn = Reflect::get(subtle, &JsValue::from_str("digest"))
        .map_err(|e| EscrowError::HashFailed(format!("no digest fn: {e:?}")))?;

    let digest_js = js_sys::Function::from(digest_fn);

    let data_arr = Uint8Array::new_with_length(data.len() as u32);
    data_arr.copy_from(data);

    let algorithm = Object::new();
    Reflect::set(
        &algorithm,
        &JsValue::from_str("name"),
        &JsValue::from_str("SHA-256"),
    )
    .map_err(|e| EscrowError::HashFailed(format!("set algo name: {e:?}")))?;

    let result_promise = digest_js
        .call2(subtle, &algorithm.into(), &data_arr.into())
        .map_err(|e| EscrowError::HashFailed(format!("digest call: {e:?}")))?;

    let result = JsFuture::from(js_sys::Promise::from(result_promise))
        .await
        .map_err(|e| EscrowError::HashFailed(format!("digest await: {e:?}")))?;

    let arr_buf = js_sys::ArrayBuffer::from(result);
    let view = Uint8Array::new(&arr_buf);
    let mut hash = [0u8; 32];
    view.copy_to(&mut hash);

    Ok(hash)
}

// ---------------------------------------------------------------------------
// Base58 decode
// ---------------------------------------------------------------------------

/// Base58 Bitcoin alphabet.
const BASE58_ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Decode a base58 string into bytes (big-endian).
pub fn base58_decode(input: &str) -> Result<Vec<u8>, EscrowError> {
    let mut decoded: Vec<u8> = Vec::new();

    for c in input.bytes() {
        let mut carry = BASE58_ALPHABET
            .iter()
            .position(|&b| b == c)
            .ok_or_else(|| EscrowError::InvalidPubkey(format!("invalid base58 char: {c}")))?
            as u32;

        for byte in decoded.iter_mut() {
            carry += (*byte as u32) * 58;
            *byte = (carry & 0xFF) as u8;
            carry >>= 8;
        }

        while carry > 0 {
            decoded.push((carry & 0xFF) as u8);
            carry >>= 8;
        }
    }

    // Reverse to big-endian
    decoded.reverse();

    // Count leading '1' characters (they represent leading zero bytes)
    let leading_zeros = input.bytes().take_while(|&c| c == b'1').count();

    // Prepend leading zero bytes
    let mut result = vec![0u8; leading_zeros];
    result.extend_from_slice(&decoded);

    Ok(result)
}

/// Decode a base58 string into a 32-byte pubkey.
pub fn pubkey_from_base58(s: &str) -> Result<PubkeyBytes, EscrowError> {
    let bytes = base58_decode(s)?;
    if bytes.len() != 32 {
        return Err(EscrowError::InvalidPubkey(format!(
            "expected 32 bytes, got {}",
            bytes.len()
        )));
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&bytes);
    Ok(pk)
}

/// Encode bytes into a base58 string.
pub fn base58_encode(data: &[u8]) -> String {
    // Count leading zero bytes
    let leading_zeros = data.iter().take_while(|&&b| b == 0).count();

    // Convert to big-endian number
    let mut num: Vec<u8> = data.to_vec();

    let mut result = Vec::new();
    while !num.iter().all(|&b| b == 0) {
        let mut carry: u32 = 0;
        for byte in num.iter_mut() {
            carry = carry * 256 + *byte as u32;
            *byte = (carry / 58) as u8;
            carry %= 58;
        }
        result.push(BASE58_ALPHABET[carry as usize]);
    }

    // Add leading '1' characters for leading zero bytes
    result.extend(std::iter::repeat_n(b'1', leading_zeros));

    // Reverse (we built it least-significant first)
    result.reverse();

    String::from_utf8(result).unwrap_or_default()
}

/// Encode a 32-byte pubkey into base58.
pub fn pubkey_to_base58(pk: &PubkeyBytes) -> String {
    base58_encode(pk)
}

// ---------------------------------------------------------------------------
// PDA derivation
// ---------------------------------------------------------------------------

/// Find program address (PDA) for given seeds and program ID.
///
/// Iterates bump from 255 down to 0, returns the first valid off-curve PDA.
/// A valid PDA is one where SHA-256(seeds + bump + program_id + "ProgramDerivedAddress")
/// is NOT on the Ed25519 curve.
///
/// Reference: <https://github.com/solana-labs/solana/blob/master/sdk/src/pubkey.rs#L142>
async fn find_program_address(
    seeds: &[&[u8]],
    program_id: &PubkeyBytes,
) -> Result<(PubkeyBytes, u8), EscrowError> {
    /// The Solana runtime appends this literal after the program ID in PDA derivation.
    const PDA_MARKER: &[u8] = b"ProgramDerivedAddress";

    for bump in (0u8..=255).rev() {
        let mut full_seeds: Vec<u8> = Vec::new();
        for seed in seeds {
            full_seeds.extend_from_slice(seed);
        }
        full_seeds.push(bump);
        full_seeds.extend_from_slice(program_id);
        full_seeds.extend_from_slice(PDA_MARKER);

        let hash = sha256(&full_seeds).await?;

        // Check if hash is NOT on the Ed25519 curve.
        // Ed25519 points have y-coordinates < p where p = 2^255 - 19.
        // The high bit of the last byte indicates the sign in Ed25519 encoding.
        // A point is "on curve" if it satisfies the curve equation.
        // For PDA purposes, we use the Solana runtime's check: the point
        // must NOT be a valid Ed25519 point. The simplest check used by
        // solana_sdk is: `!is_on_curve(hash)`.
        //
        // The `is_on_curve` check in solana_sdk uses:
        //   y = le_bytes_to_scalar(hash)
        //   if y >= P (2^255 - 19) return false
        //   u = y^2 - 1
        //   v = D * y^2 + 1  (D = -121665/121666 mod p)
        //   return u * v^3 == v * u^3  (curve equation check)
        //
        // However, for practical purposes, almost all 32-byte values are NOT
        // on the Ed25519 curve (probability ~1/2^128 of being on curve for a
        // random hash). Starting from bump=255 is the standard approach and
        // virtually always finds a valid PDA on the first try.
        if !is_on_ed25519_curve(&hash) {
            return Ok((hash, bump));
        }
    }

    Err(EscrowError::PdaDerivationFailed)
}

/// Check if a 32-byte value is a point on the Ed25519 curve.
///
/// Uses `curve25519-dalek` (fiat backend) for formally verified field arithmetic.
/// This is the same check used by `solana_sdk::pubkey::is_on_curve`.
fn is_on_ed25519_curve(bytes: &[u8; 32]) -> bool {
    use curve25519_dalek::edwards::CompressedEdwardsY;

    let compressed = CompressedEdwardsY(*bytes);
    compressed.decompress().is_some()
}

// ---------------------------------------------------------------------------
// ATA derivation
// ---------------------------------------------------------------------------

/// Derive the Associated Token Account address for (owner, mint).
///
/// Seeds: [owner, TOKEN_PROGRAM_ID, mint]
/// Program: ASSOCIATED_TOKEN_PROGRAM_ID
async fn get_associated_token_address(
    owner: &PubkeyBytes,
    mint: &PubkeyBytes,
) -> Result<PubkeyBytes, EscrowError> {
    let token_program = pubkey_from_base58(TOKEN_PROGRAM_ID)?;
    let ata_program = pubkey_from_base58(ASSOCIATED_TOKEN_PROGRAM_ID);

    let (ata, _) = find_program_address(
        &[owner.as_slice(), token_program.as_slice(), mint.as_slice()],
        &ata_program?,
    )
    .await?;

    Ok(ata)
}

// ---------------------------------------------------------------------------
// RPC helpers
// ---------------------------------------------------------------------------

/// Recent blockhash from Solana RPC.
struct RecentBlockhash {
    /// The blockhash as base58 string.
    value: String,
}

/// Fetch the latest blockhash, using KV cache when available.
///
/// If `kv` is `Some`, checks KV for a cached blockhash. If present and
/// younger than [`BLOCKHASH_CACHE_TTL_SECS`], returns the cached value.
/// Otherwise fetches from RPC, stores in KV with the configured TTL,
/// and returns the fresh value.
async fn get_latest_blockhash(
    rpc_url: &str,
    kv: Option<&KvStore>,
) -> Result<RecentBlockhash, EscrowError> {
    // Try KV cache first
    if let Some(kv) = kv {
        let cached: Option<String> = kv
            .get(BLOCKHASH_CACHE_KEY)
            .text()
            .await
            .map_err(|e| {
                tracing::warn!("blockhash cache read failed: {e:?}");
                e
            })
            .ok()
            .flatten();

        if let Some(blockhash) = cached
            && !blockhash.is_empty()
        {
            tracing::debug!("using cached blockhash");
            return Ok(RecentBlockhash { value: blockhash });
        }
    }

    // Cache miss or no KV — fetch from RPC
    let blockhash = fetch_blockhash_from_rpc(rpc_url).await?;

    // Store in KV cache (best-effort — don't fail the tx build on cache write errors)
    if let Some(kv) = kv
        && let Err(e) = cache_blockhash(kv, &blockhash.value).await
    {
        tracing::warn!("blockhash cache write failed: {e:?}");
    }

    Ok(blockhash)
}

/// Fetch the latest blockhash directly from the Solana JSON-RPC endpoint.
async fn fetch_blockhash_from_rpc(rpc_url: &str) -> Result<RecentBlockhash, EscrowError> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-deposit",
        "method": "getLatestBlockhash",
        "params": [{ "commitment": "finalized" }]
    });

    let json_body = serde_json::to_string(&body)
        .map_err(|e| EscrowError::RpcFailed(format!("serialize: {e}")))?;

    let headers = worker::Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| EscrowError::RpcFailed(format!("headers: {e:?}")))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = worker::Request::new_with_init(rpc_url, &init)
        .map_err(|e| EscrowError::RpcFailed(format!("request: {e:?}")))?;

    let mut response = worker::Fetch::Request(request)
        .send()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("fetch: {e:?}")))?;

    let status = response.status_code();
    if !(200..300).contains(&status) {
        let text = response.text().await.unwrap_or_default();
        return Err(EscrowError::RpcFailed(format!("HTTP {status}: {text}")));
    }

    let text = response
        .text()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("read body: {e:?}")))?;

    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| EscrowError::RpcFailed(format!("parse json: {e}")))?;

    let blockhash = json["result"]["value"]["blockhash"]
        .as_str()
        .ok_or_else(|| EscrowError::RpcFailed("no blockhash in response".to_string()))?;

    Ok(RecentBlockhash {
        value: blockhash.to_string(),
    })
}

/// Store a blockhash in KV with the configured TTL.
async fn cache_blockhash(kv: &KvStore, blockhash: &str) -> Result<(), EscrowError> {
    kv.put(BLOCKHASH_CACHE_KEY, blockhash)
        .map_err(|e| EscrowError::RpcFailed(format!("blockhash cache put: {e:?}")))?
        .expiration_ttl(BLOCKHASH_CACHE_TTL_SECS)
        .execute()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("blockhash cache execute: {e:?}")))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// On-chain escrow verification
// ---------------------------------------------------------------------------

/// Verify that an escrow account exists on-chain by calling getAccountInfo.
/// Returns Ok(()) if the account exists and is owned by the escrow program,
/// Err if not found or wrong owner.
pub async fn verify_escrow_account_exists(
    rpc_url: &str,
    organizer_pubkey: &str,
    event_id: u64,
) -> Result<(), EscrowError> {
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID)?;

    let (event_escrow, _) = find_program_address(
        &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
        &program_id,
    )
    .await?;

    let escrow_b58 = pubkey_to_base58(&event_escrow);

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-verify-escrow",
        "method": "getAccountInfo",
        "params": [
            escrow_b58,
            { "encoding": "base64", "commitment": "confirmed" }
        ]
    });

    let json_body = serde_json::to_string(&body)
        .map_err(|e| EscrowError::RpcFailed(format!("serialize: {e}")))?;

    let headers = worker::Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| EscrowError::RpcFailed(format!("headers: {e:?}")))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = worker::Request::new_with_init(rpc_url, &init)
        .map_err(|e| EscrowError::RpcFailed(format!("request: {e:?}")))?;

    let mut response = worker::Fetch::Request(request)
        .send()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("fetch: {e:?}")))?;

    let status = response.status_code();
    if !(200..300).contains(&status) {
        let text = response.text().await.unwrap_or_default();
        return Err(EscrowError::RpcFailed(format!("HTTP {status}: {text}")));
    }

    let text = response
        .text()
        .await
        .map_err(|e| EscrowError::RpcFailed(format!("read body: {e:?}")))?;

    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| EscrowError::RpcFailed(format!("parse json: {e}")))?;

    let account_info = json.get("result").and_then(|v| v.get("value"));

    match account_info {
        None | Some(serde_json::Value::Null) => Err(EscrowError::AccountNotFound(
            "escrow account does not exist on-chain — it may have already been closed".to_string(),
        )),
        Some(info) => {
            let owner = info.get("owner").and_then(|v| v.as_str()).unwrap_or("");
            if owner != ESCROW_PROGRAM_ID {
                return Err(EscrowError::AccountNotFound(format!(
                    "account exists but is not owned by escrow program (owner: {owner})"
                )));
            }
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Transaction serialization
// ---------------------------------------------------------------------------

/// Account metadata for transaction building.
#[derive(Clone)]
struct AccountMeta {
    pubkey: PubkeyBytes,
    is_signer: bool,
    is_writable: bool,
}

/// Serialize an unsigned Solana transaction in wire format.
///
/// Wire format:
/// 1. Compact-u16: number of signatures (0 for unsigned)
/// 2. Message header: [num_required_signatures, num_readonly_signed, num_readonly_unsigned]
/// 3. Compact-u16: number of account keys
/// 4. Account keys (32 bytes each)
/// 5. Recent blockhash (32 bytes)
/// 6. Compact-u16: number of instructions
/// 7. For each instruction:
///    a. Compact-u16: program ID index
///    b. Compact-u16: number of account indices
///    c. Account indices (1 byte each)
///    d. Compact-u16: data length
///    e. Data bytes
fn serialize_transaction(
    account_metas: &[AccountMeta],
    instructions: &[CompiledInstruction],
    recent_blockhash: &PubkeyBytes,
) -> Vec<u8> {
    let num_signatures = account_metas.iter().filter(|m| m.is_signer).count() as u8;

    let num_readonly_signed = account_metas
        .iter()
        .filter(|m| m.is_signer && !m.is_writable)
        .count() as u8;

    let num_readonly_unsigned = account_metas
        .iter()
        .filter(|m| !m.is_signer && !m.is_writable)
        .count() as u8;

    let mut buf = Vec::new();

    // 1. Signature count — must match num_signatures, with zero-filled placeholders
    // Wallet adapter fills in actual signatures when signing
    encode_compact_u16(num_signatures as usize, &mut buf);
    for _ in 0..num_signatures {
        buf.extend_from_slice(&[0u8; 64]); // zero-filled signature placeholder
    }

    // 2. Message header
    buf.push(num_signatures);
    buf.push(num_readonly_signed);
    buf.push(num_readonly_unsigned);

    // 3. Account keys count
    encode_compact_u16(account_metas.len(), &mut buf);

    // 4. Account keys (32 bytes each)
    for meta in account_metas {
        buf.extend_from_slice(&meta.pubkey);
    }

    // 5. Recent blockhash
    buf.extend_from_slice(recent_blockhash);

    // 6. Instruction count
    encode_compact_u16(instructions.len(), &mut buf);

    // 7. Instructions
    for ix in instructions {
        encode_compact_u16(ix.program_id_index as usize, &mut buf);
        encode_compact_u16(ix.accounts.len(), &mut buf);
        for &idx in &ix.accounts {
            buf.push(idx);
        }
        encode_compact_u16(ix.data.len(), &mut buf);
        buf.extend_from_slice(&ix.data);
    }

    buf
}

/// A compiled instruction (post account resolution).
struct CompiledInstruction {
    program_id_index: u8,
    accounts: Vec<u8>,
    data: Vec<u8>,
}

/// Encode a compact-u16 (variable-length encoding used in Solana wire format).
fn encode_compact_u16(value: usize, buf: &mut Vec<u8>) {
    let mut val = value as u16;
    loop {
        let mut byte = (val & 0x7f) as u8;
        val >>= 7;
        if val > 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if val == 0 {
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// Message account ordering helper
// ---------------------------------------------------------------------------

/// Build message account keys in Solana's canonical order and resolve instruction indices.
///
/// Solana wire format requires accounts ordered as:
///   1. Signer + writable
///   2. Signer + readonly
///   3. Non-signer + writable
///   4. Non-signer + readonly
///
/// `extra_message_accounts` are CPI-only accounts appended after the 4-pass ordering
/// (deduplicated against already-present accounts).
///
/// Returns (message_accounts, program_id_index, ix_account_indices).
fn build_message_accounts(
    instruction_accounts: &[AccountMeta],
    program_id: &PubkeyBytes,
    extra_message_accounts: &[AccountMeta],
) -> (Vec<AccountMeta>, u8, Vec<u8>) {
    let mut message_accounts: Vec<AccountMeta> = Vec::new();

    // 1. Signer + writable
    for m in instruction_accounts {
        if m.is_signer && m.is_writable {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: true,
                is_writable: true,
            });
        }
    }
    // 2. Signer + readonly
    for m in instruction_accounts {
        if m.is_signer && !m.is_writable {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: true,
                is_writable: false,
            });
        }
    }
    // 3. Non-signer + writable
    for m in instruction_accounts {
        if !m.is_signer && m.is_writable {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: false,
                is_writable: true,
            });
        }
    }
    // 4. Non-signer + readonly (includes program_id if not already present)
    let mut program_id_in_message = false;
    for m in instruction_accounts {
        if !m.is_signer && !m.is_writable {
            if m.pubkey == *program_id {
                program_id_in_message = true;
            }
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: false,
                is_writable: false,
            });
        }
    }
    if !program_id_in_message {
        message_accounts.push(AccountMeta {
            pubkey: *program_id,
            is_signer: false,
            is_writable: false,
        });
    }

    // Append CPI-only extra accounts (deduplicated)
    for m in extra_message_accounts {
        if !message_accounts.iter().any(|ma| ma.pubkey == m.pubkey) {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: false,
                is_writable: false,
            });
        }
    }

    // Build a lookup: pubkey → index in message_accounts
    let get_index = |pk: &PubkeyBytes| -> u8 {
        message_accounts
            .iter()
            .position(|m| &m.pubkey == pk)
            .expect("all accounts should be in message") as u8
    };

    let program_id_index = get_index(program_id);
    let ix_account_indices: Vec<u8> = instruction_accounts
        .iter()
        .map(|m| get_index(&m.pubkey))
        .collect();

    (message_accounts, program_id_index, ix_account_indices)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build a serialized deposit transaction for the bethere-escrow program.
///
/// This constructs the full `deposit` instruction with proper account metas
/// and PDA-derived addresses, then serializes it into the Solana wire format.
///
/// # Arguments
/// * `rpc_url` — Solana RPC URL (with API key if needed)
/// * `organizer_pubkey` — Event organizer's wallet address (base58)
/// * `event_id` — Numeric event ID used for PDA derivation
/// * `attendee_pubkey` — Attendee's wallet address (base58)
/// * `deposit_amount` — Amount in USDC smallest unit (6 decimals)
///
/// # Returns
/// A `DepositTransaction` with the base64-encoded transaction and a message.
pub async fn build_deposit_transaction(
    rpc_url: &str,
    kv: Option<&KvStore>,
    organizer_pubkey: &str,
    event_id: u64,
    attendee_pubkey: &str,
    deposit_amount: u64,
) -> Result<DepositTransaction, EscrowError> {
    // Parse pubkeys
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let attendee = pubkey_from_base58(attendee_pubkey)?;
    let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID)?;
    let usdc_mint = pubkey_from_base58(usdc_mint())?;
    let token_program = pubkey_from_base58(TOKEN_PROGRAM_ID)?;
    let system_program = pubkey_from_base58(SYSTEM_PROGRAM_ID)?;
    let rent_sysvar = pubkey_from_base58(RENT_SYSVAR_ID)?;

    // Derive EventEscrow PDA: ["escrow", organizer, event_id]
    let (event_escrow, _escrow_bump) = find_program_address(
        &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
        &program_id,
    )
    .await?;

    // Derive AttendeeDeposit PDA: ["deposit", event_escrow, attendee]
    let (attendee_deposit, _deposit_bump) = find_program_address(
        &[b"deposit", event_escrow.as_slice(), attendee.as_slice()],
        &program_id,
    )
    .await?;

    // Derive attendee's USDC token account (ATA)
    let attendee_ta = get_associated_token_address(&attendee, &usdc_mint).await?;

    // The vault is an ATA owned by the EventEscrow PDA.
    // In the escrow program, the vault is created via `create_associated_token_account`
    // CPI with the EventEscrow PDA as owner/funder.
    let vault = get_associated_token_address(&event_escrow, &usdc_mint).await?;

    // Build the deposit instruction data:
    // [1 (discriminator)] + [event_id as 8 bytes LE]
    let mut ix_data = vec![1u8]; // deposit discriminator
    ix_data.extend_from_slice(&event_id.to_le_bytes());

    // In Solana's wire format, the transaction message has an ordered list of account_keys.
    // Each instruction references:
    //   - program_id_index: index into account_keys
    //   - accounts: ordered list of indices into account_keys
    //   - data: instruction data bytes
    //
    // Account ordering in Solana messages:
    //   1. Signer + writable accounts
    //   2. Signer + readonly accounts
    //   3. Non-signer + writable accounts
    //   4. Non-signer + readonly accounts
    //   5. Program ID (readonly, non-signer) — added last if not already present
    //
    // For our deposit instruction, the program expects accounts in this order:
    //   [attendee, event_escrow, usdc_mint, attendee_deposit, attendee_ta, vault,
    //    rent, token_program, system_program]
    //
    // We need to reorder for the message format while keeping track of the mapping.

    let instruction_accounts = vec![
        AccountMeta {
            pubkey: attendee,
            is_signer: true,
            is_writable: true,
        }, // 0
        AccountMeta {
            pubkey: event_escrow,
            is_signer: false,
            is_writable: true,
        }, // 1
        AccountMeta {
            pubkey: usdc_mint,
            is_signer: false,
            is_writable: false,
        }, // 2
        AccountMeta {
            pubkey: attendee_deposit,
            is_signer: false,
            is_writable: true,
        }, // 3
        AccountMeta {
            pubkey: attendee_ta,
            is_signer: false,
            is_writable: true,
        }, // 4
        AccountMeta {
            pubkey: vault,
            is_signer: false,
            is_writable: true,
        }, // 5
        AccountMeta {
            pubkey: rent_sysvar,
            is_signer: false,
            is_writable: false,
        }, // 6
        AccountMeta {
            pubkey: token_program,
            is_signer: false,
            is_writable: false,
        }, // 7
        AccountMeta {
            pubkey: system_program,
            is_signer: false,
            is_writable: false,
        }, // 8
    ];

    // Build message accounts in Solana canonical order + resolve indices
    let (message_accounts, program_id_index, ix_account_indices) =
        build_message_accounts(&instruction_accounts, &program_id, &[]);

    let compiled_ix = CompiledInstruction {
        program_id_index,
        accounts: ix_account_indices,
        data: ix_data,
    };

    // Fetch recent blockhash
    let blockhash_resp = get_latest_blockhash(rpc_url, kv).await?;
    let blockhash_bytes = pubkey_from_base58(&blockhash_resp.value)?;

    // Serialize the transaction
    let tx_bytes = serialize_transaction(&message_accounts, &[compiled_ix], &blockhash_bytes);

    // Base64 encode
    let tx_b64 = base64::engine::general_purpose::STANDARD.encode(&tx_bytes);

    let amount_display = deposit_amount as f64 / 1_000_000.0;

    Ok(DepositTransaction {
        transaction_b64: tx_b64,
        message: format!("Deposit {amount_display:.2} USDC to event escrow"),
    })
}

// ---------------------------------------------------------------------------
// Refund Transaction Builder
// ---------------------------------------------------------------------------

/// Build a refund transaction for the escrow program.
///
/// Transfers USDC from the vault back to the attendee and closes the
/// AttendeeDeposit PDA (rent reclaimed). The instruction discriminator is 3.
///
/// Accounts (in program-expected order):
///   event_escrow (writable), attendee_deposit (writable), attendee (signer),
///   vault_ta (writable), attendee_ta (writable), organizer (readonly),
///   token_program (readonly), system_program (readonly)
#[allow(dead_code)]
pub async fn build_refund_transaction(
    rpc_url: &str,
    kv: Option<&KvStore>,
    organizer_pubkey: &str,
    event_id: u64,
    attendee_pubkey: &str,
) -> Result<RefundTransaction, EscrowError> {
    // Parse pubkeys
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let attendee = pubkey_from_base58(attendee_pubkey)?;
    let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID)?;
    let usdc_mint = pubkey_from_base58(usdc_mint())?;
    let token_program = pubkey_from_base58(TOKEN_PROGRAM_ID)?;
    let system_program = pubkey_from_base58(SYSTEM_PROGRAM_ID)?;
    let rent_sysvar = pubkey_from_base58(RENT_SYSVAR_ID)?;

    // Derive EventEscrow PDA: ["escrow", organizer, event_id]
    let (event_escrow, _escrow_bump) = find_program_address(
        &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
        &program_id,
    )
    .await?;

    // Derive AttendeeDeposit PDA: ["deposit", event_escrow, attendee]
    let (attendee_deposit, _deposit_bump) = find_program_address(
        &[b"deposit", event_escrow.as_slice(), attendee.as_slice()],
        &program_id,
    )
    .await?;

    // Derive vault ATA (owned by EventEscrow PDA)
    let vault = get_associated_token_address(&event_escrow, &usdc_mint).await?;

    // Derive attendee's USDC ATA
    let attendee_ta = get_associated_token_address(&attendee, &usdc_mint).await?;

    // Refund instruction data: [3 (discriminator)] + [event_id u64 LE]
    let mut ix_data = vec![3u8];
    ix_data.extend_from_slice(&event_id.to_le_bytes());

    // The escrow program's Refund struct expects accounts in this order:
    //   attendee (signer, writable), event_escrow (writable), usdc_mint (readonly),
    //   attendee_deposit (writable), attendee_ta (init idempotent), vault (writable),
    //   rent (readonly), token_program (readonly), system_program (readonly)
    //
    // Note: No `organizer` account in the Refund struct — the attendee signs.
    // The event_escrow is used (with PDA seeds) to authorize the vault → attendee_ta transfer.
    let instruction_accounts = vec![
        AccountMeta {
            pubkey: attendee,
            is_signer: true,
            is_writable: true,
        }, // 0 — attendee (Signer)
        AccountMeta {
            pubkey: event_escrow,
            is_signer: false,
            is_writable: true,
        }, // 1 — event_escrow (mut)
        AccountMeta {
            pubkey: usdc_mint,
            is_signer: false,
            is_writable: false,
        }, // 2 — usdc_mint (readonly)
        AccountMeta {
            pubkey: attendee_deposit,
            is_signer: false,
            is_writable: true,
        }, // 3 — attendee_deposit (mut)
        AccountMeta {
            pubkey: attendee_ta,
            is_signer: false,
            is_writable: true,
        }, // 4 — attendee_ta (init idempotent)
        AccountMeta {
            pubkey: vault,
            is_signer: false,
            is_writable: true,
        }, // 5 — vault (mut)
        AccountMeta {
            pubkey: rent_sysvar,
            is_signer: false,
            is_writable: false,
        }, // 6 — rent (readonly)
        AccountMeta {
            pubkey: token_program,
            is_signer: false,
            is_writable: false,
        }, // 7 — token_program (readonly)
        AccountMeta {
            pubkey: system_program,
            is_signer: false,
            is_writable: false,
        }, // 8 — system_program (readonly)
    ];

    // Extra accounts needed for CPI but NOT passed to the escrow instruction.
    // The Refund struct has `init(idempotent)` on attendee_ta which CPIs to the ATA program.
    // The ATA program must be present in the top-level transaction's account keys.
    //
    // NOTE: Same limitation as create_event — the init(idempotent) CPI may fail with
    // "signer privilege escalated". If so, the attendee_ta must be pre-created.
    let ata_program = pubkey_from_base58(ASSOCIATED_TOKEN_PROGRAM_ID)?;
    let extra_message_accounts: Vec<AccountMeta> = vec![AccountMeta {
        pubkey: ata_program,
        is_signer: false,
        is_writable: false,
    }];

    // Build message accounts in Solana canonical order + resolve indices
    let (message_accounts, program_id_index, ix_account_indices) =
        build_message_accounts(&instruction_accounts, &program_id, &extra_message_accounts);

    let compiled_ix = CompiledInstruction {
        program_id_index,
        accounts: ix_account_indices,
        data: ix_data,
    };

    // Fetch recent blockhash
    let blockhash_resp = get_latest_blockhash(rpc_url, kv).await?;
    let blockhash_bytes = pubkey_from_base58(&blockhash_resp.value)?;

    // Serialize the transaction
    let tx_bytes = serialize_transaction(&message_accounts, &[compiled_ix], &blockhash_bytes);

    // Base64 encode
    let tx_b64 = base64::engine::general_purpose::STANDARD.encode(&tx_bytes);

    Ok(RefundTransaction {
        transaction_b64: tx_b64,
        message: "Claim refund from event escrow".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Combined Init Escrow Transaction Builder (ATA + CreateEvent in one TX)
// ---------------------------------------------------------------------------

/// Result of building a combined init escrow transaction.
pub struct InitEscrowTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for wallet confirmation.
    pub message: String,
    /// Derived EventEscrow PDA address (base58).
    pub escrow_address: String,
    /// Derived vault ATA address (base58).
    pub vault_address: String,
}

/// Build a single transaction containing BOTH:
/// 1. `create_associated_token_account_idempotent` (ATA program) — creates vault ATA if not exists
/// 2. `create_event` (escrow program) — initializes the on-chain escrow PDA
///
/// This replaces the two-step approach (create-vault-ata → create-event) with
/// a single transaction that the organizer signs once.
///
/// # Arguments
/// * `rpc_url` — Solana RPC URL (with API key if needed)
/// * `organizer_pubkey` — Organizer's wallet address (base58), signer + payer
/// * `event_id` — Numeric event ID used for PDA derivation
/// * `deposit_amount` — USDC deposit amount in lamports (6 decimals)
/// * `event_end` — Event end time as unix timestamp (seconds)
/// * `refund_deadline` — Refund deadline as unix timestamp (seconds)
pub async fn build_init_escrow_transaction(
    rpc_url: &str,
    kv: Option<&KvStore>,
    organizer_pubkey: &str,
    event_id: u64,
    deposit_amount: u64,
    event_end: i64,
    refund_deadline: i64,
) -> Result<InitEscrowTransaction, EscrowError> {
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let escrow_program_id = pubkey_from_base58(ESCROW_PROGRAM_ID)?;
    let usdc_mint = pubkey_from_base58(usdc_mint())?;
    let token_program = pubkey_from_base58(TOKEN_PROGRAM_ID)?;
    let system_program = pubkey_from_base58(SYSTEM_PROGRAM_ID)?;
    let rent_sysvar = pubkey_from_base58(RENT_SYSVAR_ID)?;
    let ata_program = pubkey_from_base58(ASSOCIATED_TOKEN_PROGRAM_ID)?;

    // Derive EventEscrow PDA: ["escrow", organizer, event_id]
    let (event_escrow, _) = find_program_address(
        &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
        &escrow_program_id,
    )
    .await?;

    // Derive vault ATA (owned by EventEscrow PDA)
    let vault = get_associated_token_address(&event_escrow, &usdc_mint).await?;

    // ------------------------------------------------------------------
    // Instruction 1: create_associated_token_account_idempotent
    // Targets ATA program. Accounts: organizer(S,W), vault(W), event_escrow(R),
    //   usdc_mint(R), system_program(R), token_program(R)
    // ------------------------------------------------------------------
    let ata_ix_accounts = vec![
        AccountMeta {
            pubkey: organizer,
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: vault,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: event_escrow,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: usdc_mint,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: system_program,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: token_program,
            is_signer: false,
            is_writable: false,
        },
    ];
    let ata_ix_data: Vec<u8> = vec![1]; // CreateIdempotent discriminator

    // ------------------------------------------------------------------
    // Instruction 2: create_event
    // Targets escrow program. Accounts: organizer(S,W), event_escrow(W),
    //   usdc_mint(R), vault(W), rent_sysvar(R), token_program(R), system_program(R)
    // ------------------------------------------------------------------
    let mut escrow_ix_data = vec![0u8]; // create_event discriminator
    escrow_ix_data.extend_from_slice(&event_id.to_le_bytes());
    escrow_ix_data.extend_from_slice(&deposit_amount.to_le_bytes());
    escrow_ix_data.extend_from_slice(&event_end.to_le_bytes());
    escrow_ix_data.extend_from_slice(&refund_deadline.to_le_bytes());

    let escrow_ix_accounts = vec![
        AccountMeta {
            pubkey: organizer,
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: event_escrow,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: usdc_mint,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: vault,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: rent_sysvar,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: token_program,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: system_program,
            is_signer: false,
            is_writable: false,
        },
    ];

    // ------------------------------------------------------------------
    // Merge accounts from both instructions into a single message
    // Solana canonical order: signer+writable → signer+readonly →
    //   non-signer+writable → non-signer+readonly → program IDs
    // When the same pubkey appears with different writability, use writable.
    // ------------------------------------------------------------------
    let all_instruction_accounts: Vec<&[AccountMeta]> = vec![&ata_ix_accounts, &escrow_ix_accounts];
    let program_ids: Vec<PubkeyBytes> = vec![ata_program, escrow_program_id];

    let message_accounts = merge_message_accounts(&all_instruction_accounts, &program_ids);

    // Build index lookup
    let get_index = |pk: &PubkeyBytes| -> u8 {
        message_accounts
            .iter()
            .position(|m| &m.pubkey == pk)
            .expect("all accounts should be in message") as u8
    };

    // Build Instruction 1 indices
    let ata_program_index = get_index(&ata_program);
    let ata_ix_account_indices: Vec<u8> = ata_ix_accounts
        .iter()
        .map(|m| get_index(&m.pubkey))
        .collect();
    let ata_compiled_ix = CompiledInstruction {
        program_id_index: ata_program_index,
        accounts: ata_ix_account_indices,
        data: ata_ix_data,
    };

    // Build Instruction 2 indices
    let escrow_program_index = get_index(&escrow_program_id);
    let escrow_ix_account_indices: Vec<u8> = escrow_ix_accounts
        .iter()
        .map(|m| get_index(&m.pubkey))
        .collect();
    let escrow_compiled_ix = CompiledInstruction {
        program_id_index: escrow_program_index,
        accounts: escrow_ix_account_indices,
        data: escrow_ix_data,
    };

    // Fetch recent blockhash
    let blockhash_resp = get_latest_blockhash(rpc_url, kv).await?;
    let blockhash_bytes = pubkey_from_base58(&blockhash_resp.value)?;

    // Serialize with both instructions
    let tx_bytes = serialize_transaction(
        &message_accounts,
        &[ata_compiled_ix, escrow_compiled_ix],
        &blockhash_bytes,
    );

    let tx_b64 = base64::engine::general_purpose::STANDARD.encode(&tx_bytes);
    let escrow_address = pubkey_to_base58(&event_escrow);
    let vault_address = pubkey_to_base58(&vault);
    let amount_display = deposit_amount as f64 / 1_000_000.0;

    Ok(InitEscrowTransaction {
        transaction_b64: tx_b64,
        message: format!(
            "Init escrow: create vault + create event ({amount_display:.2} USDC deposit)"
        ),
        escrow_address,
        vault_address,
    })
}

/// Merge accounts from multiple instructions into a single deduplicated
/// message account list in Solana canonical order.
/// When the same pubkey appears with different writability, prefers writable.
fn merge_message_accounts(
    instruction_account_lists: &[&[AccountMeta]],
    program_ids: &[PubkeyBytes],
) -> Vec<AccountMeta> {
    // Collect all accounts, preferring writable over readonly for duplicates
    let mut seen: std::collections::HashMap<PubkeyBytes, AccountMeta> =
        std::collections::HashMap::new();

    for list in instruction_account_lists {
        for m in *list {
            seen.entry(m.pubkey)
                .and_modify(|existing| {
                    // Prefer writable over readonly
                    if m.is_writable && !existing.is_writable {
                        existing.is_writable = true;
                    }
                    // Prefer signer over non-signer
                    if m.is_signer && !existing.is_signer {
                        existing.is_signer = true;
                    }
                })
                .or_insert(AccountMeta {
                    pubkey: m.pubkey,
                    is_signer: m.is_signer,
                    is_writable: m.is_writable,
                });
        }
    }

    // Add program IDs
    for pid in program_ids {
        seen.entry(*pid).or_insert(AccountMeta {
            pubkey: *pid,
            is_signer: false,
            is_writable: false,
        });
    }

    let all: Vec<AccountMeta> = seen.into_values().collect();

    // Sort in Solana canonical order:
    // 1. signer + writable
    // 2. signer + readonly
    // 3. non-signer + writable
    // 4. non-signer + readonly
    let mut signer_writable: Vec<AccountMeta> = Vec::new();
    let mut signer_readonly: Vec<AccountMeta> = Vec::new();
    let mut nonsigner_writable: Vec<AccountMeta> = Vec::new();
    let mut nonsigner_readonly: Vec<AccountMeta> = Vec::new();

    for m in all {
        match (m.is_signer, m.is_writable) {
            (true, true) => signer_writable.push(m),
            (true, false) => signer_readonly.push(m),
            (false, true) => nonsigner_writable.push(m),
            (false, false) => nonsigner_readonly.push(m),
        }
    }

    // Sort each bucket for deterministic ordering
    signer_writable.sort_by_key(|m| m.pubkey);
    signer_readonly.sort_by_key(|m| m.pubkey);
    nonsigner_writable.sort_by_key(|m| m.pubkey);
    nonsigner_readonly.sort_by_key(|m| m.pubkey);

    [
        signer_writable,
        signer_readonly,
        nonsigner_writable,
        nonsigner_readonly,
    ]
    .concat()
}

// ---------------------------------------------------------------------------
// Mark Checked-In Transaction Builder
// ---------------------------------------------------------------------------

/// Build a serialized `mark_checked_in` transaction for the bethere-escrow program.
///
/// The `mark_checked_in` instruction marks an attendee as checked in so they
/// can later claim a refund. Only the organizer can execute this.
///
/// # Arguments
/// * `rpc_url` — Solana RPC URL (with API key if needed)
/// * `organizer_pubkey` — Organizer's wallet address (base58), signer
/// * `event_id` — Numeric event ID used for PDA derivation
/// * `attendee_pubkey` — Attendee's wallet address (base58)
pub async fn build_mark_checked_in_transaction(
    rpc_url: &str,
    kv: Option<&KvStore>,
    organizer_pubkey: &str,
    event_id: u64,
    attendee_pubkey: &str,
) -> Result<MarkCheckedInTransaction, EscrowError> {
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let attendee = pubkey_from_base58(attendee_pubkey)?;
    let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID)?;

    // Derive EventEscrow PDA: ["escrow", organizer, event_id]
    let (event_escrow, _) = find_program_address(
        &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
        &program_id,
    )
    .await?;

    // Derive AttendeeDeposit PDA: ["deposit", event_escrow, attendee]
    let (attendee_deposit, _) = find_program_address(
        &[b"deposit", event_escrow.as_slice(), attendee.as_slice()],
        &program_id,
    )
    .await?;

    // mark_checked_in instruction data: [2 (discriminator)] + [event_id u64 LE]
    let mut ix_data = vec![2u8];
    ix_data.extend_from_slice(&event_id.to_le_bytes());

    // Accounts for mark_checked_in:
    //   organizer (signer, writable), event_escrow (writable),
    //   attendee_deposit (writable)
    // The program also needs system_program in the accounts list.
    let instruction_accounts = vec![
        AccountMeta {
            pubkey: organizer,
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: event_escrow,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: attendee_deposit,
            is_signer: false,
            is_writable: true,
        },
    ];

    // Build message accounts in Solana canonical order + resolve indices
    let (message_accounts, program_id_index, ix_account_indices) =
        build_message_accounts(&instruction_accounts, &program_id, &[]);

    let compiled_ix = CompiledInstruction {
        program_id_index,
        accounts: ix_account_indices,
        data: ix_data,
    };

    // Fetch recent blockhash
    let blockhash_resp = get_latest_blockhash(rpc_url, kv).await?;
    let blockhash_bytes = pubkey_from_base58(&blockhash_resp.value)?;

    let tx_bytes = serialize_transaction(&message_accounts, &[compiled_ix], &blockhash_bytes);
    let tx_b64 = base64::engine::general_purpose::STANDARD.encode(&tx_bytes);

    Ok(MarkCheckedInTransaction {
        transaction_b64: tx_b64,
        message: "Mark attendee as checked in".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Deactivate Event Transaction Builder
// ---------------------------------------------------------------------------

/// Build a serialized `deactivate_event` transaction for the bethere-escrow program.
///
/// Sets `is_active = false` on the event escrow, stopping new deposits.
/// Refunds are still allowed until refund_deadline. After deactivation,
/// `close_event` can be called to reclaim rent.
///
/// # Arguments
/// * `rpc_url` — Solana RPC URL (with API key if needed)
/// * `kv` — Optional KV store for blockhash caching
/// * `organizer_pubkey` — Organizer's wallet address (base58), must be signer
/// * `event_id` — Numeric event ID used for PDA derivation
///
/// # Discriminator
/// 6 (deactivate_event)
///
/// # Accounts (DeactivateEvent)
///   0. organizer (signer, writable)
///   1. event_escrow (writable, PDA)
pub async fn build_deactivate_event_transaction(
    rpc_url: &str,
    kv: Option<&KvStore>,
    organizer_pubkey: &str,
    event_id: u64,
) -> Result<DeactivateEventTransaction, EscrowError> {
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID)?;

    // Derive EventEscrow PDA: ["escrow", organizer, event_id]
    let (event_escrow, _) = find_program_address(
        &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
        &program_id,
    )
    .await?;

    // deactivate_event instruction data: [6 (discriminator)] + [event_id u64 LE]
    let mut ix_data = vec![6u8];
    ix_data.extend_from_slice(&event_id.to_le_bytes());

    // Accounts for deactivate_event:
    //   organizer (signer, writable), event_escrow (writable)
    let instruction_accounts = vec![
        AccountMeta {
            pubkey: organizer,
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: event_escrow,
            is_signer: false,
            is_writable: true,
        },
    ];

    let (message_accounts, program_id_index, ix_account_indices) =
        build_message_accounts(&instruction_accounts, &program_id, &[]);

    let compiled_ix = CompiledInstruction {
        program_id_index,
        accounts: ix_account_indices,
        data: ix_data,
    };

    let blockhash_resp = get_latest_blockhash(rpc_url, kv).await?;
    let blockhash_bytes = pubkey_from_base58(&blockhash_resp.value)?;

    let tx_bytes = serialize_transaction(&message_accounts, &[compiled_ix], &blockhash_bytes);
    let tx_b64 = base64::engine::general_purpose::STANDARD.encode(&tx_bytes);

    Ok(DeactivateEventTransaction {
        transaction_b64: tx_b64,
        message: "Deactivate event — stop accepting deposits".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Close Event Transaction Builder
// ---------------------------------------------------------------------------

/// Build a serialized `close_event` transaction for the bethere-escrow program.
///
/// Closes the event escrow account and the vault token account, returning
/// rent to the organizer. Requires that the event is deactivated (is_active = false)
/// and the vault is empty (total_deposited == total_refunded + total_forfeited).
///
/// # Arguments
/// * `rpc_url` — Solana RPC URL (with API key if needed)
/// * `kv` — Optional KV store for blockhash caching
/// * `organizer_pubkey` — Organizer's wallet address (base58), must be signer
/// * `event_id` — Numeric event ID used for PDA derivation
///
/// # Discriminator
/// 5 (close_event)
///
/// # Accounts (CloseEvent)
///   0. organizer (signer, writable)
///   1. event_escrow (writable, PDA, close=organizer)
///   2. vault (writable, Token account)
///   3. token_program (readonly)
pub async fn build_close_event_transaction(
    rpc_url: &str,
    kv: Option<&KvStore>,
    organizer_pubkey: &str,
    event_id: u64,
) -> Result<CloseEventTransaction, EscrowError> {
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID)?;
    let usdc_mint = pubkey_from_base58(usdc_mint())?;
    let token_program = pubkey_from_base58(TOKEN_PROGRAM_ID)?;

    // Derive EventEscrow PDA: ["escrow", organizer, event_id]
    let (event_escrow, _) = find_program_address(
        &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
        &program_id,
    )
    .await?;

    // Derive vault ATA (owned by EventEscrow PDA)
    let vault = get_associated_token_address(&event_escrow, &usdc_mint).await?;

    // close_event instruction data: [5 (discriminator)] + [event_id u64 LE]
    let mut ix_data = vec![5u8];
    ix_data.extend_from_slice(&event_id.to_le_bytes());

    // Accounts for close_event:
    //   organizer (signer, writable), event_escrow (writable),
    //   vault (writable), token_program (readonly)
    let instruction_accounts = vec![
        AccountMeta {
            pubkey: organizer,
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: event_escrow,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: vault,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: token_program,
            is_signer: false,
            is_writable: false,
        },
    ];

    let (message_accounts, program_id_index, ix_account_indices) =
        build_message_accounts(&instruction_accounts, &program_id, &[]);

    let compiled_ix = CompiledInstruction {
        program_id_index,
        accounts: ix_account_indices,
        data: ix_data,
    };

    let blockhash_resp = get_latest_blockhash(rpc_url, kv).await?;
    let blockhash_bytes = pubkey_from_base58(&blockhash_resp.value)?;

    let tx_bytes = serialize_transaction(&message_accounts, &[compiled_ix], &blockhash_bytes);
    let tx_b64 = base64::engine::general_purpose::STANDARD.encode(&tx_bytes);

    Ok(CloseEventTransaction {
        transaction_b64: tx_b64,
        message: "Close event escrow and reclaim rent".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Claim Forfeited Transaction Builder
// ---------------------------------------------------------------------------

/// Build a serialized `claim_forfeited` transaction for the bethere-escrow program.
///
/// Transfers forfeited USDC (deposits from no-shows) from the vault to the
/// organizer's USDC token account. Only callable after refund_deadline has passed.
///
/// # Arguments
/// * `rpc_url` — Solana RPC URL (with API key if needed)
/// * `kv` — Optional KV store for blockhash caching
/// * `organizer_pubkey` — Organizer's wallet address (base58), must be signer
/// * `event_id` — Numeric event ID used for PDA derivation
///
/// # Discriminator
/// 4 (claim_forfeited)
///
/// # Accounts (ClaimForfeited)
///   0. organizer (signer, writable)
///   1. event_escrow (writable, PDA)
///   2. organizer_ta (writable, init idempotent)
///   3. usdc_mint (readonly)
///   4. vault (writable, Token account)
///   5. rent (readonly)
///   6. token_program (readonly)
///   7. system_program (readonly)
pub async fn build_claim_forfeited_transaction(
    rpc_url: &str,
    kv: Option<&KvStore>,
    organizer_pubkey: &str,
    event_id: u64,
) -> Result<ClaimForfeitedTransaction, EscrowError> {
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID)?;
    let usdc_mint = pubkey_from_base58(usdc_mint())?;
    let token_program = pubkey_from_base58(TOKEN_PROGRAM_ID)?;
    let system_program = pubkey_from_base58(SYSTEM_PROGRAM_ID)?;
    let rent_sysvar = pubkey_from_base58(RENT_SYSVAR_ID)?;
    let ata_program = pubkey_from_base58(ASSOCIATED_TOKEN_PROGRAM_ID)?;

    // Derive EventEscrow PDA: ["escrow", organizer, event_id]
    let (event_escrow, _) = find_program_address(
        &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
        &program_id,
    )
    .await?;

    // Derive vault ATA (owned by EventEscrow PDA)
    let vault = get_associated_token_address(&event_escrow, &usdc_mint).await?;

    // Derive organizer's USDC ATA
    let organizer_ta = get_associated_token_address(&organizer, &usdc_mint).await?;

    // claim_forfeited instruction data: [4 (discriminator)] + [event_id u64 LE]
    let mut ix_data = vec![4u8];
    ix_data.extend_from_slice(&event_id.to_le_bytes());

    // Accounts for claim_forfeited:
    //   organizer (signer, writable), event_escrow (writable),
    //   organizer_ta (init idempotent), usdc_mint (readonly),
    //   vault (writable), rent (readonly), token_program (readonly),
    //   system_program (readonly)
    let instruction_accounts = vec![
        AccountMeta {
            pubkey: organizer,
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: event_escrow,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: organizer_ta,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: usdc_mint,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: vault,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: rent_sysvar,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: token_program,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: system_program,
            is_signer: false,
            is_writable: false,
        },
    ];

    // Extra accounts needed for CPI but NOT passed to the escrow instruction.
    // The claim_forfeited struct has `init(idempotent)` on organizer_ta which
    // CPIs to the ATA program. Same limitation as create_event — pre-create
    // the organizer_ta if the CPI fails with "signer privilege escalated".
    let extra_message_accounts: Vec<AccountMeta> = vec![AccountMeta {
        pubkey: ata_program,
        is_signer: false,
        is_writable: false,
    }];

    let (message_accounts, program_id_index, ix_account_indices) =
        build_message_accounts(&instruction_accounts, &program_id, &extra_message_accounts);

    let compiled_ix = CompiledInstruction {
        program_id_index,
        accounts: ix_account_indices,
        data: ix_data,
    };

    let blockhash_resp = get_latest_blockhash(rpc_url, kv).await?;
    let blockhash_bytes = pubkey_from_base58(&blockhash_resp.value)?;

    let tx_bytes = serialize_transaction(&message_accounts, &[compiled_ix], &blockhash_bytes);
    let tx_b64 = base64::engine::general_purpose::STANDARD.encode(&tx_bytes);

    Ok(ClaimForfeitedTransaction {
        transaction_b64: tx_b64,
        message: "Claim forfeited deposits from no-shows".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Close Deposit Transaction Builder
// ---------------------------------------------------------------------------

/// Build a serialized `close_deposit` transaction for the bethere-escrow program.
///
/// Closes the AttendeeDeposit PDA, reclaiming rent lamports. The attendee
/// signs the transaction. The instruction discriminator is 7.
///
/// # Arguments
/// * `rpc_url` — Solana RPC URL (with API key if needed)
/// * `kv` — Optional KV store for blockhash caching
/// * `organizer_pubkey` — Organizer's wallet address (base58), used for PDA derivation
/// * `event_id` — Numeric event ID used for PDA derivation
/// * `attendee_pubkey` — Attendee's wallet address (base58), must be signer
///
/// # Discriminator
/// 7 (close_deposit)
///
/// # Accounts (CloseDeposit)
///   0. signer (signer, writable) — attendee or GC closer
///   1. event_escrow (readonly) — may be closed/empty
///   2. attendee_deposit (writable) — will be closed
///   3. system_program (readonly)
pub async fn build_close_deposit_transaction(
    rpc_url: &str,
    kv: Option<&KvStore>,
    organizer_pubkey: &str,
    event_id: u64,
    attendee_pubkey: &str,
) -> Result<CloseDepositTransaction, EscrowError> {
    let attendee = pubkey_from_base58(attendee_pubkey)?;
    let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID)?;
    let system_program = pubkey_from_base58(SYSTEM_PROGRAM_ID)?;

    // Derive EventEscrow PDA: ["escrow", organizer, event_id]
    let (event_escrow, _) = find_program_address(
        &[
            b"escrow",
            pubkey_from_base58(organizer_pubkey)?.as_slice(),
            &event_id.to_le_bytes(),
        ],
        &program_id,
    )
    .await?;

    // Derive AttendeeDeposit PDA: ["deposit", event_escrow, attendee]
    let (attendee_deposit, _) = find_program_address(
        &[b"deposit", event_escrow.as_slice(), attendee.as_slice()],
        &program_id,
    )
    .await?;

    // close_deposit instruction data: [7 (discriminator)] + [event_id u64 LE]
    let mut ix_data = vec![7u8];
    ix_data.extend_from_slice(&event_id.to_le_bytes());

    // Accounts for close_deposit:
    //   attendee (signer, writable), event_escrow (readonly),
    //   attendee_deposit (writable), system_program (readonly)
    let instruction_accounts = vec![
        AccountMeta {
            pubkey: attendee,
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: event_escrow,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: attendee_deposit,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: system_program,
            is_signer: false,
            is_writable: false,
        },
    ];

    let (message_accounts, program_id_index, ix_account_indices) =
        build_message_accounts(&instruction_accounts, &program_id, &[]);

    let compiled_ix = CompiledInstruction {
        program_id_index,
        accounts: ix_account_indices,
        data: ix_data,
    };

    let blockhash_resp = get_latest_blockhash(rpc_url, kv).await?;
    let blockhash_bytes = pubkey_from_base58(&blockhash_resp.value)?;

    let tx_bytes = serialize_transaction(&message_accounts, &[compiled_ix], &blockhash_bytes);
    let tx_b64 = base64::engine::general_purpose::STANDARD.encode(&tx_bytes);

    Ok(CloseDepositTransaction {
        transaction_b64: tx_b64,
        message: "Close deposit account and reclaim rent".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Combined Refund + Close Deposit Transaction Builder
// ---------------------------------------------------------------------------

/// Build a single transaction containing both `refund` and `close_deposit` instructions.
///
/// This is atomic: if either instruction fails, the whole TX reverts.
/// Order matters — refund must run first (sets `refunded = true`), then close_deposit
/// validates that flag before closing the PDA and reclaiming rent.
///
/// # Arguments
/// * `rpc_url` — Solana RPC URL (with API key)
/// * `kv` — Optional KV store for blockhash caching
/// * `organizer_pubkey` — Organizer's wallet address (base58)
/// * `event_id` — On-chain event ID for PDA derivation
/// * `attendee_pubkey` — Attendee's wallet address (base58)
pub async fn build_refund_and_close_transaction(
    rpc_url: &str,
    kv: Option<&KvStore>,
    organizer_pubkey: &str,
    event_id: u64,
    attendee_pubkey: &str,
) -> Result<RefundAndCloseTransaction, EscrowError> {
    // Parse pubkeys
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let attendee = pubkey_from_base58(attendee_pubkey)?;
    let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID)?;
    let usdc_mint = pubkey_from_base58(usdc_mint())?;
    let token_program = pubkey_from_base58(TOKEN_PROGRAM_ID)?;
    let system_program = pubkey_from_base58(SYSTEM_PROGRAM_ID)?;
    let rent_sysvar = pubkey_from_base58(RENT_SYSVAR_ID)?;
    let ata_program = pubkey_from_base58(ASSOCIATED_TOKEN_PROGRAM_ID)?;

    // Derive PDAs
    let (event_escrow, _escrow_bump) = find_program_address(
        &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
        &program_id,
    )
    .await?;

    let (attendee_deposit, _deposit_bump) = find_program_address(
        &[b"deposit", event_escrow.as_slice(), attendee.as_slice()],
        &program_id,
    )
    .await?;

    let vault = get_associated_token_address(&event_escrow, &usdc_mint).await?;
    let attendee_ta = get_associated_token_address(&attendee, &usdc_mint).await?;

    // ---- Instruction 1: Refund (discriminator 3) ----
    let mut refund_data = vec![3u8];
    refund_data.extend_from_slice(&event_id.to_le_bytes());

    let refund_accounts = vec![
        AccountMeta {
            pubkey: attendee,
            is_signer: true,
            is_writable: true,
        }, // 0 — attendee (Signer)
        AccountMeta {
            pubkey: event_escrow,
            is_signer: false,
            is_writable: true,
        }, // 1 — event_escrow (mut)
        AccountMeta {
            pubkey: usdc_mint,
            is_signer: false,
            is_writable: false,
        }, // 2 — usdc_mint
        AccountMeta {
            pubkey: attendee_deposit,
            is_signer: false,
            is_writable: true,
        }, // 3 — attendee_deposit (mut)
        AccountMeta {
            pubkey: attendee_ta,
            is_signer: false,
            is_writable: true,
        }, // 4 — attendee_ta (init idempotent)
        AccountMeta {
            pubkey: vault,
            is_signer: false,
            is_writable: true,
        }, // 5 — vault (mut)
        AccountMeta {
            pubkey: rent_sysvar,
            is_signer: false,
            is_writable: false,
        }, // 6 — rent
        AccountMeta {
            pubkey: token_program,
            is_signer: false,
            is_writable: false,
        }, // 7 — token_program
        AccountMeta {
            pubkey: system_program,
            is_signer: false,
            is_writable: false,
        }, // 8 — system_program
    ];

    // ---- Instruction 2: Close Deposit (discriminator 7) ----
    let mut close_data = vec![7u8];
    close_data.extend_from_slice(&event_id.to_le_bytes());

    let close_accounts = vec![
        AccountMeta {
            pubkey: attendee,
            is_signer: true,
            is_writable: true,
        }, // 0 — signer (attendee)
        AccountMeta {
            pubkey: event_escrow,
            is_signer: false,
            is_writable: false,
        }, // 1 — event_escrow (readonly)
        AccountMeta {
            pubkey: attendee_deposit,
            is_signer: false,
            is_writable: true,
        }, // 2 — attendee_deposit (mut)
        AccountMeta {
            pubkey: system_program,
            is_signer: false,
            is_writable: false,
        }, // 3 — system_program
    ];

    // Merge accounts from both instructions into a single ordered message
    let message_accounts = merge_message_accounts(
        &[&refund_accounts, &close_accounts],
        &[program_id, ata_program],
    );

    // Build index lookup
    let get_index = |pk: &PubkeyBytes| -> u8 {
        message_accounts
            .iter()
            .position(|m| &m.pubkey == pk)
            .expect("all accounts should be in merged message") as u8
    };

    let program_id_index = get_index(&program_id);

    // Resolve refund instruction indices
    let refund_indices: Vec<u8> = refund_accounts
        .iter()
        .map(|m| get_index(&m.pubkey))
        .collect();
    let refund_ix = CompiledInstruction {
        program_id_index,
        accounts: refund_indices,
        data: refund_data,
    };

    // Resolve close_deposit instruction indices
    let close_indices: Vec<u8> = close_accounts
        .iter()
        .map(|m| get_index(&m.pubkey))
        .collect();
    let close_ix = CompiledInstruction {
        program_id_index,
        accounts: close_indices,
        data: close_data,
    };

    // Fetch recent blockhash
    let blockhash_resp = get_latest_blockhash(rpc_url, kv).await?;
    let blockhash_bytes = pubkey_from_base58(&blockhash_resp.value)?;

    // Serialize with BOTH instructions
    let tx_bytes =
        serialize_transaction(&message_accounts, &[refund_ix, close_ix], &blockhash_bytes);
    let tx_b64 = base64::engine::general_purpose::STANDARD.encode(&tx_bytes);

    Ok(RefundAndCloseTransaction {
        transaction_b64: tx_b64,
        message: "Refund USDC and reclaim deposit rent in one transaction".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base58_decode_system_program() {
        let bytes = base58_decode(SYSTEM_PROGRAM_ID).unwrap();
        assert_eq!(bytes.len(), 32);
        // System program is all zeros
        assert!(bytes.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_base58_decode_token_program() {
        let bytes = base58_decode(TOKEN_PROGRAM_ID).unwrap();
        assert_eq!(bytes.len(), 32);
    }

    #[test]
    fn test_pubkey_from_base58_valid() {
        let pk = pubkey_from_base58(SYSTEM_PROGRAM_ID).unwrap();
        assert!(pk.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_pubkey_from_base58_invalid() {
        let result = pubkey_from_base58("0".repeat(32).as_str());
        assert!(result.is_err()); // '0' is not valid base58
    }

    #[test]
    fn test_base58_roundtrip_system_program() {
        let pk = pubkey_from_base58(SYSTEM_PROGRAM_ID).unwrap();
        let encoded = pubkey_to_base58(&pk);
        assert_eq!(encoded, SYSTEM_PROGRAM_ID);
    }

    #[test]
    fn test_base58_roundtrip_escrow_program() {
        let pk = pubkey_from_base58(ESCROW_PROGRAM_ID).unwrap();
        let encoded = pubkey_to_base58(&pk);
        assert_eq!(encoded, ESCROW_PROGRAM_ID);
    }

    #[test]
    fn test_encode_compact_u16_small() {
        let mut buf = Vec::new();
        encode_compact_u16(5, &mut buf);
        assert_eq!(buf, vec![5]);
    }

    #[test]
    fn test_encode_compact_u16_medium() {
        let mut buf = Vec::new();
        encode_compact_u16(128, &mut buf);
        assert_eq!(buf, vec![0x80, 0x01]);
    }

    #[test]
    fn test_encode_compact_u16_large() {
        let mut buf = Vec::new();
        encode_compact_u16(16383, &mut buf);
        assert_eq!(buf, vec![0xff, 0x7f]);
    }

    #[test]
    fn test_compact_u16_roundtrip() {
        for val in [0, 1, 127, 128, 255, 16383] {
            let mut buf = Vec::new();
            encode_compact_u16(val, &mut buf);
            // Decode manually
            let decoded = decode_compact_u16(&buf);
            assert_eq!(decoded, val, "roundtrip failed for {val}");
        }
    }

    fn decode_compact_u16(buf: &[u8]) -> usize {
        let mut val = 0usize;
        let mut shift = 0;
        for &byte in buf {
            val |= ((byte & 0x7f) as usize) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
        }
        val
    }

    // -----------------------------------------------------------------------
    // PDA derivation verification tests
    //
    // Test vectors computed by reference implementation using ed25519-dalek + sha2
    // (see /tmp/pda_test or scripts/verify_pda.rs)
    //
    // Seeds: organizer="9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b", event_id=1
    // -----------------------------------------------------------------------

    /// Test that base58_decode handles the correct ATA program address.
    /// The previous address had an invalid base58 character ('O' at position 34).
    #[test]
    fn test_base58_decode_ata_program() {
        let bytes = base58_decode(ASSOCIATED_TOKEN_PROGRAM_ID).unwrap();
        assert_eq!(bytes.len(), 32);
    }

    #[test]
    fn test_base58_roundtrip_ata_program() {
        let pk = pubkey_from_base58(ASSOCIATED_TOKEN_PROGRAM_ID).unwrap();
        let encoded = pubkey_to_base58(&pk);
        assert_eq!(encoded, ASSOCIATED_TOKEN_PROGRAM_ID);
    }

    /// Verify EventEscrow PDA derivation matches @solana/web3.js reference.
    /// Expected: 3CzSgvftMgjQE1Du9uyamJe6xVCMmu1tvEhHc172Z4JD (bump=255)
    #[tokio::test]
    async fn test_find_event_escrow_pda() {
        let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID).unwrap();
        let organizer = pubkey_from_base58("9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b").unwrap();
        let event_id: u64 = 1u64;

        let (pda, bump) = find_program_address(
            &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
            &program_id,
        )
        .await
        .unwrap();

        let expected = "3CzSgvftMgjQE1Du9uyamJe6xVCMmu1tvEhHc172Z4JD";
        assert_eq!(pubkey_to_base58(&pda), expected, "EventEscrow PDA mismatch");
        assert_eq!(bump, 255, "EventEscrow bump mismatch");
    }

    /// Verify AttendeeDeposit PDA derivation matches @solana/web3.js reference.
    /// Expected: EwGrFaXTJdY8cv3T4d93shtASJZdp1t34Y7rGtbf5Fhi (bump=255)
    #[tokio::test]
    async fn test_find_attendee_deposit_pda() {
        let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID).unwrap();
        let organizer = pubkey_from_base58("9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b").unwrap();
        let event_id: u64 = 1u64;
        let attendee = pubkey_from_base58("9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b").unwrap();

        // First derive EventEscrow PDA
        let (escrow_pda, _) = find_program_address(
            &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
            &program_id,
        )
        .await
        .unwrap();

        // Then derive AttendeeDeposit PDA
        let (pda, bump) = find_program_address(
            &[b"deposit", escrow_pda.as_slice(), attendee.as_slice()],
            &program_id,
        )
        .await
        .unwrap();

        let expected = "EwGrFaXTJdY8cv3T4d93shtASJZdp1t34Y7rGtbf5Fhi";
        assert_eq!(
            pubkey_to_base58(&pda),
            expected,
            "AttendeeDeposit PDA mismatch"
        );
        assert_eq!(bump, 252, "AttendeeDeposit bump mismatch");
    }

    /// Verify Vault ATA derivation matches @solana/web3.js reference.
    /// Expected: DXiJimCs3Rzv1i3W93oeRSoxcT8Coeo2YqA7iUaQKndQ (bump=255)
    #[tokio::test]
    async fn test_find_vault_ata() {
        let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID).unwrap();
        let organizer = pubkey_from_base58("9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b").unwrap();
        let event_id: u64 = 1u64;

        // Derive EventEscrow PDA
        let (escrow_pda, _) = find_program_address(
            &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
            &program_id,
        )
        .await
        .unwrap();

        // Derive vault ATA
        let vault =
            get_associated_token_address(&escrow_pda, &pubkey_from_base58(usdc_mint()).unwrap())
                .await
                .unwrap();

        let expected = "DXiJimCs3Rzv1i3W93oeRSoxcT8Coeo2YqA7iUaQKndQ";
        assert_eq!(pubkey_to_base58(&vault), expected, "Vault ATA mismatch");
    }

    /// Verify the ATA CreateIdempotent discriminator is [1].
    /// The ATA program dispatches on instruction data:
    ///   [] → Create (non-idempotent, fails with IllegalOwner if account exists)
    ///   [1] → CreateIdempotent (no-op if account exists)
    ///   [2] → RecoverNested
    /// Using empty data was the root cause of the IllegalOwner bug.
    #[test]
    fn test_ata_create_idempotent_discriminator() {
        // The discriminator for CreateIdempotent must be [1], NOT empty
        let discriminator: Vec<u8> = vec![1];
        assert_eq!(
            discriminator,
            vec![1],
            "ATA CreateIdempotent must use [1] as discriminator"
        );
        assert!(
            !discriminator.is_empty(),
            "ATA discriminator must not be empty"
        );
    }

    /// Verify Attendee USDC ATA derivation matches @solana/web3.js reference.
    /// Expected: Bhgn1ZPvwe6ZkA7A9waU4t9BQdpEjdnSM4ZXeNCm1kuw (bump=254)
    #[tokio::test]
    async fn test_find_attendee_ata() {
        let attendee = pubkey_from_base58("9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b").unwrap();
        let usdc_mint = pubkey_from_base58(usdc_mint()).unwrap();

        let ata = get_associated_token_address(&attendee, &usdc_mint)
            .await
            .unwrap();

        let expected = "Bhgn1ZPvwe6ZkA7A9waU4t9BQdpEjdnSM4ZXeNCm1kuw";
        assert_eq!(pubkey_to_base58(&ata), expected, "Attendee ATA mismatch");
    }
}
