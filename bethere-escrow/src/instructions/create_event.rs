use {
    crate::{
        errors::EscrowError,
        events::EventCreated,
        state::{EventEscrow, EventEscrowInner, ESCROW_VERSION},
    },
    quasar_lang::prelude::*,
    quasar_spl::prelude::*,
};

#[derive(Accounts)]
#[instruction(event_id: u64)]
pub struct CreateEvent {
    #[account(mut)]
    pub organizer: Signer,
    #[account(
        init,
        payer = organizer,
        address = EventEscrow::seeds(organizer.address(), event_id)
    )]
    pub event_escrow: Account<EventEscrow>,
    pub deposit_mint: Account<Mint>,
    #[account(
        init(idempotent),
        payer = organizer,
        token(mint = deposit_mint, authority = event_escrow, token_program = token_program)
    )]
    pub vault: Account<Token>,
    pub rent: Sysvar<Rent>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}

impl CreateEvent {
    #[inline(always)]
    pub fn create_escrow(
        &mut self,
        event_id: u64,
        deposit_amount: u64,
        event_end: i64,
        refund_deadline: i64,
        bumps: &CreateEventBumps,
    ) -> Result<(), ProgramError> {
        // SEC-DUP: Defense-in-depth — no duplicate mutable accounts.
        super::require_distinct(&[
            *self.organizer.address(),
            *self.event_escrow.address(),
            *self.vault.address(),
        ])?;

        if deposit_amount == 0 {
            return Err(EscrowError::InvalidDepositAmount.into());
        }

        let clock = <Clock as quasar_lang::sysvars::Sysvar>::get()?;
        if event_end <= clock.unix_timestamp.get() {
            return Err(EscrowError::EventEndInPast.into());
        }

        if refund_deadline <= event_end {
            return Err(EscrowError::RefundDeadlineNotPassed.into());
        }

        self.event_escrow.set_inner(EventEscrowInner {
            version: ESCROW_VERSION,
            organizer: *self.organizer.address(),
            event_id,
            deposit_mint: *self.deposit_mint.address(),
            vault: *self.vault.address(),
            deposit_amount,
            event_end,
            refund_deadline,
            total_deposited: 0,
            total_refunded: 0,
            total_forfeited: 0,
            is_active: true,
            bump: bumps.event_escrow,
            _padding: [0; 36],
        });
        Ok(())
    }

    #[inline(always)]
    pub fn emit_event(&self) -> Result<(), ProgramError> {
        emit!(EventCreated {
            escrow: *self.event_escrow.address(),
            organizer: *self.organizer.address(),
            deposit_amount: self.event_escrow.deposit_amount(),
            event_end: self.event_escrow.event_end(),
            refund_deadline: self.event_escrow.refund_deadline(),
        });
        Ok(())
    }
}
