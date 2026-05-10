use {
    crate::{
        events::CheckedIn,
        state::{AttendeeDeposit, EventEscrow},
    },
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
#[instruction(event_id: u64)]
pub struct MarkCheckedIn {
    #[account(mut)]
    pub organizer: Signer,
    #[account(
        has_one(organizer),
        address = EventEscrow::seeds(event_escrow.organizer(), event_id)
    )]
    pub event_escrow: Account<EventEscrow>,
    #[account(
        mut,
        constraints(!attendee_deposit.checked_in()),
        constraints(*attendee_deposit.event() == *event_escrow.address()),
        address = AttendeeDeposit::seeds(event_escrow.address(), attendee_deposit.attendee())
    )]
    pub attendee_deposit: Account<AttendeeDeposit>,
}

impl MarkCheckedIn {
    #[inline(always)]
    pub fn mark_checked_in(&mut self) -> Result<(), ProgramError> {
        // SEC-011: Prevent check-ins after event ends
        let clock = <Clock as quasar_lang::sysvars::Sysvar>::get()?;
        if clock.unix_timestamp.get() > self.event_escrow.event_end() {
            return Err(crate::errors::EscrowError::EventEnded.into());
        }

        self.attendee_deposit.checked_in = true.into();
        Ok(())
    }

    #[inline(always)]
    pub fn emit_event(&self) -> Result<(), ProgramError> {
        emit!(CheckedIn {
            escrow: *self.event_escrow.address(),
            attendee: *self.attendee_deposit.attendee(),
        });
        Ok(())
    }
}
