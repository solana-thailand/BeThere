use {
    crate::{
        errors::EscrowError,
        events::Refunded,
        state::{AttendeeDeposit, EventEscrow},
    },
    quasar_lang::prelude::*,
    quasar_spl::prelude::*,
};

#[derive(Accounts)]
#[instruction(event_id: u64)]
pub struct Refund {
    #[account(mut)]
    pub attendee: Signer,
    #[account(
        mut,
        address = EventEscrow::seeds(event_escrow.organizer(), event_id)
    )]
    pub event_escrow: Account<EventEscrow>,
    #[account(
        constraints(*usdc_mint.address() == *event_escrow.usdc_mint()) @ EscrowError::MintMismatch
    )]
    pub usdc_mint: Account<Mint>,
    #[account(
        mut,
        constraints(*attendee_deposit.attendee() == *attendee.address()) @ EscrowError::Unauthorized,
        constraints(!attendee_deposit.refunded()) @ EscrowError::AlreadyRefunded,
        address = AttendeeDeposit::seeds(event_escrow.address(), attendee.address())
    )]
    pub attendee_deposit: Account<AttendeeDeposit>,
    #[account(
        init(idempotent),
        payer = attendee,
        token(mint = usdc_mint, authority = attendee, token_program = token_program)
    )]
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

impl Refund {
    #[inline(always)]
    pub fn validate_and_update(&mut self) -> Result<(), ProgramError> {
        let clock = <Clock as quasar_lang::sysvars::Sysvar>::get()?;

        // Verify event has ended
        if clock.unix_timestamp.get() < self.event_escrow.event_end() {
            return Err(EscrowError::RefundNotYetAllowed.into());
        }

        // Verify refund deadline has not passed
        // After refund_deadline, only claim_forfeited is available.
        // This prevents a race where organizer claims forfeited (draining vault)
        // and then attendee refunds fail because vault is empty.
        if clock.unix_timestamp.get() >= self.event_escrow.refund_deadline() {
            return Err(EscrowError::RefundDeadlinePassed.into());
        }

        let amount = self.attendee_deposit.amount();

        // Mark as refunded
        self.attendee_deposit.refunded = true.into();

        // Update escrow totals (checked arithmetic)
        let total_refunded = self.event_escrow.total_refunded();
        self.event_escrow.total_refunded = total_refunded
            .checked_add(amount)
            .ok_or(EscrowError::Overflow)?
            .into();

        Ok(())
    }

    #[inline(always)]
    pub fn transfer_usdc(&self, bumps: &RefundBumps) -> Result<(), ProgramError> {
        let amount = self.attendee_deposit.amount();
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
                &self.attendee_ta,
                &self.event_escrow,
                amount,
                6,
            )
            .invoke_signed(&seeds)
    }

    #[inline(always)]
    pub fn emit_event(&self) -> Result<(), ProgramError> {
        emit!(Refunded {
            escrow: *self.event_escrow.address(),
            attendee: *self.attendee.address(),
            amount: self.attendee_deposit.amount(),
        });
        Ok(())
    }
}
