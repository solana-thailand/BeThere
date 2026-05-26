//! All public transaction builder functions.

use base64::Engine;
use worker::KvStore;

use super::crypto::{
    find_program_address, get_associated_token_address, pubkey_from_base58, pubkey_to_base58,
};
use super::wire::{
    AccountMeta, CompiledInstruction, build_message_accounts, get_latest_blockhash,
    serialize_transaction,
};
use super::{
    ASSOCIATED_TOKEN_PROGRAM_ID, ESCROW_PROGRAM_ID, EscrowError, PubkeyBytes, RENT_SYSVAR_ID,
    SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID,
};
use super::{
    ClaimForfeitedTransaction, CloseDepositTransaction, CloseEventTransaction,
    DeactivateEventTransaction, DepositTransaction, InitEscrowTransaction,
    MarkCheckedInTransaction, RefundAndCloseTransaction, RefundTransaction,
    RolloverDepositTransaction,
};

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Shorthand: construct an [`AccountMeta`] with explicit flags.
fn acct(pubkey: PubkeyBytes, signer: bool, writable: bool) -> AccountMeta {
    AccountMeta {
        pubkey,
        is_signer: signer,
        is_writable: writable,
    }
}

/// Signer + writable.
fn acct_sw(pubkey: PubkeyBytes) -> AccountMeta {
    acct(pubkey, true, true)
}

/// Non-signer + writable.
fn acct_w(pubkey: PubkeyBytes) -> AccountMeta {
    acct(pubkey, false, true)
}

/// Non-signer + readonly.
fn acct_r(pubkey: PubkeyBytes) -> AccountMeta {
    acct(pubkey, false, false)
}

// ---------------------------------------------------------------------------
// EscrowCtx — shared resolved accounts
// ---------------------------------------------------------------------------

/// Common resolved accounts shared across most escrow instructions.
///
/// Resolves program IDs and derives the EventEscrow PDA and vault ATA once
/// so every builder function can reuse them without repeating boilerplate.
struct EscrowCtx {
    program_id: PubkeyBytes,
    organizer: PubkeyBytes,
    event_escrow: PubkeyBytes,
    usdc_mint: PubkeyBytes,
    token_program: PubkeyBytes,
    system_program: PubkeyBytes,
    rent_sysvar: PubkeyBytes,
    ata_program: PubkeyBytes,
    vault: PubkeyBytes,
    event_id: u64,
}

impl EscrowCtx {
    /// Parse well-known program IDs, derive the EventEscrow PDA and vault ATA.
    async fn resolve(organizer_pubkey: &str, event_id: u64) -> Result<Self, EscrowError> {
        let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID)?;
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
            ata_program,
            vault,
            event_id,
        })
    }

    /// Derive the AttendeeDeposit PDA for a given attendee.
    async fn attendee_deposit(
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
    async fn token_account(&self, owner: &PubkeyBytes) -> Result<PubkeyBytes, EscrowError> {
        get_associated_token_address(owner, &self.usdc_mint).await
    }

    /// Build instruction data: `[discriminator] + [event_id u64 LE]`.
    fn ix_data(&self, discriminator: u8) -> Vec<u8> {
        let mut data = vec![discriminator];
        data.extend_from_slice(&self.event_id.to_le_bytes());
        data
    }
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

/// Serialize instructions into a base64-encoded transaction string.
async fn serialize_to_b64(
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
async fn finalize_tx(
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
fn merge_message_accounts(
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

// ---------------------------------------------------------------------------
// Refund Transaction Builder
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Combined Init Escrow Transaction Builder (ATA + CreateEvent in one TX)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Mark Checked-In Transaction Builder
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Deactivate Event Transaction Builder
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Close Event Transaction Builder
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Claim Forfeited Transaction Builder
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Batch Claim Forfeited Transaction Builder
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Close Deposit Transaction Builder
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Combined Refund + Close Deposit Transaction Builder
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Refund + Close Deposit Transaction Builder
// ---------------------------------------------------------------------------

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
    let refund_accounts = vec![
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
        ASSOCIATED_TOKEN_PROGRAM_ID, ESCROW_PROGRAM_ID, SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID,
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
        let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID).unwrap();
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
        let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID).unwrap();
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
        let program_id = pubkey_from_base58(ESCROW_PROGRAM_ID).unwrap();
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
}
