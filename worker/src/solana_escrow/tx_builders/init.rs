use worker::KvStore;

use super::super::EscrowError;
use super::super::InitEscrowTransaction;
use super::super::PubkeyBytes;
use super::super::crypto::pubkey_to_base58;
use super::super::wire::CompiledInstruction;
use super::{EscrowCtx, acct_r, acct_sw, acct_w, merge_message_accounts, serialize_to_b64};

/// Build a single transaction containing BOTH:
/// 1. `create_associated_token_account_idempotent` (ATA program) — creates vault ATA if not exists
/// 2. `create_event` (escrow program) — initializes the on-chain escrow PDA
///
/// This replaces the two-step approach (create-vault-ata → create-event) with
/// a single transaction that the organizer signs once.
///
/// # Arguments
/// * `rpc_url` — Solana RPC URL (with API key if needed)
/// * `organizer_pubkey` — Organizer's wallet address (base58), signer + payer
/// * `event_id` — Numeric event ID used for PDA derivation
/// * `deposit_amount` — USDC deposit amount in lamports (6 decimals)
/// * `event_end` — Event end time as unix timestamp (seconds)
/// * `refund_deadline` — Refund deadline as unix timestamp (seconds)
pub async fn build_init_escrow_transaction(
    rpc_url: &str,
    kv: Option<&KvStore>,
    organizer_pubkey: &str,
    event_id: u64,
    deposit_amount: u64,
    event_end: i64,
    refund_deadline: i64,
) -> Result<InitEscrowTransaction, EscrowError> {
    let ctx = EscrowCtx::resolve(organizer_pubkey, event_id).await?;

    // Instruction 1: create_associated_token_account_idempotent (ATA program)
    // Accounts: organizer(S,W), vault(W), event_escrow(R), usdc_mint(R),
    //   system_program(R), token_program(R)
    let ata_ix_accounts = vec![
        acct_sw(ctx.organizer),
        acct_w(ctx.vault),
        acct_r(ctx.event_escrow),
        acct_r(ctx.usdc_mint),
        acct_r(ctx.system_program),
        acct_r(ctx.token_program),
    ];
    let ata_ix_data: Vec<u8> = vec![1]; // CreateIdempotent discriminator

    // Instruction 2: create_event (escrow program)
    // Accounts: organizer(S,W), event_escrow(W), usdc_mint(R), vault(W),
    //   rent_sysvar(R), token_program(R), system_program(R)
    let mut escrow_ix_data = ctx.ix_data(0); // [0] + event_id
    escrow_ix_data.extend_from_slice(&deposit_amount.to_le_bytes());
    escrow_ix_data.extend_from_slice(&event_end.to_le_bytes());
    escrow_ix_data.extend_from_slice(&refund_deadline.to_le_bytes());

    let escrow_ix_accounts = vec![
        acct_sw(ctx.organizer),
        acct_w(ctx.event_escrow),
        acct_r(ctx.usdc_mint),
        acct_w(ctx.vault),
        acct_r(ctx.rent_sysvar),
        acct_r(ctx.token_program),
        acct_r(ctx.system_program),
    ];

    // Merge accounts from both instructions into a single message
    let message_accounts = merge_message_accounts(
        &[&ata_ix_accounts, &escrow_ix_accounts],
        &[ctx.ata_program, ctx.program_id],
    );

    // Build index lookup
    let get_index = |pk: &PubkeyBytes| -> u8 {
        message_accounts
            .iter()
            .position(|m| &m.pubkey == pk)
            .expect("all accounts should be in message") as u8
    };

    let ata_compiled_ix = CompiledInstruction {
        program_id_index: get_index(&ctx.ata_program),
        accounts: ata_ix_accounts
            .iter()
            .map(|m| get_index(&m.pubkey))
            .collect(),
        data: ata_ix_data,
    };
    let escrow_compiled_ix = CompiledInstruction {
        program_id_index: get_index(&ctx.program_id),
        accounts: escrow_ix_accounts
            .iter()
            .map(|m| get_index(&m.pubkey))
            .collect(),
        data: escrow_ix_data,
    };

    let tx_b64 = serialize_to_b64(
        rpc_url,
        kv,
        &message_accounts,
        &[ata_compiled_ix, escrow_compiled_ix],
    )
    .await?;
    let escrow_address = pubkey_to_base58(&ctx.event_escrow);
    let vault_address = pubkey_to_base58(&ctx.vault);
    let amount_display = deposit_amount as f64 / 1_000_000.0;

    Ok(InitEscrowTransaction {
        transaction_b64: tx_b64,
        message: format!(
            "Init escrow: create vault + create event ({amount_display:.2} USDC deposit)"
        ),
        escrow_address,
        vault_address,
    })
}
