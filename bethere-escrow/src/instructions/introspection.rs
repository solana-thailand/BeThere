//! Instruction introspection helpers for scanning sibling instructions.
//!
//! Uses the Instructions sysvar to inspect other instructions in the same
//! transaction. This enables program-level enforcement of structural
//! invariants (e.g., `refund` must be paired with `close_deposit`).
//!
//! All parsing is zero-allocation — we scan the sysvar data in-place.

use {crate::errors::EscrowError, quasar_lang::prelude::*};

/// Instructions sysvar address: `Sysvar1nstructions1111111111111111111111111`
const INSTRUCTIONS_SYSVAR_ID: [u8; 32] = [
    6, 167, 213, 23, 24, 123, 209, 102, 53, 218, 212, 4, 85, 253, 194, 192, 193, 36, 198, 143, 33,
    86, 117, 165, 219, 186, 203, 95, 8, 0, 0, 0,
];

/// Validate that the given account is the Instructions sysvar.
#[inline(always)]
pub fn validate_instruction_sysvar(account: &AccountView) -> Result<(), ProgramError> {
    if account.address().as_ref() != INSTRUCTIONS_SYSVAR_ID {
        return Err(ProgramError::UnsupportedSysvar);
    }
    Ok(())
}

/// Read the number of instructions in the transaction from sysvar data.
///
/// Layout: first 2 bytes = num_instructions (u16 LE).
#[inline(always)]
fn read_num_instructions(data: &[u8]) -> u16 {
    u16::from_le_bytes([data[0], data[1]])
}

/// Read the current instruction index from sysvar data.
///
/// Layout: last 2 bytes = current_index (u16 LE).
#[inline(always)]
fn read_current_index(data: &[u8]) -> u16 {
    let len = data.len();
    u16::from_le_bytes([data[len - 2], data[len - 1]])
}

/// Read the instruction offset table entry at the given index.
///
/// Layout: [2..2 + 2*N] = instruction_offsets ([u16; N]).
#[inline(always)]
fn read_instruction_offset(data: &[u8], index: usize) -> u16 {
    let start = 2 + index * 2;
    u16::from_le_bytes([data[start], data[start + 1]])
}

/// Discriminator for `close_deposit` instruction (index 7).
const CLOSE_DEPOSIT_DISCRIMINATOR: u8 = 7;

/// Discriminator for `rollover_deposit` instruction (index 8).
const ROLLOVER_DEPOSIT_DISCRIMINATOR: u8 = 8;

/// Check if an instruction at the given index has the specified discriminator,
/// is from the specified program, and references the target account.
///
/// Zero-allocation: scans the serialized sysvar data in-place.
fn instruction_matches(
    data: &[u8],
    index: usize,
    expected_discriminator: u8,
    program_id: &[u8; 32],
    target_account: &[u8; 32],
) -> Option<bool> {
    let num_instructions = read_num_instructions(data) as usize;
    if index >= num_instructions {
        return None;
    }

    let mut pos = read_instruction_offset(data, index) as usize;

    // num_accounts (u16)
    if pos + 2 > data.len() {
        return None;
    }
    let num_accounts = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
    pos += 2;

    // Skip accounts: each is 1 byte meta + 32 bytes pubkey = 33 bytes
    let accounts_start = pos;
    let accounts_end = accounts_start + num_accounts * 33;
    if accounts_end > data.len() {
        return None;
    }

    // Check if target_account appears in the accounts list
    let mut has_target = false;
    for i in 0..num_accounts {
        let key_offset = accounts_start + i * 33 + 1; // +1 to skip meta byte
        if data[key_offset..key_offset + 32] == target_account[..] {
            has_target = true;
            break;
        }
    }

    pos = accounts_end;

    // program_id (32 bytes)
    if pos + 32 > data.len() {
        return None;
    }
    let is_same_program = data[pos..pos + 32] == program_id[..];
    pos += 32;

    // data_len (u16)
    if pos + 2 > data.len() {
        return None;
    }
    let _data_len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
    pos += 2;

    // Check discriminator (first byte of instruction data)
    if pos >= data.len() {
        return None;
    }
    let disc_matches = data[pos] == expected_discriminator;

    Some(is_same_program && disc_matches && has_target)
}

/// Check if a `close_deposit` instruction follows the current instruction
/// in the same transaction, targeting the given attendee deposit account.
///
/// Scans from current_index+1 to end of instruction list.
#[inline(always)]
pub fn has_close_deposit_for(
    data: &[u8],
    program_id: &[u8; 32],
    attendee_deposit: &[u8; 32],
) -> bool {
    let current_idx = read_current_index(data) as usize;
    let num_instructions = read_num_instructions(data) as usize;

    for idx in (current_idx + 1)..num_instructions {
        if let Some(true) = instruction_matches(
            data,
            idx,
            CLOSE_DEPOSIT_DISCRIMINATOR,
            program_id,
            attendee_deposit,
        ) {
            return true;
        }
    }
    false
}

/// Check if there is a `rollover_deposit` instruction anywhere in the transaction
/// for the given attendee deposit account.
#[inline(always)]
pub fn has_rollover_deposit_for(
    data: &[u8],
    program_id: &[u8; 32],
    attendee_deposit: &[u8; 32],
) -> bool {
    let num_instructions = read_num_instructions(data) as usize;

    for idx in 0..num_instructions {
        if let Some(true) = instruction_matches(
            data,
            idx,
            ROLLOVER_DEPOSIT_DISCRIMINATOR,
            program_id,
            attendee_deposit,
        ) {
            return true;
        }
    }
    false
}

/// Require that a `close_deposit` instruction follows the current `refund`
/// instruction in the same transaction, targeting the same attendee deposit.
///
/// # Errors
///
/// Returns [`EscrowError::RefundRequiresClose`] if no matching `close_deposit`
/// is found in the remaining instructions.
#[inline(always)]
pub fn require_close_deposit_follows_refund(
    instruction_sysvar: &UncheckedAccount,
    attendee_deposit: &Address,
) -> Result<(), ProgramError> {
    let view = instruction_sysvar.to_account_view();
    validate_instruction_sysvar(view)?;

    let data = view.try_borrow()?;
    let program_id_bytes = crate::ID.to_bytes();
    let deposit_bytes = attendee_deposit.to_bytes();

    if has_close_deposit_for(&data, &program_id_bytes, &deposit_bytes) {
        Ok(())
    } else {
        Err(EscrowError::RefundRequiresClose.into())
    }
}
