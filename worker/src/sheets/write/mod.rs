//! Sheet mutation operations: check-in, claim, QR URL updates, row appends, and deposit writes.

mod append;
mod checkin;
mod deposit;

use event_checkin_domain::models::attendee::ColumnMapping;
use worker::KvStore;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Sheet write context — bundles repeated sheet operation parameters
// ---------------------------------------------------------------------------

/// Bundles the parameters commonly passed to sheet mutation functions.
/// Reduces argument count and avoids repeating sheet_id/sheet_name/kv everywhere.
pub struct SheetContext<'a> {
    pub mapping: &'a ColumnMapping,
    pub state: &'a AppState,
    pub sheet_id: &'a str,
    pub sheet_name: &'a str,
    pub kv: Option<&'a KvStore>,
}

// ---------------------------------------------------------------------------
// Re-exports — keep the public API identical to the old monolithic module
// ---------------------------------------------------------------------------

pub use append::{
    append_attendee_row, append_walkin_row, clear_sheet_cells_batch, delete_sheet_row,
    update_participation_type,
};
pub use checkin::{
    clear_checked_in, mark_checked_in, mark_claimed, mark_virtual_checked_in, update_qr_urls,
};
pub use deposit::{
    update_deposit_method, write_bank_info, write_deposit_verification, write_refund_link,
    write_refund_status, write_refund_status_batch,
};
