use worker::KvStore;

use super::super::crypto::pubkey_from_base58;
use super::super::wire::{AccountMeta, CompiledInstruction};
use super::super::{
    ClaimForfeitedTransaction, EscrowError, PubkeyBytes, RefundAndCloseTransaction,
    RefundTransaction,
};
use super::{
    EscrowCtx, acct_r, acct_sw, acct_w, finalize_tx, merge_message_accounts, serialize_to_b64,
};

/// Build the account list for the `refund` instruction (discriminator 3).
///
/// Must match the on-chain `Refund` struct account order exactly:
///   attendee, event_escrow, deposit_mint, attendee_deposit, attendee_ta,
///   vault, instruction_sysvar, rent, token_program, system_program
///
/// Count must remain 10. The SEC-010 introspection hardening added
/// `instruction_sysvar` at index 6 — removing it causes `NotEnoughAccountKeys`
/// on-chain. Regression-tested in `tx_builders/mod.rs::tests`.
pub(crate) fn refund_instruction_accounts(
    ctx: &EscrowCtx,
    attendee: PubkeyBytes,
    attendee_deposit: PubkeyBytes,
    attendee_ta: PubkeyBytes,
) -> Vec<AccountMeta> {
    vec![
        acct_sw(attendee),
        acct_w(ctx.event_escrow),
        acct_r(ctx.usdc_mint),
        acct_w(attendee_deposit),
        acct_w(attendee_ta),
        acct_w(ctx.vault),
        acct_r(ctx.instruction_sysvar),
        acct_r(ctx.rent_sysvar),
        acct_r(ctx.token_program),
        acct_r(ctx.system_program),
    ]
}

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
    let ctx = EscrowCtx::resolve(organizer_pubkey, event_id).await?;
    let attendee = pubkey_from_base58(attendee_pubkey)?;
    let (attendee_deposit, _) = ctx.attendee_deposit(&attendee).await?;
    let attendee_ta = ctx.token_account(&attendee).await?;

    // Discriminator 3 (refund). No `organizer` account — the attendee signs.
    // The event_escrow PDA authorizes the vault → attendee_ta transfer.
    let instruction_accounts =
        refund_instruction_accounts(&ctx, attendee, attendee_deposit, attendee_ta);

    // ATA program needed for CPI (init idempotent on attendee_ta).
    let extra = vec![acct_r(ctx.ata_program)];

    let tx_b64 = finalize_tx(
        rpc_url,
        kv,
        &ctx,
        instruction_accounts,
        ctx.ix_data(3),
        &extra,
    )
    .await?;

    Ok(RefundTransaction {
        transaction_b64: tx_b64,
        message: "Claim refund from event escrow".to_string(),
    })
}

/// Build a combined refund + close_deposit transaction.
///
/// Instruction 1: Refund (discriminator 3) — transfers USDC from vault to attendee.
/// Instruction 2: Close Deposit (discriminator 7) — reclaims rent from AttendeeDeposit PDA.
///
/// Both instructions are combined into a single atomic transaction.
pub async fn build_refund_and_close_transaction(
    rpc_url: &str,
    kv: Option<&KvStore>,
    organizer_pubkey: &str,
    event_id: u64,
    attendee_pubkey: &str,
) -> Result<RefundAndCloseTransaction, EscrowError> {
    let ctx = EscrowCtx::resolve(organizer_pubkey, event_id).await?;
    let attendee = pubkey_from_base58(attendee_pubkey)?;
    let (attendee_deposit, _) = ctx.attendee_deposit(&attendee).await?;
    let attendee_ta = ctx.token_account(&attendee).await?;

    // Instruction 1: Refund (discriminator 3)
    // Account order is enforced by `refund_instruction_accounts` (see that
    // function for the canonical on-chain `Refund` struct ordering).
    let refund_accounts =
        refund_instruction_accounts(&ctx, attendee, attendee_deposit, attendee_ta);

    // Instruction 2: Close Deposit (discriminator 7)
    let close_accounts = vec![
        acct_sw(attendee),
        acct_r(ctx.event_escrow),
        acct_w(attendee_deposit),
        acct_r(ctx.system_program),
    ];

    // Merge accounts from both instructions into a single ordered message
    let message_accounts = merge_message_accounts(
        &[&refund_accounts, &close_accounts],
        &[ctx.program_id, ctx.ata_program],
    );

    // Build index lookup
    let get_index = |pk: &PubkeyBytes| -> u8 {
        message_accounts
            .iter()
            .position(|m| &m.pubkey == pk)
            .expect("all accounts should be in merged message") as u8
    };

    let program_id_index = get_index(&ctx.program_id);

    let refund_ix = CompiledInstruction {
        program_id_index,
        accounts: refund_accounts
            .iter()
            .map(|m| get_index(&m.pubkey))
            .collect(),
        data: ctx.ix_data(3),
    };
    let close_ix = CompiledInstruction {
        program_id_index,
        accounts: close_accounts
            .iter()
            .map(|m| get_index(&m.pubkey))
            .collect(),
        data: ctx.ix_data(7),
    };

    let tx_b64 = serialize_to_b64(rpc_url, kv, &message_accounts, &[refund_ix, close_ix]).await?;

    Ok(RefundAndCloseTransaction {
        transaction_b64: tx_b64,
        message: "Refund USDC and reclaim deposit rent in one transaction".to_string(),
    })
}

/// Build a single transaction containing `claim_forfeited` instructions for
/// multiple no-show attendees.
///
/// All instructions share the same organizer, event_escrow, vault, and program
/// accounts. Each attendee gets their own AttendeeDeposit PDA resolved.
///
/// # Arguments
/// * `rpc_url` — Solana RPC URL (with API key if needed)
/// * `kv` — Optional KV store for blockhash caching
/// * `organizer_pubkey` — Organizer's wallet address (base58), must be signer
/// * `event_id` — Numeric event ID used for PDA derivation
/// * `attendee_pubkeys` — List of attendee wallet addresses (base58) to claim
///
/// # Errors
/// Returns `EscrowError` if the list is empty or any pubkey is invalid.
pub async fn build_batch_claim_forfeited_transaction(
    rpc_url: &str,
    kv: Option<&KvStore>,
    organizer_pubkey: &str,
    event_id: u64,
    attendee_pubkeys: &[String],
) -> Result<ClaimForfeitedTransaction, EscrowError> {
    if attendee_pubkeys.is_empty() {
        return Err(EscrowError::AccountNotFound(
            "no forfeited deposits to claim".to_string(),
        ));
    }

    let ctx = EscrowCtx::resolve(organizer_pubkey, event_id).await?;
    let organizer_ta = ctx.token_account(&ctx.organizer).await?;

    // Shared accounts that appear in every claim_forfeited instruction.
    // organizer_ta, rent, token_program, system_program are appended as extra.
    let extra = vec![acct_r(ctx.ata_program)];

    // Build per-attendee account lists and resolve PDAs
    let mut instruction_account_lists: Vec<Vec<AccountMeta>> =
        Vec::with_capacity(attendee_pubkeys.len());

    for ap in attendee_pubkeys {
        let attendee = pubkey_from_base58(ap)?;
        let (attendee_deposit, _) = ctx.attendee_deposit(&attendee).await?;

        // Discriminator 4 (claim_forfeited).
        // Accounts: organizer(S,W), event_escrow(W), attendee_deposit(W),
        //   organizer_ta(W, init idempotent), usdc_mint(R), vault(W),
        //   rent(R), token_program(R), system_program(R)
        let accounts = vec![
            acct_sw(ctx.organizer),
            acct_w(ctx.event_escrow),
            acct_w(attendee_deposit),
            acct_w(organizer_ta),
            acct_r(ctx.usdc_mint),
            acct_w(ctx.vault),
            acct_r(ctx.rent_sysvar),
            acct_r(ctx.token_program),
            acct_r(ctx.system_program),
        ];
        instruction_account_lists.push(accounts);
    }

    // Merge all instruction account lists into a single deduplicated message
    let refs: Vec<&[AccountMeta]> = instruction_account_lists
        .iter()
        .map(|v| v.as_slice())
        .collect();
    let message_accounts = merge_message_accounts(&refs, &[ctx.program_id]);

    // Append extra accounts (ATA program for CPI)
    let mut message_accounts = message_accounts;
    for e in &extra {
        if !message_accounts.iter().any(|ma| ma.pubkey == e.pubkey) {
            message_accounts.push(AccountMeta {
                pubkey: e.pubkey,
                is_signer: e.is_signer,
                is_writable: e.is_writable,
            });
        }
    }

    // Build index lookup
    let get_index = |pk: &PubkeyBytes| -> u8 {
        message_accounts
            .iter()
            .position(|m| &m.pubkey == pk)
            .expect("all accounts should be in merged message") as u8
    };

    let program_id_index = get_index(&ctx.program_id);

    // Build compiled instructions
    let compiled_ixs: Vec<CompiledInstruction> = instruction_account_lists
        .iter()
        .map(|accounts| CompiledInstruction {
            program_id_index,
            accounts: accounts.iter().map(|m| get_index(&m.pubkey)).collect(),
            data: ctx.ix_data(4),
        })
        .collect();

    let tx_b64 = serialize_to_b64(rpc_url, kv, &message_accounts, &compiled_ixs).await?;

    Ok(ClaimForfeitedTransaction {
        transaction_b64: tx_b64,
        message: format!(
            "Claim forfeited deposits from {} no-show(s)",
            attendee_pubkeys.len()
        ),
    })
}
