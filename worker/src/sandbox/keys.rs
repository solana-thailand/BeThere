//! A devnet keypair the Worker signs with, and the one place it signs.
//!
//! The secret is the keypair file `solana-keygen new` writes, pasted whole:
//! a JSON array of 64 bytes, the 32-byte seed then the 32-byte public key.
//! Parsing re-derives the public key from the seed and refuses a file whose
//! halves disagree, so a truncated or mixed-up paste fails at load, not as a
//! signature the cluster rejects.

use ed25519_dalek::{Signer, SigningKey};

use crate::solana_escrow::pubkey_to_base58;

/// Why a keypair or a transaction could not be used. The text never carries
/// key material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    /// The secret is not a JSON array of 64 bytes.
    NotKeypairJson,
    /// The second half is not the public key of the first.
    PubkeyMismatch,
    /// The transaction bytes do not parse as a legacy transaction.
    MalformedTransaction,
    /// This key is not one of the transaction's required signers.
    NotASigner,
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::NotKeypairJson => "keypair is not a JSON array of 64 bytes",
            Self::PubkeyMismatch => "keypair public half does not match its secret half",
            Self::MalformedTransaction => "transaction bytes are malformed",
            Self::NotASigner => "key is not a required signer of the transaction",
        };
        f.write_str(text)
    }
}

/// A Worker-held signing key. `Debug` shows the address only.
pub struct SandboxSigner {
    key: SigningKey,
    pubkey: [u8; 32],
    address: String,
}

impl std::fmt::Debug for SandboxSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SandboxSigner")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

impl SandboxSigner {
    /// Load the `solana-keygen` keypair file contents.
    pub fn from_keypair_json(raw: &str) -> Result<Self, KeyError> {
        let bytes: Vec<u8> =
            serde_json::from_str(raw.trim()).map_err(|_| KeyError::NotKeypairJson)?;
        let (seed, public) = match <[u8; 64]>::try_from(bytes.as_slice()) {
            Ok(all) => {
                let mut seed = [0u8; 32];
                let mut public = [0u8; 32];
                seed.copy_from_slice(&all[..32]);
                public.copy_from_slice(&all[32..]);
                (seed, public)
            }
            Err(_) => return Err(KeyError::NotKeypairJson),
        };
        let key = SigningKey::from_bytes(&seed);
        let pubkey = key.verifying_key().to_bytes();
        if pubkey != public {
            return Err(KeyError::PubkeyMismatch);
        }
        Ok(Self {
            key,
            pubkey,
            address: pubkey_to_base58(&pubkey),
        })
    }

    pub fn pubkey(&self) -> &[u8; 32] {
        &self.pubkey
    }

    /// The base58 address.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Sign a serialized legacy transaction in place: find this key among the
    /// required signers and write its signature into that slot.
    pub fn sign_transaction(&self, tx: &mut [u8]) -> Result<(), KeyError> {
        let slot = signer_slot(tx, &self.pubkey)?;
        let message_start = 1 + 64 * usize::from(tx[0]);
        let signature = self.key.sign(&tx[message_start..]).to_bytes();
        let at = 1 + 64 * slot;
        tx[at..at + 64].copy_from_slice(&signature);
        Ok(())
    }
}

/// The signature slot `pubkey` owns in `tx`. Our builders emit fewer than 128
/// signers, so the signature count is a one-byte compact-u16.
pub fn signer_slot(tx: &[u8], pubkey: &[u8; 32]) -> Result<usize, KeyError> {
    let signatures = usize::from(*tx.first().ok_or(KeyError::MalformedTransaction)?);
    if signatures == 0 || signatures >= 0x80 {
        return Err(KeyError::MalformedTransaction);
    }
    let message = tx
        .get(1 + 64 * signatures..)
        .ok_or(KeyError::MalformedTransaction)?;
    // Header: required signers, readonly signed, readonly unsigned; then the
    // one-byte key count (also < 128 for our builders) and the keys.
    let required = usize::from(*message.first().ok_or(KeyError::MalformedTransaction)?);
    let key_count = usize::from(*message.get(3).ok_or(KeyError::MalformedTransaction)?);
    if required != signatures || key_count >= 0x80 || key_count < required {
        return Err(KeyError::MalformedTransaction);
    }
    let keys = message
        .get(4..4 + 32 * key_count)
        .ok_or(KeyError::MalformedTransaction)?;
    keys.as_chunks::<32>()
        .0
        .iter()
        .take(required)
        .position(|key| key == pubkey)
        .ok_or(KeyError::NotASigner)
}
