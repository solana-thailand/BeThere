use {
    crate::{
        errors::EscrowError,
        events::Refunded,
        state::{AttendeeDeposit, EventEscrow},
    },
    quasar_lang::prelude::*,
    quasar_spl::prelude::*,
};

/// Refund deposit to attendee.
///
/// Two paths:
/// 1. **Checked-in attendee**: Can refund anytime after `event_end`. No deadline.
///    They earned their deposit back by showing up — they should always be able to claim it.
/// 2. **No-show attendee (not checked in)**: Can only refund after `event_end`
///    and before `refund_deadline`. After the deadline, the organizer can claim
///    their deposit as forfeited.
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
        constraints(*deposit_mint.address() == *event_escrow.deposit_mint()) @ EscrowError::MintMismatch
    )]
    pub deposit_mint: Account<Mint>,
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
        token(mint = deposit_mint, authority = attendee, token_program = token_program)
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
        self.event_escrow.validate_version()?;
        self.attendee_deposit.validate_version()?;

        let clock = <Clock as quasar_lang::sysvars::Sysvar>::get()?;

        // Verify event has ended.
        if clock.unix_timestamp.get() < self.event_escrow.event_end() {
            return Err(EscrowError::RefundNotYetAllowed.into());
        }

        // If attendee was NOT checked in, they must refund before the deadline.
        // After refund_deadline, the organizer can claim no-show deposits.
        // Checked-in attendees can refund anytime — they showed up.
        if !self.attendee_deposit.checked_in()
            && clock.unix_timestamp.get() >= self.event_escrow.refund_deadline()
        {
            return Err(EscrowError::RefundDeadlinePassed.into());
        }

        let amount = self.attendee_deposit.amount();

        // Mark as refunded.
        self.attendee_deposit.refunded = true.into();

        // Update escrow totals (checked arithmetic).
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

        let mint_decimals = self.deposit_mint.decimals();
        self.token_program
            .transfer_checked(
                &self.vault,
                &self.deposit_mint,
                &self.attendee_ta,
                &self.event_escrow,
                amount,
                mint_decimals,
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
