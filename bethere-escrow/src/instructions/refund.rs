use {
    crate::{
        errors::EscrowError,
        events::Refunded,
        state::{AttendeeDeposit, EventEscrow},
    },
    quasar_lang::prelude::*,
    quasar_spl::prelude::*,
};

/// Instructions sysvar address: `Sysvar1nstructions1111111111111111111111111`
const INSTRUCTIONS_SYSVAR_ID: [u8; 32] = [
    6, 167, 213, 23, 24, 123, 209, 102, 53, 218, 212, 4, 85, 253, 194, 192, 193, 36, 198, 143, 33,
    86, 117, 165, 219, 186, 203, 95, 8, 0, 0, 0,
];

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
    /// CHECK: Instructions sysvar — validated in handler.
    pub instruction_sysvar: UncheckedAccount,
    pub rent: Sysvar<Rent>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}

impl Refund {
    #[inline(always)]
    pub fn validate_and_update(&mut self) -> Result<(), ProgramError> {
        // Note: require_distinct and validate_version removed to free BPF call depth
        // frames for instruction introspection. PDA seeds guarantee distinct addresses
        // and correct account discriminators.

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

        // SEC-010: Instruction introspection — refund must be paired with
        // close_deposit in the same transaction to prevent rent leaks.
        self.require_close_deposit_pair()?;

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

    /// SEC-010: Require that a `close_deposit` instruction follows this
    /// `refund` in the same transaction. Inlined zero-allocation scanning
    /// to minimize BPF call depth.
    #[inline(always)]
    fn require_close_deposit_pair(&self) -> Result<(), ProgramError> {
        let view = self.instruction_sysvar.to_account_view();

        // Validate sysvar address
        if view.address().as_ref() != INSTRUCTIONS_SYSVAR_ID {
            return Err(ProgramError::UnsupportedSysvar);
        }

        let data = view.try_borrow()?;
        if data.len() < 4 {
            return Err(EscrowError::RefundRequiresClose.into());
        }

        // Instructions sysvar layout:
        //   [0..2]              num_instructions (u16 LE)
        //   [2..2+2*N]          instruction_offsets (N × u16 LE)
        //   Per instruction at each offset:
        //     [0..2]            num_accounts (u16 LE)
        //     [2..2+33*A]       accounts (1 byte meta + 32 bytes pubkey each)
        //     [2+33*A..34+33*A] program_id (32 bytes)
        //     [34+33*A..36+33*A] data_len (u16 LE)
        //     [36+33*A..]       data bytes
        //   [last 2 bytes]      current_index (u16 LE)

        let num_instructions = u16::from_le_bytes([data[0], data[1]]) as usize;
        let len = data.len();
        let current_idx = u16::from_le_bytes([data[len - 2], data[len - 1]]) as usize;

        // Scan sibling instructions after this one for close_deposit (discriminator 7)
        // from the same program targeting the same attendee deposit.
        let program_id = crate::ID.to_bytes();
        let deposit_bytes = self.attendee_deposit.address().to_bytes();

        for idx in (current_idx + 1)..num_instructions {
            // Read offset from the offset table
            let offset_start = 2 + idx * 2;
            if offset_start + 2 > len {
                break;
            }
            let offset = u16::from_le_bytes([data[offset_start], data[offset_start + 1]]) as usize;
            if offset >= len {
                continue;
            }

            // At the instruction: num_accounts (u16)
            if offset + 2 > len {
                continue;
            }
            let num_accounts = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;

            // Skip accounts: 33 bytes each (1 meta + 32 pubkey)
            let accounts_end = offset + 2 + num_accounts * 33;
            if accounts_end + 32 > len {
                continue;
            }

            // Check program_id
            let prog_start = accounts_end;
            if data[prog_start..prog_start + 32] != program_id {
                continue;
            }

            // data_len (u16) after program_id
            let data_len_start = prog_start + 32;
            if data_len_start + 2 > len {
                continue;
            }
            let _data_len =
                u16::from_le_bytes([data[data_len_start], data[data_len_start + 1]]) as usize;

            // Check discriminator (first byte of instruction data)
            let disc_pos = data_len_start + 2;
            if disc_pos >= len {
                continue;
            }
            if data[disc_pos] != 7 {
                continue;
            }

            // Found close_deposit from our program. Verify it targets the same
            // attendee deposit account.
            let accounts_start = offset + 2;
            let mut found_deposit = false;
            for i in 0..num_accounts {
                let key_offset = accounts_start + i * 33 + 1; // +1 to skip meta byte
                if data[key_offset..key_offset + 32] == deposit_bytes {
                    found_deposit = true;
                    break;
                }
            }
            if found_deposit {
                return Ok(());
            }
        }

        Err(EscrowError::RefundRequiresClose.into())
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
