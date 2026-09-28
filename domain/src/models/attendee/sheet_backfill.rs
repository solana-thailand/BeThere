//! D1 → sheet backfill planner (`.issues/151`, "Not done: Backfill").
//!
//! Until the 151 fix every row write from D1 addressed row 0 and was dropped,
//! so the sheets of recent events are missing statuses that D1 holds. This
//! decides which cells to write. It fills **empty cells only**: organizers edit
//! these sheets by hand, and a value already there is theirs, even when it
//! differs from D1. It resolves rows by api_id the same way the live writers
//! do, and an id with no single matching row is counted and skipped.

use super::columns::{ColumnKey, ColumnMapping};
use super::sheet_row::{RowMatch, find_row};

/// One cell D1 says the sheet should hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellWant {
    pub api_id: String,
    pub column: ColumnKey,
    pub value: String,
}

/// One cell to write: 1-based sheet row, 0-based column index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellWrite {
    pub row: usize,
    pub column: usize,
    pub value: String,
}

/// What a backfill would do, in counts an organizer can check.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackfillPlan {
    pub writes: Vec<CellWrite>,
    /// The sheet already holds a value there (kept as is).
    pub already_filled: usize,
    /// The api_id is not in column A.
    pub row_missing: usize,
    /// The api_id is in column A more than once.
    pub row_duplicated: usize,
    /// The sheet has no column for the value (`ColumnMapping::resolve`).
    pub column_missing: usize,
}

/// `sheet` is the tab read from `A1`, header row first, rows as returned by
/// the Sheets API (trailing empty cells omitted).
pub fn plan_backfill(
    sheet: &[Vec<String>],
    mapping: &ColumnMapping,
    wants: &[CellWant],
) -> BackfillPlan {
    let mut plan = BackfillPlan::default();
    for want in wants.iter().filter(|w| !w.value.trim().is_empty()) {
        let row = match find_row(sheet, &want.api_id) {
            RowMatch::Found(row) => row,
            RowMatch::NotFound => {
                plan.row_missing += 1;
                continue;
            }
            RowMatch::Duplicate => {
                plan.row_duplicated += 1;
                continue;
            }
        };
        let Some(column) = mapping.resolve(want.column) else {
            plan.column_missing += 1;
            continue;
        };
        let current = sheet
            .get(row - 1)
            .and_then(|cells| cells.get(column))
            .map(|c| c.trim())
            .unwrap_or_default();
        if current.is_empty() {
            plan.writes.push(CellWrite {
                row,
                column,
                value: want.value.clone(),
            });
        } else {
            plan.already_filled += 1;
        }
    }
    plan
}
