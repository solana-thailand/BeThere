use {
    crate::{errors::EscrowError, events::EventDeactivated, state::EventEscrow},
    quasar_lang::prelude::*,
};

/// Deactivate an event escrow — sets `is_active = false`.
///
/// After deactivation:
/// - No more deposits accepted
/// - Refunds still allowed (until refund_deadline)
/// - `close_event` can be called to reclaim rent
///
/// **Access control**: Only the organizer can deactivate.
/// **Timing**: Can deactivate anytime (organizer may want to close registration early).
#[derive(Accounts)]
#[instruction(event_id: u64)]
pub struct DeactivateEvent {
    #[account(mut)]
    pub organizer: Signer,
    #[account(
        mut,
        has_one(organizer) @ EscrowError::Unauthorized,
        constraints(event_escrow.is_active()) @ EscrowError::EventNotActive,
        address = EventEscrow::seeds(organizer.address(), event_id)
    )]
    pub event_escrow: Account<EventEscrow>,
}

impl DeactivateEvent {
    #[inline(always)]
    pub fn deactivate(&mut self) -> Result<(), ProgramError> {
        self.event_escrow.validate_version()?;
        self.event_escrow.is_active = false.into();

        emit!(EventDeactivated {
            escrow: *self.event_escrow.address(),
            organizer: *self.organizer.address(),
        });

        Ok(())
    }
}
