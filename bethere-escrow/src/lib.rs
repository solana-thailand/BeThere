#![no_std]
#![allow(dead_code)]

use quasar_lang::prelude::*;

mod errors;
mod events;
mod instructions;
mod state;
use instructions::*;

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani;

declare_id!("C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T");

#[program]
mod bethere_escrow {
    use super::*;

    /// Initialize an event escrow with deposit vault.
    /// Called by organizer before registration opens.
    #[instruction(discriminator = 0)]
    pub fn create_event(
        ctx: Ctx<CreateEvent>,
        event_id: u64,
        deposit_amount: u64,
        event_end: i64,
        refund_deadline: i64,
    ) -> Result<(), ProgramError> {
        ctx.accounts.create_escrow(
            event_id,
            deposit_amount,
            event_end,
            refund_deadline,
            &ctx.bumps,
        )?;
        ctx.accounts.emit_event()
    }

    /// Deposit tokens into the event escrow vault.
    /// Called by attendee at registration time.
    #[instruction(discriminator = 1)]
    pub fn deposit(ctx: Ctx<Deposit>, _event_id: u64) -> Result<(), ProgramError> {
        ctx.accounts.create_deposit(&ctx.bumps)?;
        ctx.accounts.transfer_usdc()?;
        ctx.accounts.emit_event()
    }

    /// Mark attendee as checked in.
    /// Called by organizer authority at event (scanner).
    #[instruction(discriminator = 2)]
    pub fn mark_checked_in(ctx: Ctx<MarkCheckedIn>, _event_id: u64) -> Result<(), ProgramError> {
        ctx.accounts.mark_checked_in()?;
        ctx.accounts.emit_event()
    }

    /// Refund tokens to attendee after event ends.
    /// Called by attendee (signed by their wallet).
    /// Checked-in attendees: can refund anytime after event_end (no deadline).
    /// No-show attendees: must refund before refund_deadline.
    #[instruction(discriminator = 3)]
    pub fn refund(ctx: Ctx<Refund>, _event_id: u64) -> Result<(), ProgramError> {
        ctx.accounts.validate_and_update()?;
        ctx.accounts.transfer_usdc(&ctx.bumps)?;
        ctx.accounts.emit_event()
    }

    /// Claim a single no-show's forfeited deposit after refund deadline.
    /// Must pass the specific AttendeeDeposit account.
    /// Checked-in attendees are protected — their deposits cannot be forfeited.
    #[instruction(discriminator = 4)]
    pub fn claim_forfeited(ctx: Ctx<ClaimForfeited>, _event_id: u64) -> Result<(), ProgramError> {
        ctx.accounts.validate_and_claim(&ctx.bumps)
    }

    /// Close the event escrow and reclaim rent.
    /// Called by organizer after all refunds/claims are done.
    #[instruction(discriminator = 5)]
    pub fn close_event(ctx: Ctx<CloseEvent>, _event_id: u64) -> Result<(), ProgramError> {
        ctx.accounts.close_event(&ctx.bumps)
    }

    /// Deactivate an event escrow — stops accepting deposits.
    /// Called by organizer when registration closes.
    /// Refunds are still allowed; close_event can be called after.
    #[instruction(discriminator = 6)]
    pub fn deactivate_event(ctx: Ctx<DeactivateEvent>, _event_id: u64) -> Result<(), ProgramError> {
        ctx.accounts.deactivate()
    }

    /// Close an attendee's deposit PDA to reclaim rent.
    /// Attendee can close after refund; anyone can close after event escrow is closed.
    #[instruction(discriminator = 7)]
    pub fn close_deposit(ctx: Ctx<CloseDeposit>, _event_id: u64) -> Result<(), ProgramError> {
        ctx.accounts.close_deposit()
    }

    /// Roll a checked-in attendee's deposit from one event to another.
    /// Same organizer, same deposit amount required. Atomic vault-to-vault transfer.
    #[instruction(discriminator = 8)]
    pub fn rollover_deposit(
        ctx: Ctx<RolloverDeposit>,
        _source_event_id: u64,
        _target_event_id: u64,
    ) -> Result<(), ProgramError> {
        ctx.accounts.validate_and_update(&ctx.bumps)
    }
}
