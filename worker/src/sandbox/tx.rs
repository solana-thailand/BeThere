//! The two sandbox transactions the escrow builders do not cover: the faucet
//! grant and the visitor's return of the test USDC. Both are plain System,
//! Associated Token and SPL Token instructions, compiled into one legacy
//! message with the same canonical account order as the escrow builders.

use crate::solana_escrow::tx_builders::{acct_r, acct_sw, acct_w, merge_message_accounts};
use crate::solana_escrow::wire::{AccountMeta, CompiledInstruction, serialize_transaction};
use crate::solana_escrow::{
    ASSOCIATED_TOKEN_PROGRAM_ID, EscrowError, SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID,
    get_associated_token_address, pubkey_from_base58,
};

/// Decimals of Circle's devnet USDC (and of USDC everywhere).
pub const USDC_DECIMALS: u8 = 6;

/// System program `Transfer`.
const SYSTEM_TRANSFER: u32 = 2;
/// Associated Token program `CreateIdempotent`.
const ATA_CREATE_IDEMPOTENT: u8 = 1;
/// SPL Token `TransferChecked`.
const TOKEN_TRANSFER_CHECKED: u8 = 12;

/// One instruction before account indices are resolved.
struct Ix {
    program_id: [u8; 32],
    accounts: Vec<AccountMeta>,
    data: Vec<u8>,
}

/// Merge the instructions' accounts into one message and serialize it with
/// zeroed signature slots.
fn compile(ixs: &[Ix], blockhash: &[u8; 32]) -> Vec<u8> {
    let lists: Vec<&[AccountMeta]> = ixs.iter().map(|ix| ix.accounts.as_slice()).collect();
    let program_ids: Vec<[u8; 32]> = ixs.iter().map(|ix| ix.program_id).collect();
    let metas = merge_message_accounts(&lists, &program_ids);
    let index = |key: &[u8; 32]| -> u8 {
        metas
            .iter()
            .position(|m| &m.pubkey == key)
            .expect("merge_message_accounts keeps every key") as u8
    };
    let compiled: Vec<CompiledInstruction> = ixs
        .iter()
        .map(|ix| CompiledInstruction {
            program_id_index: index(&ix.program_id),
            accounts: ix.accounts.iter().map(|m| index(&m.pubkey)).collect(),
            data: ix.data.clone(),
        })
        .collect();
    serialize_transaction(&metas, &compiled, blockhash)
}

fn transfer_checked(
    source: [u8; 32],
    mint: [u8; 32],
    destination: [u8; 32],
    authority: [u8; 32],
    amount: u64,
) -> Result<Ix, EscrowError> {
    let mut data = vec![TOKEN_TRANSFER_CHECKED];
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(USDC_DECIMALS);
    Ok(Ix {
        program_id: pubkey_from_base58(TOKEN_PROGRAM_ID)?,
        accounts: vec![
            acct_w(source),
            acct_r(mint),
            acct_w(destination),
            acct_sw(authority),
        ],
        data,
    })
}

/// The faucet grant, signed by the faucet alone: `lamports` of SOL for the
/// visitor's fees and deposit rent, the visitor's USDC account (created if
/// missing, paid by the faucet), and `amount` USDC into it.
pub async fn faucet_grant_tx(
    faucet: &[u8; 32],
    recipient: &[u8; 32],
    mint: &[u8; 32],
    lamports: u64,
    amount: u64,
    blockhash: &[u8; 32],
) -> Result<Vec<u8>, EscrowError> {
    let system_program = pubkey_from_base58(SYSTEM_PROGRAM_ID)?;
    let token_program = pubkey_from_base58(TOKEN_PROGRAM_ID)?;
    let faucet_ata = get_associated_token_address(faucet, mint).await?;
    let recipient_ata = get_associated_token_address(recipient, mint).await?;

    let mut sol_data = SYSTEM_TRANSFER.to_le_bytes().to_vec();
    sol_data.extend_from_slice(&lamports.to_le_bytes());
    let sol = Ix {
        program_id: system_program,
        accounts: vec![acct_sw(*faucet), acct_w(*recipient)],
        data: sol_data,
    };
    let create_ata = Ix {
        program_id: pubkey_from_base58(ASSOCIATED_TOKEN_PROGRAM_ID)?,
        accounts: vec![
            acct_sw(*faucet),
            acct_w(recipient_ata),
            acct_r(*recipient),
            acct_r(*mint),
            acct_r(system_program),
            acct_r(token_program),
        ],
        data: vec![ATA_CREATE_IDEMPOTENT],
    };
    let usdc = transfer_checked(faucet_ata, *mint, recipient_ata, *faucet, amount)?;
    Ok(compile(&[sol, create_ata, usdc], blockhash))
}

/// The visitor's return of `amount` test USDC to the faucet, signed and paid
/// for by the visitor. The faucet's USDC account exists: it funded the grant.
pub async fn token_return_tx(
    owner: &[u8; 32],
    faucet: &[u8; 32],
    mint: &[u8; 32],
    amount: u64,
    blockhash: &[u8; 32],
) -> Result<Vec<u8>, EscrowError> {
    let owner_ata = get_associated_token_address(owner, mint).await?;
    let faucet_ata = get_associated_token_address(faucet, mint).await?;
    let usdc = transfer_checked(owner_ata, *mint, faucet_ata, *owner, amount)?;
    Ok(compile(&[usdc], blockhash))
}
