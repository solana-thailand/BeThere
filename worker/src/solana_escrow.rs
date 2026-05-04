//! Solana escrow transaction building for USDC deposit.
//!
//! Builds a serialized Solana transaction containing the `deposit` instruction
//! for the bethere-escrow program. The transaction is returned as base64
//! for Solana Pay Transaction Request flow.
//!
//! PDA derivation uses SHA-256 via Web Crypto (SubtleCrypto).
//! Transaction serialization follows the Solana wire format (bincode-like).

use base64::Engine;

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
const ESCROW_PROGRAM_ID: &str = "2TGfNNXNez2NgopffDnYYhLNYmndUBBwg5SvpD5XQeLo";

/// Devnet USDC mint.
/// Mainnet: EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1m
const USDC_MINT_DEVNET: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

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
pub struct RefundTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for Solana Pay.
    pub message: String,
}

/// Result of building a create_event transaction.
pub struct CreateEventTransaction {
    /// Base64-encoded serialized transaction (unsigned).
    pub transaction_b64: String,
    /// Human-readable message for Solana Pay.
    pub message: String,
    /// Derived EventEscrow PDA address (base58).
    pub escrow_address: String,
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
}

impl std::fmt::Display for EscrowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPubkey(s) => write!(f, "invalid pubkey: {s}"),
            Self::PdaDerivationFailed => write!(f, "PDA derivation failed — no valid bump found"),
            Self::HashFailed(s) => write!(f, "SHA-256 hash failed: {s}"),
            Self::RpcFailed(s) => write!(f, "RPC call failed: {s}"),
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
fn base58_decode(input: &str) -> Result<Vec<u8>, EscrowError> {
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
fn pubkey_from_base58(s: &str) -> Result<PubkeyBytes, EscrowError> {
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
fn base58_encode(data: &[u8]) -> String {
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
fn pubkey_to_base58(pk: &PubkeyBytes) -> String {
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

/// Fetch the latest blockhash from the Solana JSON-RPC endpoint.
async fn get_latest_blockhash(rpc_url: &str) -> Result<RecentBlockhash, EscrowError> {
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

// ---------------------------------------------------------------------------
// Transaction serialization
// ---------------------------------------------------------------------------

/// Account metadata for transaction building.
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
    organizer_pubkey: &str,
    event_id: u64,
    attendee_pubkey: &str,
    deposit_amount: u64,
) -> Result<DepositTransaction, EscrowError> {
    // Parse pubkeys
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let attendee = pubkey_from_base58(attendee_pubkey)?;
    let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID)?;
    let usdc_mint = pubkey_from_base58(USDC_MINT_DEVNET)?;
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

    // Build the message account keys in Solana's canonical order:
    // signers(writable) → signers(readonly) → non-signers(writable) → non-signers(readonly)
    // The program_id is included as a non-signer readonly.
    let mut message_accounts: Vec<AccountMeta> = Vec::new();

    // 1. Signer + writable
    for m in &instruction_accounts {
        if m.is_signer && m.is_writable {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: true,
                is_writable: true,
            });
        }
    }
    // 2. Signer + readonly (none in our case)
    for m in &instruction_accounts {
        if m.is_signer && !m.is_writable {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: true,
                is_writable: false,
            });
        }
    }
    // 3. Non-signer + writable
    for m in &instruction_accounts {
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
    for m in &instruction_accounts {
        if !m.is_signer && !m.is_writable {
            if m.pubkey == program_id {
                program_id_in_message = true;
            }
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: false,
                is_writable: false,
            });
        }
    }
    // Add program_id if not already in the list
    if !program_id_in_message {
        message_accounts.push(AccountMeta {
            pubkey: program_id,
            is_signer: false,
            is_writable: false,
        });
    }

    // Build a lookup: pubkey → index in message_accounts
    let get_index = |pk: &PubkeyBytes| -> u8 {
        message_accounts
            .iter()
            .position(|m| &m.pubkey == pk)
            .expect("all accounts should be in message") as u8
    };

    // The program_id_index
    let program_id_index = get_index(&program_id);

    // Build instruction account indices in the order the program expects
    let ix_account_indices: Vec<u8> = instruction_accounts
        .iter()
        .map(|m| get_index(&m.pubkey))
        .collect();

    let compiled_ix = CompiledInstruction {
        program_id_index,
        accounts: ix_account_indices,
        data: ix_data,
    };

    // Fetch recent blockhash
    let blockhash_resp = get_latest_blockhash(rpc_url).await?;
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
pub async fn build_refund_transaction(
    rpc_url: &str,
    organizer_pubkey: &str,
    event_id: u64,
    attendee_pubkey: &str,
) -> Result<RefundTransaction, EscrowError> {
    // Parse pubkeys
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let attendee = pubkey_from_base58(attendee_pubkey)?;
    let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID)?;
    let usdc_mint = pubkey_from_base58(USDC_MINT_DEVNET)?;
    let token_program = pubkey_from_base58(TOKEN_PROGRAM_ID)?;
    let system_program = pubkey_from_base58(SYSTEM_PROGRAM_ID)?;

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
    let vault_ta = get_associated_token_address(&event_escrow, &usdc_mint).await?;

    // Derive attendee's USDC ATA
    let attendee_ta = get_associated_token_address(&attendee, &usdc_mint).await?;

    // Refund instruction data: just the discriminator [3]
    let ix_data = vec![3u8];

    // The refund instruction expects accounts in this order:
    //   event_escrow (writable), attendee_deposit (writable), attendee (signer),
    //   vault_ta (writable), attendee_ta (writable), organizer (readonly),
    //   token_program (readonly), system_program (readonly)
    let instruction_accounts = vec![
        AccountMeta {
            pubkey: event_escrow,
            is_signer: false,
            is_writable: true,
        }, // 0
        AccountMeta {
            pubkey: attendee_deposit,
            is_signer: false,
            is_writable: true,
        }, // 1
        AccountMeta {
            pubkey: attendee,
            is_signer: true,
            is_writable: true,
        }, // 2
        AccountMeta {
            pubkey: vault_ta,
            is_signer: false,
            is_writable: true,
        }, // 3
        AccountMeta {
            pubkey: attendee_ta,
            is_signer: false,
            is_writable: true,
        }, // 4
        AccountMeta {
            pubkey: organizer,
            is_signer: false,
            is_writable: false,
        }, // 5
        AccountMeta {
            pubkey: token_program,
            is_signer: false,
            is_writable: false,
        }, // 6
        AccountMeta {
            pubkey: system_program,
            is_signer: false,
            is_writable: false,
        }, // 7
    ];

    // Build the message account keys in Solana's canonical order:
    // signers(writable) → signers(readonly) → non-signers(writable) → non-signers(readonly)
    // The program_id is included as a non-signer readonly.
    let mut message_accounts: Vec<AccountMeta> = Vec::new();

    // 1. Signer + writable
    for m in &instruction_accounts {
        if m.is_signer && m.is_writable {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: true,
                is_writable: true,
            });
        }
    }
    // 2. Signer + readonly (none in our case)
    for m in &instruction_accounts {
        if m.is_signer && !m.is_writable {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: true,
                is_writable: false,
            });
        }
    }
    // 3. Non-signer + writable
    for m in &instruction_accounts {
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
    for m in &instruction_accounts {
        if !m.is_signer && !m.is_writable {
            if m.pubkey == program_id {
                program_id_in_message = true;
            }
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: false,
                is_writable: false,
            });
        }
    }
    // Add program_id if not already in the list
    if !program_id_in_message {
        message_accounts.push(AccountMeta {
            pubkey: program_id,
            is_signer: false,
            is_writable: false,
        });
    }

    // Build a lookup: pubkey → index in message_accounts
    let get_index = |pk: &PubkeyBytes| -> u8 {
        message_accounts
            .iter()
            .position(|m| &m.pubkey == pk)
            .expect("all accounts should be in message") as u8
    };

    // The program_id_index
    let program_id_index = get_index(&program_id);

    // Build instruction account indices in the order the program expects
    let ix_account_indices: Vec<u8> = instruction_accounts
        .iter()
        .map(|m| get_index(&m.pubkey))
        .collect();

    let compiled_ix = CompiledInstruction {
        program_id_index,
        accounts: ix_account_indices,
        data: ix_data,
    };

    // Fetch recent blockhash
    let blockhash_resp = get_latest_blockhash(rpc_url).await?;
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
// Create Event Transaction Builder
// ---------------------------------------------------------------------------

/// Build a serialized `create_event` transaction for the bethere-escrow program.
///
/// The `create_event` instruction initializes the EventEscrow PDA and the
/// vault ATA on-chain. This must be called by the organizer before any
/// attendee can deposit.
///
/// # Arguments
/// * `rpc_url` — Solana RPC URL (with API key if needed)
/// * `organizer_pubkey` — Organizer's wallet address (base58), also the signer
/// * `event_id` — Numeric event ID used for PDA derivation
/// * `deposit_amount` — Amount in USDC smallest unit (6 decimals)
/// * `event_end` — Unix timestamp (seconds) when the event ends
/// * `refund_deadline` — Unix timestamp (seconds) for the refund deadline
///
/// # Returns
/// A `CreateEventTransaction` with the base64-encoded transaction, a message,
/// and the derived EventEscrow PDA address for storage.
pub async fn build_create_event_transaction(
    rpc_url: &str,
    organizer_pubkey: &str,
    event_id: u64,
    deposit_amount: u64,
    event_end: i64,
    refund_deadline: i64,
) -> Result<CreateEventTransaction, EscrowError> {
    // Parse pubkeys
    let organizer = pubkey_from_base58(organizer_pubkey)?;
    let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID)?;
    let usdc_mint = pubkey_from_base58(USDC_MINT_DEVNET)?;
    let token_program = pubkey_from_base58(TOKEN_PROGRAM_ID)?;
    let system_program = pubkey_from_base58(SYSTEM_PROGRAM_ID)?;
    let rent_sysvar = pubkey_from_base58(RENT_SYSVAR_ID)?;

    // Derive EventEscrow PDA: ["escrow", organizer, event_id]
    let (event_escrow, _escrow_bump) = find_program_address(
        &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
        &program_id,
    )
    .await?;

    // Derive vault ATA (owned by EventEscrow PDA)
    let vault = get_associated_token_address(&event_escrow, &usdc_mint).await?;

    // Build the create_event instruction data:
    // [0 (discriminator)] + [event_id u64 LE] + [deposit_amount u64 LE] + [event_end i64 LE] + [refund_deadline i64 LE]
    let mut ix_data = vec![0u8]; // create_event discriminator
    ix_data.extend_from_slice(&event_id.to_le_bytes());
    ix_data.extend_from_slice(&deposit_amount.to_le_bytes());
    ix_data.extend_from_slice(&event_end.to_le_bytes());
    ix_data.extend_from_slice(&refund_deadline.to_le_bytes());

    // IMPORTANT: The vault ATA must be pre-created before this transaction is submitted.
    // The quasar-lang `init(idempotent)` constraint validates the account exists but
    // does NOT create it via CPI. The handler should create the vault ATA via a
    // separate RPC call before returning this transaction to the client.
    //
    // All accounts needed for the create_event instruction
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
            pubkey: usdc_mint,
            is_signer: false,
            is_writable: false,
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

    // Build the message account keys in Solana's canonical order:
    // signers(writable) → signers(readonly) → non-signers(writable) → non-signers(readonly)
    let mut message_accounts: Vec<AccountMeta> = Vec::new();

    // 1. Signer + writable
    for m in &instruction_accounts {
        if m.is_signer && m.is_writable {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: true,
                is_writable: true,
            });
        }
    }
    // 2. Signer + readonly (none)
    for m in &instruction_accounts {
        if m.is_signer && !m.is_writable {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: true,
                is_writable: false,
            });
        }
    }
    // 3. Non-signer + writable
    for m in &instruction_accounts {
        if !m.is_signer && m.is_writable {
            message_accounts.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: false,
                is_writable: true,
            });
        }
    }
    // 4. Non-signer + readonly
    let mut program_id_in_message = false;
    for m in &instruction_accounts {
        if !m.is_signer && !m.is_writable {
            if m.pubkey == program_id {
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
            pubkey: program_id,
            is_signer: false,
            is_writable: false,
        });
    }

    // Build lookups
    let get_index = |pk: &PubkeyBytes| -> u8 {
        message_accounts
            .iter()
            .position(|m| &m.pubkey == pk)
            .expect("all accounts should be in message") as u8
    };

    let program_id_index = get_index(&program_id);

    let ix_account_indices: Vec<u8> = instruction_accounts
        .iter()
        .map(|m| get_index(&m.pubkey))
        .collect();

    let compiled_ix = CompiledInstruction {
        program_id_index,
        accounts: ix_account_indices,
        data: ix_data,
    };

    // Fetch recent blockhash
    let blockhash_resp = get_latest_blockhash(rpc_url).await?;
    let blockhash_bytes = pubkey_from_base58(&blockhash_resp.value)?;

    // Serialize the transaction
    let tx_bytes = serialize_transaction(&message_accounts, &[compiled_ix], &blockhash_bytes);

    let tx_b64 = base64::engine::general_purpose::STANDARD.encode(&tx_bytes);
    let escrow_address = pubkey_to_base58(&event_escrow);

    let amount_display = deposit_amount as f64 / 1_000_000.0;

    Ok(CreateEventTransaction {
        transaction_b64: tx_b64,
        message: format!("Create event escrow ({amount_display:.2} USDC deposit)"),
        escrow_address,
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
    /// Expected: PawcSqdjb66SKp1utWraYZJDcMQfYBwwpK9QSb3EY5a (bump=255)
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

        let expected = "PawcSqdjb66SKp1utWraYZJDcMQfYBwwpK9QSb3EY5a";
        assert_eq!(pubkey_to_base58(&pda), expected, "EventEscrow PDA mismatch");
        assert_eq!(bump, 255, "EventEscrow bump mismatch");
    }

    /// Verify AttendeeDeposit PDA derivation matches @solana/web3.js reference.
    /// Expected: Cm8bAdgASHKBYehSBxC8YeVmUw2oT7sB2zVu8VQmfqcn (bump=255)
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

        let expected = "Cm8bAdgASHKBYehSBxC8YeVmUw2oT7sB2zVu8VQmfqcn";
        assert_eq!(
            pubkey_to_base58(&pda),
            expected,
            "AttendeeDeposit PDA mismatch"
        );
        assert_eq!(bump, 255, "AttendeeDeposit bump mismatch");
    }

    /// Verify Vault ATA derivation matches @solana/web3.js reference.
    /// Expected: 5exYHTcLvUbKPd3V8jxpkXn4RJL337URHM38kM2K6zbS (bump=255)
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
        let vault = get_associated_token_address(
            &escrow_pda,
            &pubkey_from_base58(USDC_MINT_DEVNET).unwrap(),
        )
        .await
        .unwrap();

        let expected = "5exYHTcLvUbKPd3V8jxpkXn4RJL337URHM38kM2K6zbS";
        assert_eq!(pubkey_to_base58(&vault), expected, "Vault ATA mismatch");
    }

    /// Verify Attendee USDC ATA derivation matches @solana/web3.js reference.
    /// Expected: Bhgn1ZPvwe6ZkA7A9waU4t9BQdpEjdnSM4ZXeNCm1kuw (bump=254)
    #[tokio::test]
    async fn test_find_attendee_ata() {
        let attendee = pubkey_from_base58("9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b").unwrap();
        let usdc_mint = pubkey_from_base58(USDC_MINT_DEVNET).unwrap();

        let ata = get_associated_token_address(&attendee, &usdc_mint)
            .await
            .unwrap();

        let expected = "Bhgn1ZPvwe6ZkA7A9waU4t9BQdpEjdnSM4ZXeNCm1kuw";
        assert_eq!(pubkey_to_base58(&ata), expected, "Attendee ATA mismatch");
    }
}
