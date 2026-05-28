use worker::KvStore;

use super::super::crypto::pubkey_from_base58;
use super::super::{EscrowError, RolloverDepositTransaction};
use super::{EscrowCtx, acct_r, acct_sw, acct_w, finalize_tx};

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
pub async fn build_rollover_deposit_transaction(
    rpc_url: &str,
    kv: Option<&KvStore>,
    organizer_pubkey: &str,
    source_event_id: u64,
    target_event_id: u64,
    attendee_pubkey: &str,
) -> Result<RolloverDepositTransaction, EscrowError> {
    let source_ctx = EscrowCtx::resolve(organizer_pubkey, source_event_id).await?;
    let target_ctx = EscrowCtx::resolve(organizer_pubkey, target_event_id).await?;
    let attendee = pubkey_from_base58(attendee_pubkey)?;

    // Derive PDAs for both source and target
    let (source_deposit, _) = source_ctx.attendee_deposit(&attendee).await?;
    let (target_deposit, _) = target_ctx.attendee_deposit(&attendee).await?;

    // Discriminator 8 (rollover_deposit). Accounts in program-expected order:
    //   attendee(S,W), source_escrow(W), source_deposit(W), source_vault(W),
    //   target_escrow(W), target_deposit(W), target_vault(W),
    //   deposit_mint(R), rent(R), token_program(R), system_program(R)
    let instruction_accounts = vec![
        acct_sw(attendee),
        acct_w(source_ctx.event_escrow),
        acct_w(source_deposit),
        acct_w(source_ctx.vault),
        acct_w(target_ctx.event_escrow),
        acct_w(target_deposit),
        acct_w(target_ctx.vault),
        acct_r(source_ctx.usdc_mint),
        acct_r(source_ctx.rent_sysvar),
        acct_r(source_ctx.token_program),
        acct_r(source_ctx.system_program),
    ];

    // Instruction data: [8] + [source_event_id u64 LE] + [target_event_id u64 LE]
    let mut ix_data = vec![8];
    ix_data.extend_from_slice(&source_event_id.to_le_bytes());
    ix_data.extend_from_slice(&target_event_id.to_le_bytes());

    // ATA program for CPI init of target_deposit
    let extra = vec![acct_r(target_ctx.ata_program)];

    let tx_b64 = finalize_tx(
        rpc_url,
        kv,
        &source_ctx,
        instruction_accounts,
        ix_data,
        &extra,
    )
    .await?;

    Ok(RolloverDepositTransaction {
        transaction_b64: tx_b64,
        message: format!("Roll deposit from event {source_event_id} to event {target_event_id}"),
    })
}
