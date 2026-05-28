use worker::KvStore;

use super::super::crypto::pubkey_from_base58;
use super::super::{
    ClaimForfeitedTransaction, CloseDepositTransaction, CloseEventTransaction,
    DeactivateEventTransaction, EscrowError,
};
use super::{EscrowCtx, acct_sw, acct_w, acct_r, finalize_tx};

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
    let ctx = EscrowCtx::resolve(organizer_pubkey, event_id).await?;

    // Discriminator 6 (deactivate_event).
    // Accounts: organizer(S,W), event_escrow(W)
    let instruction_accounts = vec![acct_sw(ctx.organizer), acct_w(ctx.event_escrow)];

    let tx_b64 = finalize_tx(rpc_url, kv, &ctx, instruction_accounts, ctx.ix_data(6), &[]).await?;

    Ok(DeactivateEventTransaction {
        transaction_b64: tx_b64,
        message: "Deactivate event — stop accepting deposits".to_string(),
    })
}

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
    let ctx = EscrowCtx::resolve(organizer_pubkey, event_id).await?;

    // Discriminator 5 (close_event).
    // Accounts: organizer(S,W), event_escrow(W), vault(W), token_program(R)
    let instruction_accounts = vec![
        acct_sw(ctx.organizer),
        acct_w(ctx.event_escrow),
        acct_w(ctx.vault),
        acct_r(ctx.token_program),
    ];

    let tx_b64 = finalize_tx(rpc_url, kv, &ctx, instruction_accounts, ctx.ix_data(5), &[]).await?;

    Ok(CloseEventTransaction {
        transaction_b64: tx_b64,
        message: "Close event escrow and reclaim rent".to_string(),
    })
}

/// Build a serialized `claim_forfeited` transaction for the bethere-escrow program.
///
/// Transfers forfeited USDC (deposits from no-shows) from the vault to the
/// organizer's USDC token account. Only callable after refund_deadline has passed.
///
/// # Arguments
/// * `rpc_url` — Solana RPC URL (with API key if needed)
/// * `kv` — Optional KV store for blockhash caching
/// * `organizer_pubkey` — Organizer's wallet address (base58), must be signer
/// * `attendee_pubkey` — Attendee's wallet address (base58) for deposit derivation
/// * `event_id` — Numeric event ID used for PDA derivation
///
/// # Discriminator
/// 4 (claim_forfeited)
///
/// # Accounts (ClaimForfeited)
///   0. organizer (signer, writable)
///   1. event_escrow (writable, PDA)
///   2. attendee_deposit (writable, PDA)
///   3. organizer_ta (writable, init idempotent)
///   4. usdc_mint (readonly)
///   5. vault (writable, Token account)
///   6. rent (readonly)
///   7. token_program (readonly)
///   8. system_program (readonly)
#[allow(dead_code)]
pub async fn build_claim_forfeited_transaction(
    rpc_url: &str,
    kv: Option<&KvStore>,
    organizer_pubkey: &str,
    attendee_pubkey: &str,
    event_id: u64,
) -> Result<ClaimForfeitedTransaction, EscrowError> {
    let ctx = EscrowCtx::resolve(organizer_pubkey, event_id).await?;
    let attendee = pubkey_from_base58(attendee_pubkey)?;
    let (attendee_deposit, _) = ctx.attendee_deposit(&attendee).await?;
    let organizer_ta = ctx.token_account(&ctx.organizer).await?;

    // Discriminator 4 (claim_forfeited).
    // Accounts: organizer(S,W), event_escrow(W), attendee_deposit(W),
    //   organizer_ta(W, init idempotent), usdc_mint(R), vault(W),
    //   rent(R), token_program(R), system_program(R)
    let instruction_accounts = vec![
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

    // ATA program needed for CPI (init idempotent on organizer_ta).
    let extra = vec![acct_r(ctx.ata_program)];

    let tx_b64 = finalize_tx(
        rpc_url,
        kv,
        &ctx,
        instruction_accounts,
        ctx.ix_data(4),
        &extra,
    )
    .await?;

    Ok(ClaimForfeitedTransaction {
        transaction_b64: tx_b64,
        message: "Claim forfeited deposits from no-shows".to_string(),
    })
}

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
    let ctx = EscrowCtx::resolve(organizer_pubkey, event_id).await?;
    let attendee = pubkey_from_base58(attendee_pubkey)?;
    let (attendee_deposit, _) = ctx.attendee_deposit(&attendee).await?;

    // Discriminator 7 (close_deposit).
    // Accounts: attendee(S,W), event_escrow(R), attendee_deposit(W), system_program(R)
    let instruction_accounts = vec![
        acct_sw(attendee),
        acct_r(ctx.event_escrow),
        acct_w(attendee_deposit),
        acct_r(ctx.system_program),
    ];

    let tx_b64 = finalize_tx(rpc_url, kv, &ctx, instruction_accounts, ctx.ix_data(7), &[]).await?;

    Ok(CloseDepositTransaction {
        transaction_b64: tx_b64,
        message: "Close deposit account and reclaim rent".to_string(),
    })
}
