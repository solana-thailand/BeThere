use quasar_lang::prelude::*;

use crate::errors::EscrowError;

/// Current schema version for EventEscrow.
pub const ESCROW_VERSION: u8 = 1;

/// Current schema version for AttendeeDeposit.
pub const DEPOSIT_VERSION: u8 = 1;

/// PDA — one per event, holds all deposits in a token account.
/// Seeds: ["escrow", organizer, event_id]
///
/// Space (v1):
///   1 (discriminator) + 1 (version) + 32 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8 + 1 + 1 + 36 (padding) = 192 bytes
#[account(discriminator = 1, set_inner)]
#[seeds(b"escrow", organizer: Address, event_id: u64)]
pub struct EventEscrow {
    /// Schema version for future migrations.
    pub version: u8,
    /// Event organizer (can claim forfeited deposits).
    pub organizer: Address,
    /// Unique event identifier for PDA seed derivation.
    pub event_id: u64,
    /// Deposit mint address (USDC, USDG, or any SPL token).
    pub deposit_mint: Address,
    /// Token account owned by this PDA — holds deposited tokens.
    pub vault: Address,
    /// Fixed deposit amount in smallest unit (e.g., 6 decimals -> $15 = 15_000_000).
    pub deposit_amount: u64,
    /// Event end timestamp (unix seconds). Refunds allowed after this.
    pub event_end: i64,
    /// Refund deadline (event_end + grace period, e.g., +7 days).
    /// Only applies to no-shows (checked_in == false).
    /// Checked-in attendees can refund anytime after event_end.
    pub refund_deadline: i64,
    /// Total tokens deposited across all attendees.
    pub total_deposited: u64,
    /// Total tokens refunded.
    pub total_refunded: u64,
    /// Total tokens claimed by organizer (forfeited no-show deposits).
    pub total_forfeited: u64,
    /// Whether the event is active (deposits accepted).
    pub is_active: bool,
    /// Bump seed for PDA.
    pub bump: u8,
    /// Reserved padding for future fields (36 bytes).
    pub _padding: [u8; 36],
}

/// PDA — one per attendee per event.
/// Seeds: ["deposit", event, attendee]
///
/// Space (v1):
///   1 (discriminator) + 1 (version) + 32 + 32 + 8 + 8 + 1 + 1 + 1 + 11 (padding) = 96 bytes
#[account(discriminator = 2, set_inner)]
#[seeds(b"deposit", event: Address, attendee: Address)]
pub struct AttendeeDeposit {
    /// Schema version for future migrations.
    pub version: u8,
    /// Attendee's wallet address.
    pub attendee: Address,
    /// Reference to EventEscrow.
    pub event: Address,
    /// Amount deposited (smallest unit).
    pub amount: u64,
    /// Deposit timestamp.
    pub deposited_at: i64,
    /// Whether attendee checked in (set by organizer authority).
    pub checked_in: bool,
    /// Whether deposit has been settled (refunded or forfeited).
    pub refunded: bool,
    /// Bump seed for PDA.
    pub bump: u8,
    /// Reserved padding for future fields (11 bytes).
    pub _padding: [u8; 11],
}

impl EventEscrow {
    /// Validate the schema version of this account.
    /// Returns an error if the version is not supported.
    /// Currently only v1 is supported.
    #[inline(always)]
    pub fn validate_version(&self) -> Result<(), ProgramError> {
        if self.version() != ESCROW_VERSION {
            return Err(EscrowError::EscrowVersionMismatch.into());
        }
        Ok(())
    }
}

impl AttendeeDeposit {
    /// Validate the schema version of this account.
    /// Returns an error if the version is not supported.
    /// Currently only v1 is supported.
    #[inline(always)]
    pub fn validate_version(&self) -> Result<(), ProgramError> {
        if self.version() != DEPOSIT_VERSION {
            return Err(EscrowError::DepositVersionMismatch.into());
        }
        Ok(())
    }
}
