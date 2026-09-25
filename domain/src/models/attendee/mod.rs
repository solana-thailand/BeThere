//! Attendee records, sheet column mapping and walk-in registration types.

mod columns;
mod core;
mod recent;
mod roster_page;
mod row;
mod sheet_row;
mod status;
mod ticket_name;
mod walkin;

#[cfg(test)]
mod tests;

pub use columns::{ColumnKey, ColumnMapping, PII_COLUMNS};
pub use core::{Attendee, CheckInError};
pub use recent::{RECENT_CHECK_INS_PER_TYPE, recent_check_ins};
pub use roster_page::{ROSTER_PAGE_MAX, RosterPage, roster_page};
pub use row::AttendeeRow;
pub use sheet_row::{RowMatch, SheetRow, column_index, column_letter, find_row, range_start};
pub use status::{CheckInStatus, ParticipationType};
pub use ticket_name::{
    SYSTEM_TICKET_NAMES, TICKET_NAME_SELF_REGISTERED, TICKET_NAME_WALK_IN, is_system_ticket_name,
};
pub use walkin::WalkinAttendee;
