//! Cryptographic primitives: SHA-256, base58, PDA derivation, ATA derivation.

#[cfg(not(test))]
use js_sys::{Object, Reflect, Uint8Array};
#[cfg(not(test))]
use wasm_bindgen::prelude::*;
#[cfg(not(test))]
use wasm_bindgen_futures::JsFuture;

use super::{ASSOCIATED_TOKEN_PROGRAM_ID, EscrowError, PubkeyBytes, TOKEN_PROGRAM_ID};

// ---------------------------------------------------------------------------
// SHA-256
// ---------------------------------------------------------------------------

/// Compute SHA-256 hash.
/// In WASM (worker runtime): uses Web Crypto SubtleCrypto.
/// In native (tests): uses pure Rust implementation.
pub(crate) async fn sha256(data: &[u8]) -> Result<[u8; 32], EscrowError> {
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
pub(crate) async fn find_program_address(
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
pub async fn get_associated_token_address(
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
