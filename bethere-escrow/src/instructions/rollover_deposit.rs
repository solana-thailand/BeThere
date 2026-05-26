use {
    crate::{
        errors::EscrowError,
        events::DepositRolledOver,
        state::{AttendeeDeposit, AttendeeDepositInner, EventEscrow, DEPOSIT_VERSION},
    },
    quasar_lang::prelude::*,
    quasar_spl::prelude::*,
};

/// Roll a checked-in attendee's deposit from a past event to a new event.
///
/// Atomically transfers USDC from the source event vault to the target event
/// vault, settles the source deposit, and creates a new deposit for the target
/// event.
///
/// **Guards**:
/// - Signer is the attendee (their deposit, their choice to roll over)
/// - Source and target events have the same organizer
/// - Source and target events use the same deposit mint
/// - Source and target events have the same deposit amount (v1)
/// - Attendee was checked in on the source event
/// - Source deposit has not been refunded/forfeited already
/// - Target event is active (accepting deposits)
/// - Target attendee deposit does not already exist
#[derive(Accounts)]
#[instruction(source_event_id: u64, target_event_id: u64)]
pub struct RolloverDeposit {
    // ── Attendee (signer) ──
    #[account(mut)]
    pub attendee: Signer,

    // ── Source event (past) ──
    #[account(
        mut,
        address = EventEscrow::seeds(source_escrow.organizer(), source_event_id)
    )]
    pub source_escrow: Account<EventEscrow>,
    #[account(
        mut,
        constraints(*source_deposit.attendee() == *attendee.address()) @ EscrowError::Unauthorized,
        constraints(*source_deposit.event() == *source_escrow.address()) @ EscrowError::Unauthorized,
        constraints(source_deposit.checked_in()) @ EscrowError::NotCheckedIn,
        constraints(!source_deposit.refunded()) @ EscrowError::AlreadyRefunded,
        address = AttendeeDeposit::seeds(source_escrow.address(), attendee.address())
    )]
    pub source_deposit: Account<AttendeeDeposit>,
    #[account(
        mut,
        constraints(*source_vault.address() == *source_escrow.vault()) @ EscrowError::VaultMismatch
    )]
    pub source_vault: Account<Token>,

    // ── Target event (new) ──
    #[account(
        mut,
        constraints(target_escrow.is_active()) @ EscrowError::EventNotActive,
        constraints(*target_escrow.organizer() == *source_escrow.organizer()) @ EscrowError::Unauthorized,
        constraints(*target_escrow.deposit_mint() == *source_escrow.deposit_mint()) @ EscrowError::MintMismatch,
        constraints(target_escrow.deposit_amount() == source_escrow.deposit_amount()) @ EscrowError::IncorrectDepositAmount,
        address = EventEscrow::seeds(target_escrow.organizer(), target_event_id)
    )]
    pub target_escrow: Account<EventEscrow>,
    #[account(
        init,
        payer = attendee,
        address = AttendeeDeposit::seeds(target_escrow.address(), attendee.address())
    )]
    pub target_deposit: Account<AttendeeDeposit>,
    #[account(
        mut,
        constraints(*target_vault.address() == *target_escrow.vault()) @ EscrowError::VaultMismatch
    )]
    pub target_vault: Account<Token>,

    // ── Shared ──
    #[account(
        constraints(*deposit_mint.address() == *source_escrow.deposit_mint()) @ EscrowError::MintMismatch
    )]
    pub deposit_mint: Account<Mint>,
    pub rent: Sysvar<Rent>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}

impl RolloverDeposit {
    /// Validate all accounts and update state for both source and target.
    #[inline(always)]
    pub fn validate_and_update(
        &mut self,
        bumps: &RolloverDepositBumps,
    ) -> Result<(), ProgramError> {
        // SEC-DUP: Defense-in-depth — no duplicate mutable accounts.
        super::require_distinct(&[
            *self.attendee.address(),
            *self.source_escrow.address(),
            *self.source_deposit.address(),
            *self.source_vault.address(),
            *self.target_escrow.address(),
            *self.target_deposit.address(),
            *self.target_vault.address(),
        ])?;

        self.source_escrow.validate_version()?;
        self.source_deposit.validate_version()?;
        self.target_escrow.validate_version()?;

        let amount = self.source_deposit.amount();

        // ── Settle source deposit ──
        self.source_deposit.refunded = true.into();

        // Source escrow: move from total_deposited bucket to total_refunded.
        let source_refunded = self.source_escrow.total_refunded();
        self.source_escrow.total_refunded = source_refunded
            .checked_add(amount)
            .ok_or(EscrowError::Overflow)?
            .into();

        // ── Create target deposit ──
        let clock = <Clock as quasar_lang::sysvars::Sysvar>::get()?;
        self.target_deposit.set_inner(AttendeeDepositInner {
            version: DEPOSIT_VERSION,
            attendee: *self.attendee.address(),
            event: *self.target_escrow.address(),
            amount,
            deposited_at: clock.unix_timestamp.get(),
            checked_in: false,
            refunded: false,
            bump: bumps.target_deposit,
            _padding: [0; 11],
        });

        // Target escrow: track new deposit.
        let target_deposited = self.target_escrow.total_deposited();
        self.target_escrow.total_deposited = target_deposited
            .checked_add(amount)
            .ok_or(EscrowError::Overflow)?
            .into();

        // ── Transfer USDC from source vault to target vault ──
        // Source escrow PDA signs as vault authority.
        let source_bump = [bumps.source_escrow];
        let source_event_id_bytes = self.source_escrow.event_id().to_le_bytes();
        let source_seeds = [
            Seed::from(b"escrow" as &[u8]),
            Seed::from(self.source_escrow.organizer().as_ref()),
            Seed::from(source_event_id_bytes.as_ref()),
            Seed::from(source_bump.as_ref()),
        ];

        let mint_decimals = self.deposit_mint.decimals();
        self.token_program
            .transfer_checked(
                &self.source_vault,
                &self.deposit_mint,
                &self.target_vault,
                &self.source_escrow,
                amount,
                mint_decimals,
            )
            .invoke_signed(&source_seeds)?;

        emit!(DepositRolledOver {
            source_escrow: *self.source_escrow.address(),
            target_escrow: *self.target_escrow.address(),
            attendee: *self.attendee.address(),
            amount,
        });

        Ok(())
    }
}
