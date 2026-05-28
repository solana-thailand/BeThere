use worker::KvStore;

use super::super::crypto::pubkey_from_base58;
use super::super::{DepositTransaction, EscrowError};
use super::{EscrowCtx, acct_sw, acct_w, acct_r, finalize_tx};

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
    let ctx = EscrowCtx::resolve(organizer_pubkey, event_id).await?;
    let attendee = pubkey_from_base58(attendee_pubkey)?;
    let (attendee_deposit, _) = ctx.attendee_deposit(&attendee).await?;
    let attendee_ta = ctx.token_account(&attendee).await?;

    // Discriminator 1 (deposit). Accounts in program-expected order:
    //   attendee(S,W), event_escrow(W), usdc_mint(R), attendee_deposit(W),
    //   attendee_ta(W), vault(W), rent(R), token_program(R), system_program(R)
    let instruction_accounts = vec![
        acct_sw(attendee),
        acct_w(ctx.event_escrow),
        acct_r(ctx.usdc_mint),
        acct_w(attendee_deposit),
        acct_w(attendee_ta),
        acct_w(ctx.vault),
        acct_r(ctx.rent_sysvar),
        acct_r(ctx.token_program),
        acct_r(ctx.system_program),
    ];

    let tx_b64 = finalize_tx(rpc_url, kv, &ctx, instruction_accounts, ctx.ix_data(1), &[]).await?;
    let amount_display = deposit_amount as f64 / 1_000_000.0;

    Ok(DepositTransaction {
        transaction_b64: tx_b64,
        message: format!("Deposit {amount_display:.2} USDC to event escrow"),
    })
}
