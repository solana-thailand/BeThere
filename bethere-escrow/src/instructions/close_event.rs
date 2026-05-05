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
    #[account(
        mut,
        constraints(*vault.address() == *event_escrow.vault()) @ EscrowError::VaultMismatch
    )]
    pub vault: Account<Token>,
    pub token_program: Program<TokenProgram>,
}

impl CloseEvent {
    #[inline(always)]
    pub fn close_event(&self, bumps: &CloseEventBumps) -> Result<(), ProgramError> {
        // Safety: vault must be empty before closing.
        // SPL token close_account zeros the data — remaining tokens would be
        // permanently lost.
        //
        // We verify via accounting: total_deposited == total_refunded + total_forfeited.
        // This is equivalent to checking vault balance == 0 without reading token data.
        let total_deposited = self.event_escrow.total_deposited();
        let total_refunded = self.event_escrow.total_refunded();
        let total_forfeited = self.event_escrow.total_forfeited();
        let settled = total_refunded
            .checked_add(total_forfeited)
            .ok_or(EscrowError::Overflow)?;
        if total_deposited != settled {
            return Err(EscrowError::VaultNotEmpty.into());
        }

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
