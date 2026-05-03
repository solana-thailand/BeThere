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
}
