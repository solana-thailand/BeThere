use {
    crate::{errors::EscrowError, events::DepositClosed, state::AttendeeDeposit},
    quasar_lang::prelude::*,
};

/// Close an attendee's deposit PDA to reclaim rent.
///
/// Two paths:
/// 1. **Self-close**: Attendee closes their own deposit after `refunded == true`.
/// 2. **GC close**: Anyone can close after the parent `EventEscrow` has been
///    closed (detected by `event_escrow.data_len() == 0`).
#[derive(Accounts)]
#[instruction(_event_id: u64)]
pub struct CloseDeposit {
    #[account(mut)]
    pub signer: Signer,
    /// CHECK: May be closed (zero data). Validated in handler.
    pub event_escrow: UncheckedAccount,
    #[account(
        mut,
        close(dest = signer),
        address = AttendeeDeposit::seeds(event_escrow.address(), attendee_deposit.attendee())
    )]
    pub attendee_deposit: Account<AttendeeDeposit>,
    pub system_program: Program<SystemProgram>,
}

impl CloseDeposit {
    #[inline(always)]
    pub fn close_deposit(&self) -> Result<(), ProgramError> {
        self.attendee_deposit.validate_version()?;
        let event_escrow_closed = self.event_escrow.to_account_view().data_len() == 0;

        if !event_escrow_closed {
            // Event escrow still exists — only the attendee can close,
            // and only after refund.
            if *self.attendee_deposit.attendee() != *self.signer.address() {
                return Err(EscrowError::Unauthorized.into());
            }
            if !self.attendee_deposit.refunded() {
                return Err(EscrowError::DepositNotRefunded.into());
            }
        }
        // GC path: if event_escrow is closed, anyone can close the deposit PDA.

        emit!(DepositClosed {
            escrow: *self.attendee_deposit.event(),
            attendee: *self.attendee_deposit.attendee(),
        });

        Ok(())
    }
}
