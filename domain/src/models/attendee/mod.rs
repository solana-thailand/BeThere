//! Attendee records, sheet column mapping and walk-in registration types.

mod columns;
mod core;
mod row;
mod status;
mod walkin;

#[cfg(test)]
mod tests;

pub use columns::{ColumnKey, ColumnMapping};
pub use core::{Attendee, CheckInError};
pub use row::AttendeeRow;
pub use status::{CheckInStatus, ParticipationType};
pub use walkin::WalkinAttendee;
