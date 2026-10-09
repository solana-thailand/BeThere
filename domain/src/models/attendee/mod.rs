//! Attendee records, sheet column mapping and walk-in registration types.

mod attendance_answer;
mod columns;
mod core;
mod display_code;
mod duplicates;
mod recent;
mod roster_page;
mod row;
mod sheet_backfill;
mod sheet_row;
mod status;
mod ticket_name;
mod track_counts;
mod walkin;

#[cfg(test)]
mod tests;

pub use attendance_answer::AttendanceAnswer;
pub use columns::{ColumnKey, ColumnMapping, PII_COLUMNS};
pub use core::{Attendee, CheckInError};
pub use display_code::{
    DISPLAY_CODE_ALPHABET, DISPLAY_CODE_LEN, DISPLAY_CODE_RANDOM_BYTES, DisplayCode,
    DisplayCodeError,
};
pub use duplicates::{DuplicateMatch, DuplicateReason, possible_duplicates};
pub use recent::{RECENT_CHECK_INS_PER_TYPE, recent_check_ins};
pub use roster_page::{ROSTER_PAGE_MAX, RosterPage, roster_page};
pub use row::AttendeeRow;
pub use sheet_backfill::{BackfillPlan, CellWant, CellWrite, plan_backfill};
pub use sheet_row::{RowMatch, SheetRow, column_index, column_letter, find_row, range_start};
pub use status::{CheckInStatus, PARTICIPATION_WALK_IN, ParticipationType};
pub use ticket_name::{
    SYSTEM_TICKET_NAMES, TICKET_NAME_SELF_REGISTERED, TICKET_NAME_WALK_IN, is_system_ticket_name,
};
pub use track_counts::TrackCounts;
pub use walkin::WalkinAttendee;
