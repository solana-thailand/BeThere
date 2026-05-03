use quasar_lang::prelude::*;

/// PDA — one per event, holds all USDC deposits in a token account.
/// Seeds: ["escrow", organizer, event_id]
/// Space: 8 (discriminator) + 32 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8 + 1 + 1 = 156 bytes
#[account(discriminator = 1, set_inner)]
#[seeds(b"escrow", organizer: Address, event_id: u64)]
pub struct EventEscrow {
    /// Event organizer (can claim forfeited deposits).
    pub organizer: Address,
    /// Unique event identifier for PDA seed derivation.
    pub event_id: u64,
    /// USDC mint address.
    pub usdc_mint: Address,
    /// Token account owned by this PDA — holds deposited USDC.
    pub vault: Address,
    /// Fixed deposit amount in USDC smallest unit (6 decimals -> $15 = 15_000_000).
    pub deposit_amount: u64,
    /// Event end timestamp (unix seconds). Refunds allowed after this.
    pub event_end: i64,
    /// Refund deadline (event_end + grace period, e.g., +7 days).
    pub refund_deadline: i64,
    /// Total USDC deposited across all attendees.
    pub total_deposited: u64,
    /// Total USDC refunded.
    pub total_refunded: u64,
    /// Total USDC claimed by organizer (forfeited no-show deposits).
    pub total_forfeited: u64,
    /// Whether the event is active (deposits accepted).
    pub is_active: bool,
    /// Bump seed for PDA.
    pub bump: u8,
}

/// PDA — one per attendee per event.
/// Seeds: ["deposit", event, attendee]
/// Space: 8 (discriminator) + 32 + 32 + 8 + 8 + 1 + 1 + 1 = 91 bytes
#[account(discriminator = 2, set_inner)]
#[seeds(b"deposit", event: Address, attendee: Address)]
pub struct AttendeeDeposit {
    /// Attendee's wallet address.
    pub attendee: Address,
    /// Reference to EventEscrow.
    pub event: Address,
    /// Amount deposited (USDC smallest unit).
    pub amount: u64,
    /// Deposit timestamp.
    pub deposited_at: i64,
    /// Whether attendee checked in (set by organizer authority).
    pub checked_in: bool,
    /// Whether refund has been claimed.
    pub refunded: bool,
    /// Bump seed for PDA.
    pub bump: u8,
}
