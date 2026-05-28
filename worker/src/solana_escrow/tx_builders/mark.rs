use worker::KvStore;

use super::super::crypto::pubkey_from_base58;
use super::super::{EscrowError, MarkCheckedInTransaction};
use super::{EscrowCtx, acct_sw, acct_w, finalize_tx};

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
    let ctx = EscrowCtx::resolve(organizer_pubkey, event_id).await?;
    let attendee = pubkey_from_base58(attendee_pubkey)?;
    let (attendee_deposit, _) = ctx.attendee_deposit(&attendee).await?;

    // Discriminator 2 (mark_checked_in).
    // Accounts: organizer(S,W), event_escrow(W), attendee_deposit(W)
    let instruction_accounts = vec![
        acct_sw(ctx.organizer),
        acct_w(ctx.event_escrow),
        acct_w(attendee_deposit),
    ];

    let tx_b64 = finalize_tx(rpc_url, kv, &ctx, instruction_accounts, ctx.ix_data(2), &[]).await?;

    Ok(MarkCheckedInTransaction {
        transaction_b64: tx_b64,
        message: "Mark attendee as checked in".to_string(),
    })
}
