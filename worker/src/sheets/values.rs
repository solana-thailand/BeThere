//! The shared tail of every Sheets cell writer: turn `(column, value)` pairs
//! into A1 ranges and send them as one `values:batchUpdate`.
//!
//! Callers find the row first (`locate::resolve_row`, or a pre-resolved batch)
//! and pass the 1-based row here; nothing in this module looks a row up.

use event_checkin_domain::models::attendee::{ColumnKey, ColumnMapping};

use super::a1;
use crate::http::{BatchUpdateRequest, ValueRange, batch_update_sheet};

/// The ranges for `cells` on 1-based `row_index`. A column the sheet lacks is
/// skipped (see [`a1::cell`]).
pub(crate) fn row_cells(
    sheet_ref: &str,
    mapping: &ColumnMapping,
    row_index: usize,
    cells: Vec<(ColumnKey, String)>,
) -> Vec<ValueRange> {
    cells
        .into_iter()
        .filter_map(|(key, value)| a1::cell(sheet_ref, mapping, key, row_index, value))
        .collect()
}

/// Send `data` as one `values:batchUpdate`. An empty `data` sends nothing.
pub(crate) async fn send(
    sheet_id: &str,
    data: Vec<ValueRange>,
    access_token: &str,
) -> Result<(), String> {
    let url =
        format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values:batchUpdate");
    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };
    batch_update_sheet(&url, &body, access_token).await
}

/// [`row_cells`] then [`send`]: write `cells` into an already-resolved row.
pub(crate) async fn write_cells(
    sheet_id: &str,
    sheet_ref: &str,
    mapping: &ColumnMapping,
    row_index: usize,
    cells: Vec<(ColumnKey, String)>,
    access_token: &str,
) -> Result<(), String> {
    let data = row_cells(sheet_ref, mapping, row_index, cells);
    send(sheet_id, data, access_token).await
}
