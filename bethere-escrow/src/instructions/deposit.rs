use {
    crate::{
        errors::EscrowError,
        events::Deposited,
        state::{AttendeeDeposit, AttendeeDepositInner, EventEscrow},
    },
    quasar_lang::prelude::*,
    quasar_spl::prelude::*,
};

#[derive(Accounts)]
#[instruction(event_id: u64)]
pub struct Deposit {
    #[account(mut)]
    pub attendee: Signer,
    #[account(
        mut,
        constraints(event_escrow.is_active()) @ EscrowError::EventNotActive,
        address = EventEscrow::seeds(event_escrow.organizer(), event_id)
    )]
    pub event_escrow: Account<EventEscrow>,
    #[account(
        constraints(*usdc_mint.address() == *event_escrow.usdc_mint()) @ EscrowError::MintMismatch
    )]
    pub usdc_mint: Account<Mint>,
    #[account(
        init,
        payer = attendee,
        address = AttendeeDeposit::seeds(event_escrow.address(), attendee.address())
    )]
    pub attendee_deposit: Account<AttendeeDeposit>,
    #[account(mut)]
    pub attendee_ta: Account<Token>,
    #[account(
        mut,
        constraints(*vault.address() == *event_escrow.vault()) @ EscrowError::VaultMismatch
    )]
    pub vault: Account<Token>,
    pub rent: Sysvar<Rent>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}

impl Deposit {
    #[inline(always)]
    pub fn create_deposit(&mut self, bumps: &DepositBumps) -> Result<(), ProgramError> {
        let clock = <Clock as quasar_lang::sysvars::Sysvar>::get()?;
        let amount = self.event_escrow.deposit_amount();

        self.attendee_deposit.set_inner(AttendeeDepositInner {
            attendee: *self.attendee.address(),
            event: *self.event_escrow.address(),
            amount,
            deposited_at: clock.unix_timestamp.get(),
            checked_in: false,
            refunded: false,
            bump: bumps.attendee_deposit,
        });

        // Update escrow totals (checked arithmetic)
        let total_deposited = self.event_escrow.total_deposited();
        self.event_escrow.total_deposited = total_deposited
            .checked_add(amount)
            .ok_or(EscrowError::Overflow)?
            .into();

        Ok(())
    }

    #[inline(always)]
    pub fn transfer_usdc(&self) -> Result<(), ProgramError> {
        let amount = self.event_escrow.deposit_amount();
        self.token_program
            .transfer(&self.attendee_ta, &self.vault, &self.attendee, amount)
            .invoke()
    }

    #[inline(always)]
    pub fn emit_event(&self) -> Result<(), ProgramError> {
        emit!(Deposited {
            escrow: *self.event_escrow.address(),
            attendee: *self.attendee.address(),
            amount: self.event_escrow.deposit_amount(),
        });
        Ok(())
    }
}
