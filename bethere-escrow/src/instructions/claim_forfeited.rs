use {
    crate::{
        errors::EscrowError,
        events::ForfeitedClaimed,
        state::{AttendeeDeposit, EventEscrow},
    },
    quasar_lang::prelude::*,
    quasar_spl::prelude::*,
};

/// Claim a single no-show's forfeited deposit after refund deadline.
///
/// The organizer must pass the specific `AttendeeDeposit` account for each no-show.
/// This prevents the organizer from claiming deposits of attendees who checked in
/// but haven't refunded yet.
///
/// **Guards**:
/// - Signer is the organizer (has_one check)
/// - Refund deadline has passed
/// - Attendee was NOT checked in (checked_in == false)
/// - Deposit has not already been refunded/forfeited (refunded == false)
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
    /// The specific no-show attendee's deposit to claim.
    #[account(
        mut,
        constraints(*attendee_deposit.event() == *event_escrow.address()) @ EscrowError::Unauthorized,
        constraints(!attendee_deposit.checked_in()) @ EscrowError::AttendeeCheckedIn,
        constraints(!attendee_deposit.refunded()) @ EscrowError::AlreadyRefunded,
        address = AttendeeDeposit::seeds(event_escrow.address(), attendee_deposit.attendee())
    )]
    pub attendee_deposit: Account<AttendeeDeposit>,
    #[account(
        init(idempotent),
        payer = organizer,
        token(mint = deposit_mint, authority = organizer, token_program = token_program)
    )]
    pub organizer_ta: Account<Token>,
    #[account(
        constraints(*deposit_mint.address() == *event_escrow.deposit_mint()) @ EscrowError::MintMismatch
    )]
    pub deposit_mint: Account<Mint>,
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
        self.attendee_deposit.validate_version()?;
        let clock = <Clock as quasar_lang::sysvars::Sysvar>::get()?;

        // Verify refund deadline has passed.
        // Before this, attendees can still refund (even no-shows).
        if clock.unix_timestamp.get() < self.event_escrow.refund_deadline() {
            return Err(EscrowError::RefundDeadlineNotPassed.into());
        }

        let amount = self.attendee_deposit.amount();

        if amount == 0 {
            return Err(EscrowError::NoForfeitedFunds.into());
        }

        // Mark deposit as settled (forfeited) — prevents double-claim.
        self.attendee_deposit.refunded = true.into();

        // Transfer forfeited tokens from vault to organizer.
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
                &self.organizer_ta,
                &self.event_escrow,
                amount,
                mint_decimals,
            )
            .invoke_signed(&seeds)?;

        // Update escrow totals (checked arithmetic).
        // Invariant: total_deposited == total_refunded + total_forfeited.
        let total_forfeited = self.event_escrow.total_forfeited();
        self.event_escrow.total_forfeited = total_forfeited
            .checked_add(amount)
            .ok_or(EscrowError::Overflow)?
            .into();

        emit!(ForfeitedClaimed {
            escrow: *self.event_escrow.address(),
            organizer: *self.organizer.address(),
            amount,
        });

        Ok(())
    }
}
