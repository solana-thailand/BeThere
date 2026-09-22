//! Attendee records, sheet column mapping and walk-in registration types.

mod columns;
mod core;
mod row;
mod status;
mod ticket_name;
mod walkin;

#[cfg(test)]
mod tests;

pub use columns::{ColumnKey, ColumnMapping};
pub use core::{Attendee, CheckInError};
pub use row::AttendeeRow;
pub use status::{CheckInStatus, ParticipationType};
pub use ticket_name::{
    SYSTEM_TICKET_NAMES, TICKET_NAME_SELF_REGISTERED, TICKET_NAME_WALK_IN, is_system_ticket_name,
};
pub use walkin::WalkinAttendee;
