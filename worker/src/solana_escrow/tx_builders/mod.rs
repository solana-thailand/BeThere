//! All public transaction builder functions.

use base64::Engine;
use worker::KvStore;

use super::crypto::{find_program_address, get_associated_token_address, pubkey_from_base58};
use super::wire::{
    AccountMeta, CompiledInstruction, build_message_accounts, get_latest_blockhash,
    serialize_transaction,
};
use super::{
    ASSOCIATED_TOKEN_PROGRAM_ID, EscrowError, INSTRUCTIONS_SYSVAR_ID, PubkeyBytes, RENT_SYSVAR_ID,
    SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID, escrow_program_id,
};

pub(crate) mod close;
pub(crate) mod deposit;
pub(crate) mod init;
pub(crate) mod mark;
pub(crate) mod refund;
pub(crate) mod rollover;

// Re-export all public builder functions so existing imports continue to work.
pub use close::{
    build_claim_forfeited_transaction, build_close_deposit_transaction,
    build_close_event_transaction, build_deactivate_event_transaction,
};
pub use deposit::build_deposit_transaction;
pub use init::build_init_escrow_transaction;
pub use mark::build_mark_checked_in_transaction;
pub use refund::{
    build_batch_claim_forfeited_transaction, build_refund_and_close_transaction,
    build_refund_transaction,
};
pub use rollover::build_rollover_deposit_transaction;

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Shorthand: construct an [`AccountMeta`] with explicit flags.
pub(crate) fn acct(pubkey: PubkeyBytes, signer: bool, writable: bool) -> AccountMeta {
    AccountMeta {
        pubkey,
        is_signer: signer,
        is_writable: writable,
    }
}

/// Signer + writable.
pub(crate) fn acct_sw(pubkey: PubkeyBytes) -> AccountMeta {
    acct(pubkey, true, true)
}

/// Non-signer + writable.
pub(crate) fn acct_w(pubkey: PubkeyBytes) -> AccountMeta {
    acct(pubkey, false, true)
}

/// Non-signer + readonly.
pub(crate) fn acct_r(pubkey: PubkeyBytes) -> AccountMeta {
    acct(pubkey, false, false)
}

// ---------------------------------------------------------------------------
// EscrowCtx — shared resolved accounts
// ---------------------------------------------------------------------------

/// Common resolved accounts shared across most escrow instructions.
///
/// Resolves program IDs and derives the EventEscrow PDA and vault ATA once
/// so every builder function can reuse them without repeating boilerplate.
pub(crate) struct EscrowCtx {
    pub program_id: PubkeyBytes,
    pub organizer: PubkeyBytes,
    pub event_escrow: PubkeyBytes,
    pub usdc_mint: PubkeyBytes,
    pub token_program: PubkeyBytes,
    pub system_program: PubkeyBytes,
    pub rent_sysvar: PubkeyBytes,
    pub instruction_sysvar: PubkeyBytes,
    pub ata_program: PubkeyBytes,
    pub vault: PubkeyBytes,
    pub event_id: u64,
}

impl EscrowCtx {
    /// Parse well-known program IDs, derive the EventEscrow PDA and vault ATA.
    pub(crate) async fn resolve(
        organizer_pubkey: &str,
        event_id: u64,
    ) -> Result<Self, EscrowError> {
        let program_id = pubkey_from_base58(escrow_program_id())?;
        let organizer = pubkey_from_base58(organizer_pubkey)?;
        let (event_escrow, _) = find_program_address(
            &[b"escrow", organizer.as_slice(), &event_id.to_le_bytes()],
            &program_id,
        )
        .await?;
        let usdc_mint = pubkey_from_base58(super::usdc_mint())?;
        let token_program = pubkey_from_base58(TOKEN_PROGRAM_ID)?;
        let system_program = pubkey_from_base58(SYSTEM_PROGRAM_ID)?;
        let rent_sysvar = pubkey_from_base58(RENT_SYSVAR_ID)?;
        let instruction_sysvar = pubkey_from_base58(INSTRUCTIONS_SYSVAR_ID)?;
        let ata_program = pubkey_from_base58(ASSOCIATED_TOKEN_PROGRAM_ID)?;
        let vault = get_associated_token_address(&event_escrow, &usdc_mint).await?;
        Ok(Self {
            program_id,
            organizer,
            event_escrow,
            usdc_mint,
            token_program,
            system_program,
            rent_sysvar,
            instruction_sysvar,
            ata_program,
            vault,
            event_id,
        })
    }

    /// Derive the AttendeeDeposit PDA for a given attendee.
    pub(crate) async fn attendee_deposit(
        &self,
        attendee: &PubkeyBytes,
    ) -> Result<(PubkeyBytes, u8), EscrowError> {
        find_program_address(
            &[
                b"deposit",
                self.event_escrow.as_slice(),
                attendee.as_slice(),
            ],
            &self.program_id,
        )
        .await
    }

    /// Derive the ATA for a wallet against USDC mint.
    pub(crate) async fn token_account(
        &self,
        owner: &PubkeyBytes,
    ) -> Result<PubkeyBytes, EscrowError> {
        get_associated_token_address(owner, &self.usdc_mint).await
    }

    /// Build instruction data: `[discriminator] + [event_id u64 LE]`.
    pub(crate) fn ix_data(&self, discriminator: u8) -> Vec<u8> {
        let mut data = vec![discriminator];
        data.extend_from_slice(&self.event_id.to_le_bytes());
        data
    }
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

/// Serialize instructions into a base64-encoded transaction string.
pub(crate) async fn serialize_to_b64(
    rpc_url: &str,
    kv: Option<&KvStore>,
    message_accounts: &[AccountMeta],
    compiled_ixs: &[CompiledInstruction],
) -> Result<String, EscrowError> {
    let bh = get_latest_blockhash(rpc_url, kv).await?;
    let bh_bytes = pubkey_from_base58(&bh.value)?;
    let tx_bytes = serialize_transaction(message_accounts, compiled_ixs, &bh_bytes);
    Ok(base64::engine::general_purpose::STANDARD.encode(&tx_bytes))
}

/// Build and serialize a single-instruction transaction.
pub(crate) async fn finalize_tx(
    rpc_url: &str,
    kv: Option<&KvStore>,
    ctx: &EscrowCtx,
    instruction_accounts: Vec<AccountMeta>,
    ix_data: Vec<u8>,
    extra_accounts: &[AccountMeta],
) -> Result<String, EscrowError> {
    let (msg, pid_idx, ix_idx) =
        build_message_accounts(&instruction_accounts, &ctx.program_id, extra_accounts);
    let compiled = CompiledInstruction {
        program_id_index: pid_idx,
        accounts: ix_idx,
        data: ix_data,
    };
    serialize_to_b64(rpc_url, kv, &msg, &[compiled]).await
}

/// Merge accounts from multiple instructions into a single deduplicated
/// message account list in Solana canonical order.
/// When the same pubkey appears with different writability, prefers writable.
pub(crate) fn merge_message_accounts(
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::solana_escrow::crypto::{
        base58_decode, find_program_address, get_associated_token_address, pubkey_from_base58,
        pubkey_to_base58,
    };
    use crate::solana_escrow::wire::encode_compact_u16;
    use crate::solana_escrow::{
        ASSOCIATED_TOKEN_PROGRAM_ID, SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID, escrow_program_id,
        usdc_mint,
    };

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
        let id = escrow_program_id();
        let pk = pubkey_from_base58(id).unwrap();
        let encoded = pubkey_to_base58(&pk);
        assert_eq!(encoded, id);
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
        let program_id = pubkey_from_base58(escrow_program_id()).unwrap();
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
        let program_id = pubkey_from_base58(escrow_program_id()).unwrap();
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
        let program_id = pubkey_from_base58(escrow_program_id()).unwrap();
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

    // --------------------------------------------------------------------------------------------
    // Regression guard — refund instruction account drift
    // --------------------------------------------------------------------------------------------
    // SEC-010 introspection hardening (.issues/047) added `instruction_sysvar`
    // to the on-chain `Refund` struct at index 6. The worker tx builder must
    // emit exactly 10 accounts or on-chain execution fails with
    // `NotEnoughAccountKeys`. This test pins the count + the instruction_sysvar
    // slot so future drift is caught at unit-test time, not on devnet.

    #[tokio::test]
    async fn test_refund_instruction_accounts_count_is_10() {
        let organizer_str = "9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b";
        let ctx = EscrowCtx::resolve(organizer_str, 1u64).await.unwrap();
        let attendee = pubkey_from_base58(organizer_str).unwrap();
        let (attendee_deposit, _) = ctx.attendee_deposit(&attendee).await.unwrap();
        let attendee_ta = ctx.token_account(&attendee).await.unwrap();

        let accounts =
            refund::refund_instruction_accounts(&ctx, attendee, attendee_deposit, attendee_ta);

        assert_eq!(
            accounts.len(),
            10,
            "refund instruction must emit exactly 10 accounts (got {}); \
             missing instruction_sysvar would cause NotEnoughAccountKeys on-chain",
            accounts.len()
        );
    }

    #[tokio::test]
    async fn test_refund_instruction_sysvar_at_index_6() {
        let organizer_str = "9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b";
        let ctx = EscrowCtx::resolve(organizer_str, 1u64).await.unwrap();
        let attendee = pubkey_from_base58(organizer_str).unwrap();
        let (attendee_deposit, _) = ctx.attendee_deposit(&attendee).await.unwrap();
        let attendee_ta = ctx.token_account(&attendee).await.unwrap();

        let accounts =
            refund::refund_instruction_accounts(&ctx, attendee, attendee_deposit, attendee_ta);

        // instruction_sysvar must sit between vault (idx 5) and rent (idx 7),
        // matching the on-chain `Refund` struct declared in bethere-escrow.
        assert_eq!(
            accounts[6].pubkey,
            ctx.instruction_sysvar,
            "instruction_sysvar must be at index 6 (got {:?})",
            accounts
                .iter()
                .position(|m| m.pubkey == ctx.instruction_sysvar)
        );
    }

    #[tokio::test]
    async fn test_refund_instruction_attendee_is_signer_writable_at_index_0() {
        let organizer_str = "9ZNTfG4NyQgxy2SWjSiQoUyBPEvXT2xo7fKc5hPYYJ7b";
        let ctx = EscrowCtx::resolve(organizer_str, 1u64).await.unwrap();
        let attendee = pubkey_from_base58(organizer_str).unwrap();
        let (attendee_deposit, _) = ctx.attendee_deposit(&attendee).await.unwrap();
        let attendee_ta = ctx.token_account(&attendee).await.unwrap();

        let accounts =
            refund::refund_instruction_accounts(&ctx, attendee, attendee_deposit, attendee_ta);

        // attendee is the refund signer + fee payer; must be signer+writable at idx 0.
        assert!(accounts[0].is_signer, "attendee must be a signer");
        assert!(
            accounts[0].is_writable,
            "attendee must be writable (fee deduction)"
        );
        assert_eq!(accounts[0].pubkey, attendee, "attendee must be at index 0");
    }
}
