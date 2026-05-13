use {
    crate::{errors::EscrowError, events::ForfeitedClaimed, state::EventEscrow},
    quasar_lang::prelude::*,
    quasar_spl::prelude::*,
};

#[derive(Accounts)]
#[instruction(event_id: u64)]
pub struct ClaimForfeited {
    #[account(mut)]
    pub organizer: Signer,
    #[account(
        mut,
        has_one(organizer) @ EscrowError::Unauthorized,
        address = EventEscrow::seeds(organizer.address(), event_id)
    )]
    pub event_escrow: Account<EventEscrow>,
    #[account(
        init(idempotent),
        payer = organizer,
        token(mint = usdc_mint, authority = organizer, token_program = token_program)
    )]
    pub organizer_ta: Account<Token>,
    #[account(
        constraints(*usdc_mint.address() == *event_escrow.usdc_mint()) @ EscrowError::MintMismatch
    )]
    pub usdc_mint: Account<Mint>,
    #[account(
        mut,
        constraints(*vault.address() == *event_escrow.vault()) @ EscrowError::VaultMismatch
    )]
    pub vault: Account<Token>,
    pub rent: Sysvar<Rent>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}

impl ClaimForfeited {
    #[inline(always)]
    pub fn validate_and_claim(&mut self, bumps: &ClaimForfeitedBumps) -> Result<(), ProgramError> {
        self.event_escrow.validate_version()?;
        let clock = <Clock as quasar_lang::sysvars::Sysvar>::get()?;

        // Verify refund deadline has passed
        if clock.unix_timestamp.get() < self.event_escrow.refund_deadline() {
            return Err(EscrowError::RefundDeadlineNotPassed.into());
        }

        // Calculate forfeited amount: total_deposited - total_refunded - total_forfeited
        let total_deposited = self.event_escrow.total_deposited();
        let total_refunded = self.event_escrow.total_refunded();
        let total_forfeited = self.event_escrow.total_forfeited();
        let forfeited = total_deposited
            .checked_sub(total_refunded)
            .and_then(|v| v.checked_sub(total_forfeited))
            .ok_or_else(|| EscrowError::NoForfeitedFunds)?;

        if forfeited == 0 {
            return Err(EscrowError::NoForfeitedFunds.into());
        }

        // Transfer forfeited USDC from vault to organizer
        let bump = [bumps.event_escrow];
        let event_id_bytes = self.event_escrow.event_id().to_le_bytes();
        let seeds = [
            Seed::from(b"escrow" as &[u8]),
            Seed::from(self.event_escrow.organizer().as_ref()),
            Seed::from(event_id_bytes.as_ref()),
            Seed::from(bump.as_ref()),
        ];

        self.token_program
            .transfer_checked(
                &self.vault,
                &self.usdc_mint,
                &self.organizer_ta,
                &self.event_escrow,
                forfeited,
                6,
            )
            .invoke_signed(&seeds)?;

        // Update escrow totals (checked arithmetic)
        self.event_escrow.total_forfeited = total_forfeited
            .checked_add(forfeited)
            .ok_or(EscrowError::Overflow)?
            .into();

        // Emit event
        emit!(ForfeitedClaimed {
            escrow: *self.event_escrow.address(),
            organizer: *self.organizer.address(),
            amount: forfeited,
        });

        Ok(())
    }
}
