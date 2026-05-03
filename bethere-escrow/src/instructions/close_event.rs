use {
    crate::{errors::EscrowError, events::EventClosed, state::EventEscrow},
    quasar_lang::prelude::*,
    quasar_spl::prelude::*,
};

#[derive(Accounts)]
#[instruction(event_id: u64)]
pub struct CloseEvent {
    #[account(mut)]
    pub organizer: Signer,
    #[account(
        mut,
        has_one(organizer) @ EscrowError::Unauthorized,
        constraints(!event_escrow.is_active()) @ EscrowError::EventStillActive,
        close(dest = organizer),
        address = EventEscrow::seeds(organizer.address(), event_id)
    )]
    pub event_escrow: Account<EventEscrow>,
    #[account(mut)]
    pub vault: Account<Token>,
    pub token_program: Program<TokenProgram>,
}

impl CloseEvent {
    #[inline(always)]
    pub fn close_event(&self, bumps: &CloseEventBumps) -> Result<(), ProgramError> {
        let bump = [bumps.event_escrow];
        let event_id_bytes = self.event_escrow.event_id().to_le_bytes();
        let seeds = [
            Seed::from(b"escrow" as &[u8]),
            Seed::from(self.event_escrow.organizer().as_ref()),
            Seed::from(event_id_bytes.as_ref()),
            Seed::from(bump.as_ref()),
        ];

        // Close the vault token account — rent goes to organizer
        self.token_program
            .close_account(&self.vault, &self.organizer, &self.event_escrow)
            .invoke_signed(&seeds)?;

        emit!(EventClosed {
            escrow: *self.event_escrow.address(),
        });

        Ok(())
    }
}
