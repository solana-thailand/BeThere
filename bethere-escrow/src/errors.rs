use quasar_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    /// Deposit amount does not match the required deposit_amount.
    IncorrectDepositAmount = 0,
    /// Refund period has not started yet (event_end not reached).
    RefundNotYetAllowed = 1,
    /// Attendee has not been checked in.
    NotCheckedIn = 2,
    /// Refund deadline has not passed yet — organizer cannot claim forfeited.
    RefundDeadlineNotPassed = 3,
    /// This deposit has already been refunded.
    AlreadyRefunded = 4,
    /// Attendee was checked in — deposit cannot be forfeited.
    AttendeeCheckedIn = 5,
    /// No forfeited funds to claim.
    NoForfeitedFunds = 6,
    /// Event is not active — deposits not accepted.
    EventNotActive = 7,
    /// Event still active — cannot close.
    EventStillActive = 8,
    /// Unauthorized signer.
    Unauthorized = 9,
    /// Vault account does not match the escrow's stored vault.
    VaultMismatch = 10,
    /// USDC mint does not match the escrow's stored mint.
    MintMismatch = 11,
    /// Deposit amount must be greater than zero.
    InvalidDepositAmount = 12,
    /// Event end must be in the future.
    EventEndInPast = 13,
    /// Arithmetic overflow.
    Overflow = 14,
    /// Vault still has tokens — settle all refunds/claims first.
    VaultNotEmpty = 15,
    /// Event has ended — check-ins no longer allowed.
    EventEnded = 16,
    /// Deposit has not been refunded yet — cannot close.
    DepositNotRefunded = 17,
    /// Event escrow still has data and deposit not refunded.
    EventEscrowStillActive = 18,
    /// Refund deadline has passed — organizer may claim forfeited.
    RefundDeadlinePassed = 19,
    /// EventEscrow schema version is not supported by this program.
    EscrowVersionMismatch = 20,
    /// AttendeeDeposit schema version is not supported by this program.
    DepositVersionMismatch = 21,
}
