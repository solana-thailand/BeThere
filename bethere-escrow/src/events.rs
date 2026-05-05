use quasar_lang::prelude::*;

#[event(discriminator = 0)]
pub struct EventCreated {
    pub escrow: Address,
    pub organizer: Address,
    pub deposit_amount: u64,
    pub event_end: i64,
    pub refund_deadline: i64,
}

#[event(discriminator = 1)]
pub struct Deposited {
    pub escrow: Address,
    pub attendee: Address,
    pub amount: u64,
}

#[event(discriminator = 2)]
pub struct CheckedIn {
    pub escrow: Address,
    pub attendee: Address,
}

#[event(discriminator = 3)]
pub struct Refunded {
    pub escrow: Address,
    pub attendee: Address,
    pub amount: u64,
}

#[event(discriminator = 4)]
pub struct ForfeitedClaimed {
    pub escrow: Address,
    pub organizer: Address,
    pub amount: u64,
}

#[event(discriminator = 5)]
pub struct EventClosed {
    pub escrow: Address,
}

#[event(discriminator = 6)]
pub struct EventDeactivated {
    pub escrow: Address,
    pub organizer: Address,
}
