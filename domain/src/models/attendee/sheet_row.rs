//! Finding an attendee's row in the attendee sheet by `api_id`.
//!
//! The sheet is edited by hand: organizers insert, move and delete rows. A
//! row number remembered from an earlier read is therefore never an address.
//! D1's `sheet_row_index` is also empty for every attendee D1 created, so
//! writers that trusted it wrote to row 0, which the API rejects, and every
//! status write from D1 silently failed. A write now names the attendee
//! ([`SheetRow`]) and looks the row up in column A just before writing.

use super::columns::index_to_column_letter;

/// The attendee a sheet write is for. It resolves to a row number only at
/// write time, from column A (`api_id`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetRow {
    api_id: String,
}

impl SheetRow {
    pub fn of(api_id: impl Into<String>) -> Self {
        Self {
            api_id: api_id.into(),
        }
    }

    pub fn api_id(&self) -> &str {
        &self.api_id
    }
}

/// Outcome of looking an `api_id` up in column A.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowMatch {
    /// The 1-based sheet row holding the `api_id` (always 2 or more).
    Found(usize),
    /// No row holds it (never synced to the sheet, deleted, or misplaced).
    NotFound,
    /// More than one row holds it, so any single write could hit the wrong
    /// copy. The writer skips rather than guess.
    Duplicate,
}

/// Look `api_id` up in `column_a`: the values of `A:A`, header row included,
/// as the Sheets API returns them (one inner vec per row, empty for a blank
/// cell). Row 1 is the header and never matches.
pub fn find_row(column_a: &[Vec<String>], api_id: &str) -> RowMatch {
    let wanted = api_id.trim();
    if wanted.is_empty() {
        return RowMatch::NotFound;
    }
    let mut rows = column_a
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, cells)| cells.first().is_some_and(|c| c.trim() == wanted))
        .map(|(i, _)| i + 1);
    match (rows.next(), rows.next()) {
        (Some(row), None) => RowMatch::Found(row),
        (Some(_), Some(_)) => RowMatch::Duplicate,
        (None, _) => RowMatch::NotFound,
    }
}

/// The spreadsheet letter for a 0-based column index (0 → `A`, 26 → `AA`).
pub fn column_letter(idx: usize) -> String {
    index_to_column_letter(idx)
}

/// The 0-based index of a column letter (`A` → 0, `AA` → 26). `None` for
/// anything that is not one or more ASCII letters.
pub fn column_index(letters: &str) -> Option<usize> {
    if letters.is_empty() || !letters.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    let n = letters.bytes().fold(0usize, |acc, b| {
        acc * 26 + usize::from(b.to_ascii_uppercase() - b'A') + 1
    });
    Some(n - 1)
}

/// The first cell of an A1 range as `(0-based column, 1-based row)`, e.g.
/// `'VIP list'!B57:AI57` → `(1, 57)`. Used on an append's `updatedRange` to
/// see where Google actually wrote the row.
pub fn range_start(range: &str) -> Option<(usize, usize)> {
    let cells = range.rsplit_once('!').map_or(range, |(_, cells)| cells);
    let first = cells.split(':').next()?;
    let split = first.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = first.split_at(split);
    let row: usize = digits.parse().ok()?;
    Some((column_index(letters)?, row))
}
