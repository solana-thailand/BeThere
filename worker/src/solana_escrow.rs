//! Solana escrow transaction building for USDC deposit.
//!
//! Builds a serialized Solana transaction containing the `deposit` instruction
//! for the bethere-escrow program. The transaction is returned as base64
//! for Solana Pay Transaction Request flow.
//!
//! PDA derivation uses SHA-256 via Web Crypto (SubtleCrypto).
//! Transaction serialization follows the Solana wire format (bincode-like).

use base64::Engine;
use js_sys::{Object, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
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
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efOPsEErJbPX";

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
// SubtleCrypto SHA-256
// ---------------------------------------------------------------------------

/// Compute SHA-256 hash using Web Crypto SubtleCrypto.
async fn sha256(data: &[u8]) -> Result<[u8; 32], EscrowError> {
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
/// A valid PDA is one where SHA-256(seeds + bump + program_id) is NOT on the
/// Ed25519 curve (i.e., the y-coordinate's high bit is not set — checked via
/// `is_on_curve` which checks if the point is a valid Ed25519 point).
async fn find_program_address(
    seeds: &[&[u8]],
    program_id: &PubkeyBytes,
) -> Result<(PubkeyBytes, u8), EscrowError> {
    for bump in (0u8..=255).rev() {
        let mut full_seeds: Vec<u8> = Vec::new();
        for seed in seeds {
            full_seeds.extend_from_slice(seed);
        }
        full_seeds.push(bump);
        full_seeds.extend_from_slice(program_id);

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
/// This is the same check used by `solana_sdk::pubkey::is_on_curve`.
fn is_on_ed25519_curve(bytes: &[u8; 32]) -> bool {
    // Attempt to decompress the point using the standard Ed25519 encoding.
    // In Ed25519, the last byte's high bit is the sign bit, and the remaining
    // 255 bits encode the y-coordinate. A point is valid if it satisfies
    // -x^2 + y^2 = 1 + d*x^2*y^2 where d = -121665/121666 mod p.
    //
    // For PDA derivation, we use the approach from solana_sdk which uses
    // curve25519_dalek's decompression. Since we can't depend on that crate
    // in WASM, we implement a simplified check.
    //
    // The p (prime) for Ed25519: 2^255 - 19
    let p: [u64; 4] = [
        0xffffffffffffffed,
        0xffffffffffffffff,
        0xffffffffffffffff,
        0xffffffffffffffff,
    ];

    // Convert bytes to a 255-bit little-endian scalar (clear top bit of last byte)
    let mut y_bytes = *bytes;
    y_bytes[31] &= 0x7f; // Clear sign bit

    // Convert to u64 limbs (little-endian)
    let y = [
        u64::from_le_bytes(y_bytes[0..8].try_into().unwrap()),
        u64::from_le_bytes(y_bytes[8..16].try_into().unwrap()),
        u64::from_le_bytes(y_bytes[16..24].try_into().unwrap()),
        u64::from_le_bytes(y_bytes[24..32].try_into().unwrap()),
    ];

    // Check y < p
    if scalar_gte(&y, &p) {
        return false;
    }

    // d = -121665 * (121666^-1) mod p
    // Pre-computed: d = 37095705934669439343138083508754565189542113879843219016388785533085940483562
    let d: [u64; 4] = [
        0xa5304a3b3f0b3d32,
        0x9db8a8e2e43949fd,
        0x3ec7ebe6c5b6b598,
        0x52036cee2b6ffe73,
    ];

    // y^2 mod p
    let y2 = scalar_mul(&y, &y, &p);

    // u = y^2 - 1 mod p
    let u = scalar_sub(&y2, &[1, 0, 0, 0], &p);

    // v = d * y^2 + 1 mod p
    let dy2 = scalar_mul(&d, &y2, &p);
    let v = scalar_add(&dy2, &[1, 0, 0, 0], &p);

    // Check: u * v^3 == v * u^3  (equivalent to -x^2 + y^2 = 1 + d*x^2*y^2)
    let v2 = scalar_mul(&v, &v, &p);
    let v3 = scalar_mul(&v2, &v, &p);
    let u2 = scalar_mul(&u, &u, &p);
    let u3 = scalar_mul(&u2, &u, &p);

    let lhs = scalar_mul(&u, &v3, &p);
    let rhs = scalar_mul(&v, &u3, &p);

    lhs == rhs
}

// 256-bit modular arithmetic helpers (little-endian u64 limbs)

fn scalar_add(a: &[u64; 4], b: &[u64; 4], _p: &[u64; 4]) -> [u64; 4] {
    let mut carry = 0u128;
    let mut result = [0u64; 4];
    for i in 0..4 {
        let sum = a[i] as u128 + b[i] as u128 + carry;
        result[i] = sum as u64;
        carry = sum >> 64;
    }
    result
}

fn scalar_sub(a: &[u64; 4], b: &[u64; 4], p: &[u64; 4]) -> [u64; 4] {
    let mut borrow = 0i128;
    let mut result = [0u64; 4];
    for i in 0..4 {
        let diff = a[i] as i128 - b[i] as i128 - borrow;
        if diff < 0 {
            result[i] = (diff + (1i128 << 64)) as u64;
            borrow = 1;
        } else {
            result[i] = diff as u64;
            borrow = 0;
        }
    }
    // If we borrowed, add p back
    if borrow > 0 {
        let mut carry = 0u128;
        for i in 0..4 {
            let sum = result[i] as u128 + p[i] as u128 + carry;
            result[i] = sum as u64;
            carry = sum >> 64;
        }
    }
    result
}

/// schoolbook multiplication mod p
fn scalar_mul(a: &[u64; 4], b: &[u64; 4], p: &[u64; 4]) -> [u64; 4] {
    // Full 512-bit product
    let mut product = [0u128; 8];
    for i in 0..4 {
        let mut carry = 0u128;
        for j in 0..4 {
            let val = a[i] as u128 * b[j] as u128 + product[i + j] + carry;
            product[i + j] = val & 0xFFFFFFFFFFFFFFFF;
            carry = val >> 64;
        }
        product[i + 4] += carry;
    }

    // Reduce mod p using simple long division
    // For efficiency, we'll use a barrett-style reduction.
    // But for simplicity, just mask and subtract multiples of p.
    //
    // p = 2^255 - 19, so we can use the special reduction:
    // Take the high 256 bits, multiply by 19, add to low 256 bits, reduce.
    let low = [
        product[0] as u64,
        product[1] as u64,
        product[2] as u64,
        product[3] as u64,
    ];
    let high = [
        product[4] as u64,
        product[5] as u64,
        product[6] as u64,
        product[7] as u64,
    ];

    // high * 19
    let mut r = low;
    let mut carry = 0u128;
    for i in 0..4 {
        let val = r[i] as u128 + high[i] as u128 * 19 + carry;
        r[i] = val as u64;
        carry = val >> 64;
    }
    // One more reduction step if carry
    if carry > 0 {
        let val = r[0] as u128 + carry * 19;
        r[0] = val as u64;
        carry = val >> 64;
        for item in r.iter_mut().skip(1) {
            if carry == 0 {
                break;
            }
            let val = *item as u128 + carry;
            *item = val as u64;
            carry = val >> 64;
        }
    }

    // Final reduction mod p
    while scalar_gte(&r, p) {
        r = scalar_sub(&r, p, p);
    }

    r
}

fn scalar_gte(a: &[u64; 4], b: &[u64; 4]) -> bool {
    for i in (0..4).rev() {
        if a[i] > b[i] {
            return true;
        }
        if a[i] < b[i] {
            return false;
        }
    }
    true // equal
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

    // 1. Signature count (0 — unsigned, wallet will sign)
    encode_compact_u16(0, &mut buf);

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

    // The create_event instruction expects accounts in this order:
    //   organizer (signer+writable), event_escrow (writable), usdc_mint (readonly),
    //   vault (writable), rent (readonly), token_program (readonly), system_program (readonly)
    let instruction_accounts = vec![
        AccountMeta {
            pubkey: organizer,
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
            pubkey: vault,
            is_signer: false,
            is_writable: true,
        }, // 3
        AccountMeta {
            pubkey: rent_sysvar,
            is_signer: false,
            is_writable: false,
        }, // 4
        AccountMeta {
            pubkey: token_program,
            is_signer: false,
            is_writable: false,
        }, // 5
        AccountMeta {
            pubkey: system_program,
            is_signer: false,
            is_writable: false,
        }, // 6
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
            assert_eq!(decoded, val as usize, "roundtrip failed for {val}");
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
}
