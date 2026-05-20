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
        // SEC-DUP: Defense-in-depth — no duplicate mutable accounts.
        super::require_distinct(&[
            *self.organizer.address(),
            *self.event_escrow.address(),
            *self.vault.address(),
        ])?;

        self.event_escrow.validate_version()?;

        // Safety: vault must be empty before closing.
        // SPL token close_account zeros the data — remaining tokens would be
        // permanently lost.
        //
        // Double check: accounting invariant AND actual vault balance.
        // The accounting check catches normal flow issues.
        // The vault balance check catches external airdrop griefing
        // (anyone can transfer tokens to the vault without going through the program).
        let total_deposited = self.event_escrow.total_deposited();
        let total_refunded = self.event_escrow.total_refunded();
        let total_forfeited = self.event_escrow.total_forfeited();
        let settled = total_refunded
            .checked_add(total_forfeited)
            .ok_or(EscrowError::Overflow)?;
        if total_deposited != settled {
            return Err(EscrowError::VaultNotEmpty.into());
        }

        // Verify actual vault token balance is zero.
        // Prevents griefing where someone airdrops 1 lamport USDC to vault,
        // which would cause SPL close_account to fail (requires amount == 0).
        if self.vault.amount() != 0 {
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
